#!/usr/bin/env python3
"""Measure map update/publication and retained-version reads separately."""
import argparse
import gzip
import json
from pathlib import Path
import random
import statistics
import subprocess
import tempfile

import applications as app

CASES = {
    **{f'retain-{size}': dict(mode='retain', n=131072, size=size, interval=0)
       for size in [64, 1024, 4096]},
    **{f'{mode}-{style}-{size}': dict(mode=mode, n=65536, size=size, interval=interval)
       for mode in ['scalar', 'pointers'] for size in [256, 1024]
       for style, interval in [('private', 0), ('every-update', 1), ('snapshots', 64)]},
}


def oracle(config):
    if config['mode'] == 'retain':
        return config['n'] * (config['size'] + 7), config['n']
    current, checksum, versions = {}, 0, 0
    for i in range(config['n']):
        current[i % config['size']] = i if config['mode'] == 'scalar' else 1 + i % 127
        if config['interval'] and (i + 1) % config['interval'] == 0:
            checksum += len(current) + sum((k + 1) * v for k, v in current.items())
            versions += 1
    return checksum + len(current) + sum((k + 1) * v for k, v in current.items()), versions + 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--baseline-revision', required=True)
    parser.add_argument('--candidate-revision')
    parser.add_argument('--cases', nargs='+', choices=CASES, default=list(CASES))
    parser.add_argument('--workers', type=int, nargs='+', default=[1, 2, 4, 8])
    parser.add_argument('--reps', type=int, default=5)
    parser.add_argument('--seed', type=int, default=2026101960)
    parser.add_argument('--diagnostic', action='store_true')
    parser.add_argument('--allocations', action='store_true')
    parser.add_argument('--smoke', action='store_true')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if min(args.reps, *args.workers) < 1 or max(args.workers) > 64:
        parser.error('positive repetitions and 1..64 workers required')
    if args.candidate and not args.candidate_revision:
        parser.error('candidate revision is required')
    if args.allocations and not args.diagnostic:
        parser.error('allocation census requires diagnostic binaries')
    with app.gc.benchmark_lock():
        run(args)


def run(args):
    args.out.mkdir(parents=True, exist_ok=False)
    binaries = {'baseline': args.baseline.resolve()}
    if args.candidate:
        binaries['candidate'] = args.candidate.resolve()
    source = app.gc.ROOT / 'bench/gc/map_phases.bdg'
    sources = [source, Path(__file__), Path(app.__file__), Path(app.gc.__file__)]
    report = dict(arguments=vars(args) | {k: str(v) for k, v in vars(args).items() if isinstance(v, Path)},
                  binaries={k: dict(path=str(p), sha256=app.rw.digest(p)) for k, p in binaries.items()},
                  sources={str(p.relative_to(app.gc.ROOT)): app.rw.digest(p) for p in sources},
                  runs=[], status='running')
    configs = {k: CASES[k] | (dict(n=79, size=17) if args.smoke else {}) for k in args.cases}
    expected = {k: oracle(c) for k, c in configs.items()}
    env = app.rw.clean_env()
    rng = random.Random(args.seed)
    with tempfile.TemporaryDirectory(prefix='badger-map-phases-') as temporary:
        temporary = Path(temporary)
        programs = {}
        for label, binary in binaries.items():
            program = temporary / f'{label}.bdgmir'
            compiled = subprocess.run([str(binary), 'compile', str(source), '-o', str(program)],
                                      cwd=app.gc.ROOT, env=env, capture_output=True, text=True, timeout=180)
            assert compiled.returncode == 0, (compiled.stdout, compiled.stderr)
            programs[label] = program
        hashes = {label: app.rw.digest(p) for label, p in programs.items()}
        assert len(set(hashes.values())) == 1, 'runtime-only comparison requires identical saved MIR'
        report['program_sha256'] = hashes
        (args.out / 'program.bdgmir.gz').write_bytes(gzip.compress(next(iter(programs.values())).read_bytes(), mtime=0))
        cells = [(case, workers) for case in args.cases for workers in args.workers]
        for repetition in range(-1, args.reps):
            order = cells.copy()
            rng.shuffle(order)
            for case, workers in order:
                labels = list(binaries)
                rng.shuffle(labels)
                for label in labels:
                    phases, census = temporary / 'gc.json', temporary / 'allocations.json'
                    settings = dict(MAP_PHASE_CONFIG=json.dumps(configs[case]))
                    if args.diagnostic:
                        settings['BADGER_GC_PROFILE'] = str(phases)
                    if args.allocations:
                        settings['BADGER_ALLOC_PROFILE'] = str(census)
                    command = [str(binaries[label]), 'run', '--release', '--bench', '--workers', str(workers), str(programs[label])]
                    before = app.gc.host_context()
                    process = app.rw.process(command, env | settings, 180)
                    after = app.gc.host_context()
                    record = json.loads(process['stdout'])
                    assert record['config'] == configs[case]
                    assert (record['checksum'], record['versions']) == expected[case], (case, record, expected[case])
                    assert record['update_ns'] > 0 and record['read_ns'] > 0
                    profile = json.loads(phases.read_text()) if args.diagnostic else None
                    allocations = json.loads(census.read_text()) if args.allocations else None
                    if profile is not None:
                        app.gc.validate_sweep(profile)
                        phases.unlink()
                    if allocations is not None:
                        app.gc.validate_allocations(allocations)
                        census.unlink()
                    if repetition >= 0:
                        report['runs'].append(dict(case=case, workers=workers, variant=label, repetition=repetition,
                                                   command=command, settings=settings, record=record, process=process,
                                                   gc=profile, allocations=allocations, host_before=before, host_after=after))
                        save(args.out, report)
            print(f'repetition {repetition}: checked', flush=True)
    assert all(app.rw.digest(p) == report['binaries'][label]['sha256'] for label, p in binaries.items())
    assert all(app.rw.digest(p) == report['sources'][str(p.relative_to(app.gc.ROOT))] for p in sources)
    report['status'] = 'complete'
    save(args.out, report)
    lines = ['| Case | Workers | Variant | Update/retain ms | Read ms |', '|---|---:|---|---:|---:|']
    for case, workers in cells:
        for label in binaries:
            rows = [r['record'] for r in report['runs'] if (r['case'], r['workers'], r['variant']) == (case, workers, label)]
            times = [statistics.median(r[k] for r in rows) / 1e6 for k in ['update_ns', 'read_ns']]
            lines.append(f'| {case} | {workers} | {label} | {times[0]:.3f} | {times[1]:.3f} |')
    (args.out / 'summary.md').write_text('\n'.join(lines) + '\n')


def save(directory, report):
    (directory / 'measurements.json.gz').write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))


if __name__ == '__main__':
    main()
