#!/usr/bin/env python3
"""Stress completed-task roots, returned aliases, cleanups and post-GC inspection."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from run import ROOT, validate_coverage

INSPECT = '''import std.array
import std.codec
import std.inspect
import std.io
import std.task
pub fn main() -> Result<Unit, io.Error> {
    let r = task.scope(fn(s) -> Result<Unit, String> {
        let job = s.spawn(fn() { 42 })
        assert(job.join() == 42)
        for i in 0..1024 {
            let values = array.build(256, fn(j: Int) { i * 31 + j })
            assert(values[255] == i * 31 + 255)
        }
        let _ = io.println(codec.encode_text(inspect.tasks()))
        ok(())
    })
    assert(r.is_ok())
    ok(())
}
'''


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--ordinary', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    binary = args.ordinary.resolve()
    clean = {k: v for k, v in os.environ.items() if not k.startswith('BADGER_') and k != 'GC_BENCH_CONFIG'}
    clean['BADGER_GC_POOL_MAX'] = 'unsupported'  # Refuse diagnostic artifacts.
    results = []
    with tempfile.TemporaryDirectory(prefix='badger-task-roots-') as directory:
        mir = Path(directory) / 'lifetime.bdgmir'
        probe = Path(directory) / 'inspect.bdg'
        probe.write_text(INSPECT)
        command = [str(binary), 'compile', 'tests/run/task_input_lifetime.bdg', '-o', str(mir)]
        subprocess.run(command, cwd=ROOT, env=clean, capture_output=True, check=True, timeout=60)
        def execute(source, workers, generation, chaos, config=None):
            env = clean | dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY='1', BADGER_GC_GENERATIONAL=str(generation))
            if chaos:
                env['BADGER_CHAOS'] = str(chaos)
            if config:
                env['GC_BENCH_CONFIG'] = json.dumps(config)
            command = [str(binary), 'run', '--release', '--workers', str(workers), str(source)]
            output = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=180)
            assert output.returncode == 0, (command, env, output.stdout, output.stderr)
            if config:
                validate_coverage(json.loads(output.stdout), config)
            elif source == probe:
                tasks = json.loads(output.stdout)
                done = [t for t in tasks if t['state'] == 'done']
                assert done and all(t['memory']['captures'] == 0 for t in done), tasks
            else:
                assert output.stdout == (ROOT / 'tests/run/task_input_lifetime.out').read_text(), output.stdout
            results.append(dict(command=command, generational=generation, chaos=chaos, config=config,
                                stdout=output.stdout, stderr=output.stderr))
        for generation in [0, 1]:
            for workers in [1, 4, 8]:
                for chaos in [0, 731]:
                    for source in [Path('tests/run/task_input_lifetime.bdg'), mir, probe]:
                        execute(source, workers, generation, chaos)
            for workers in [1, 8]:
                for mode in ['tasks-input', 'tasks-result']:
                    execute(Path('bench/gc/coverage.bdg'), workers, generation, 947,
                            dict(mode=mode, n=8, rounds=4096, payload=257, retain=64))
    with binary.open('rb') as file:
        digest = hashlib.file_digest(file, 'sha256').hexdigest()
    sources = {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in
               ['bench/gc/check_task_roots.py', 'bench/gc/coverage.bdg', 'tests/run/task_input_lifetime.bdg']}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(dict(binary=str(binary), sha256=digest, source_sha256=sources,
                                       inspection_source=INSPECT, runs=results), indent=2) + '\n')
    print(f'{len(results)} task input/result/cleanup/inspection stress checks passed')


if __name__ == '__main__':
    main()
