#!/usr/bin/env python3
"""Dependency-free, correctness-gated language comparison. See README.md."""
from __future__ import annotations

import argparse
from dataclasses import dataclass, field
from datetime import datetime, timezone
from decimal import Decimal
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import shlex
import shutil
import signal
import statistics
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
PROGRAMS = HERE / 'programs'
BUILD = HERE / '.build'
TOOLS = HERE / '.tools'
WORKLOADS = {
    'cpu': (200_000, 'Eight rounds of bounded integer arithmetic per event; CPU control.'),
    'records': (100_000, 'Build records, retain originals, enrich copies, filter and reduce both versions.'),
    'snapshots': (16_384, 'Update a 256-key map; retain a snapshot every 64 updates; read all versions.'),
    'handoff': (100_003, 'One producer and one consumer; bounded queue of 256 scalar values.'),
    'batches': (100_003, 'Same values in fixed 64-item batches; four queued batches; includes partial tail.'),
}
COMMON = tuple(WORKLOADS)[:3]
ALL = tuple(WORKLOADS)


class BenchError(RuntimeError):
    pass


def execute(command, *, env=None, timeout=120):
    """Kill the whole process group on timeout, including compiler/task children."""
    started = time.perf_counter_ns()
    with subprocess.Popen([str(x) for x in command], cwd=ROOT, env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          text=True, start_new_session=True) as process:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
            raise BenchError(f'timeout after {timeout}s: {shlex.join(map(str, command))}\n{stderr[-3000:]}')
    result = dict(command=list(map(str, command)), wall_ns=time.perf_counter_ns() - started,
                  returncode=process.returncode, stdout=stdout, stderr=stderr)
    if process.returncode:
        raise BenchError(json.dumps(result, ensure_ascii=False)[-6000:])
    return result


def oracle(name, n, seed):
    """Independent contract oracle, including closed-form CPU recurrence."""
    if name == 'cpu':
        multiplier = pow(17, 8, 1_000_003)
        offset = 23 * sum(17 ** k for k in range(8))
        return sum((multiplier * ((i + seed) % 1_000_003) + offset) % 1_000_003 for i in range(n))
    if name == 'records':
        raw = sum((i + seed) % 1000 for i in range(n))
        active = sum(3 * ((i + seed) % 1000) + 7 for i in range(n) if i % 3)
        return raw + active
    if name == 'snapshots':
        total = 0
        for stop in [*range(64, n + 1, 64), n]:
            for key in range(min(stop, 256)):
                latest = stop - 1 - ((stop - 1 - key) % 256)
                total += (key + 1) * (latest + seed)
        return total
    if name in ('handoff', 'batches'):
        return n * (n - 1) // 2 + n * seed
    raise ValueError(name)


def parse_samples(stdout, count, expected):
    lines = stdout.splitlines()
    if len(lines) != count:
        raise BenchError(f'expected {count} protocol lines, got {len(lines)}: {stdout[:1000]}')
    samples = []
    for index, line in enumerate(lines):
        match = re.fullmatch(r'BENCH (\d+) (\d+) (-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)', line)
        if not match:
            raise BenchError(f'invalid protocol: {line[:500]}')
        iteration, elapsed = map(int, match.groups()[:2])
        checksum = Decimal(match.group(3))
        if iteration != index or elapsed <= 0 or checksum != expected:
            raise BenchError(f'correctness/timer failure: {line}; expected index={index}, checksum={expected}')
        samples.append(dict(iteration=iteration, elapsed_ns=elapsed, checksum=int(checksum)))
    return samples


def stats(values):
    if not values:
        return None
    median = statistics.median(values)
    mean = statistics.mean(values)
    return dict(n=len(values), median_ns=median, min_ns=min(values), max_ns=max(values),
                cv_pct=100 * statistics.pstdev(values) / mean if mean else 0)


@dataclass
class Variant:
    id: str
    family: str
    label: str
    command: list
    version_command: list
    builds: list = field(default_factory=list)
    artifacts: list = field(default_factory=list)
    workloads: tuple = ALL
    env: dict = field(default_factory=dict)
    note: str = ''
    missing: str = ''
    setup: list = field(default_factory=list)


def locate(variable, name, candidates=()):
    if os.environ.get(variable):
        return os.environ[variable]
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    return shutil.which(name) or ''


def variants():
    lang = locate('COMPARE_LANG', 'lang', [ROOT / 'target/release/lang'])
    python = locate('COMPARE_PYTHON', 'python3')
    node = locate('COMPARE_NODE', 'node')
    go = locate('COMPARE_GO', 'go')
    rust = locate('COMPARE_RUSTC', 'rustc')
    cpp = locate('COMPARE_CXX', 'g++')
    lua = locate('COMPARE_LUA', 'lua')
    luac = locate('COMPARE_LUAC', 'luac')
    luajit = locate('COMPARE_LUAJIT', 'luajit')
    java = locate('COMPARE_JAVA', 'java', sorted((TOOLS / 'jdk').glob('*/bin/java')))
    javac = locate('COMPARE_JAVAC', 'javac', [Path(java).with_name('javac')] if java else [])
    bb = locate('COMPARE_BB', 'bb', [TOOLS / 'babashka/bb'])
    jank = locate('COMPARE_JANK', 'jank', [TOOLS / 'jank/inst/usr/local/bin/jank'])
    cc = locate('COMPARE_CC', 'cc')
    jars = os.environ.get('COMPARE_CLOJURE_CP') or os.pathsep.join(map(str, sorted((TOOLS / 'clojure').glob('*.jar'))))
    clojure = locate('COMPARE_CLOJURE', 'clojure')
    cljbase = ([java, '-cp', jars + os.pathsep + str(PROGRAMS), 'clojure.main'] if java and jars
               else [clojure, '-Sdeps', json.dumps({'paths': [str(PROGRAMS)]})])
    cljaot = [java, '-cp', str(BUILD / 'clojure') + os.pathsep + jars, 'clojure.main', '-m', 'workload']
    result = []
    for workers in (1, 4):
        for saved in (False, True):
            artifact = BUILD / 'workload.langmir'
            result.append(Variant(
                f'lang-{"mir" if saved else "source"}-w{workers}', 'lang',
                f'Lang {"saved MIR" if saved else "source"} · {workers} worker(s)',
                [lang, 'run', '--release', '--workers', str(workers), artifact if saved else PROGRAMS / 'workload.lang', '--'],
                [lang, '--version'],
                [[lang, 'compile', PROGRAMS / 'workload.lang', '-o', artifact]] if saved else [],
                [artifact] if saved else [],
                note=('Release Cranelift JIT. Current lang compile saves MIR without release-only inlining/receive-splitting; artifact launch does not rerun those passes.'
                      if saved else 'Release MIR inlining/receive-splitting plus Cranelift JIT; source frontend runs at each launch.'),
                missing='' if lang else 'Build the release compiler: cargo build --release --locked'))
    pyc = BUILD / 'workload.pyc'
    result.extend([
        Variant('python-source', 'python', 'Python source', [python, '-B', PROGRAMS / 'workload.py'], [python, '--version']),
        Variant('python-pyc', 'python', 'Python bytecode (.pyc)', [python, '-B', pyc], [python, '--version'],
                [[python, '-c', 'import py_compile,sys; py_compile.compile(sys.argv[1], cfile=sys.argv[2], doraise=True)', PROGRAMS / 'workload.py', pyc]], [pyc],
                note='.pyc is cached bytecode, executed by the same interpreter.'),
        Variant('node', 'node', 'Node.js source (V8 JIT)', [node, PROGRAMS / 'workload.js'], [node, '--version']),
        Variant('go-source', 'go', 'Go source launcher (go run)', [go, 'run', PROGRAMS / 'workload.go'], [go, 'version'], env={'GOMAXPROCS': '4'},
                note='go run uses Go’s normal warm build cache. Wall time includes launcher/build/cache lookup.'),
        Variant('go-native', 'go', 'Go native', [BUILD / 'go-workload'], [go, 'version'],
                [[go, 'build', '-o', BUILD / 'go-workload', PROGRAMS / 'workload.go']], [BUILD / 'go-workload'], env={'GOMAXPROCS': '4'}),
        Variant('rust-native', 'rust', 'Rust native (release)', [BUILD / 'rust-workload'], [rust, '--version'],
                [[rust, '--edition=2024', '-O', PROGRAMS / 'workload.rs', '-o', BUILD / 'rust-workload']], [BUILD / 'rust-workload']),
        Variant('cpp-native', 'cpp', 'C++ native (-O2)', [BUILD / 'cpp-workload'], [cpp, '--version'],
                [[cpp, '-std=c++20', '-O2', '-pthread', PROGRAMS / 'workload.cpp', '-o', BUILD / 'cpp-workload']], [BUILD / 'cpp-workload'],
                note='Standard mutex/condition-variable queue; C++ has no standard bounded channel.'),
    ])
    for name, runtime, compiler, include, version in [
        ('lua', lua, luac, os.environ.get('COMPARE_LUA_INCLUDE', '/usr/include'), '-v'),
        ('luajit', luajit, luajit, os.environ.get('COMPARE_LUAJIT_INCLUDE', '/usr/include/luajit-2.1'), '-v')]:
        timer = BUILD / name / 'benchclock.so'
        timer.parent.mkdir(parents=True, exist_ok=True)
        setup = [[cc, '-O2', '-shared', '-fPIC', '-I' + include, PROGRAMS / 'benchclock.c', '-o', timer]]
        for saved in (False, True):
            artifact = BUILD / (name + '.bytecode')
            build = ([compiler, '-o', artifact, PROGRAMS / 'workload.lua'] if name == 'lua'
                     else [compiler, '-b', PROGRAMS / 'workload.lua', artifact])
            result.append(Variant(
                name + ('-bytecode' if saved else '-source'), name,
                ('LuaJIT' if name == 'luajit' else 'Lua') + (' bytecode' if saved else ' source'),
                [runtime, artifact if saved else PROGRAMS / 'workload.lua'], [runtime, version],
                [build] if saved else [], [artifact] if saved else [], workloads=COMMON,
                env={'LUA_CPATH': str(timer.parent / '?.so')}, setup=setup,
                note='Portable Lua workloads; native monotonic clock instrumentation. No built-in parallel task/channel runtime.'))
    result.extend([
        Variant('java-source', 'java', 'Java source launcher', [java, PROGRAMS / 'Workload.java'], [java, '-version'],
                note='Java 21+ virtual producer, ArrayBlockingQueue; source launcher compiles at startup.'),
        Variant('java-class', 'java', 'Java saved JVM classes', [java, '-cp', BUILD / 'java', 'Workload'], [java, '-version'],
                [[javac, '-d', BUILD / 'java', PROGRAMS / 'Workload.java']], [BUILD / 'java'],
                note='Saved bytecode still uses HotSpot JIT; virtual producer and ArrayBlockingQueue.'),
        Variant('clojure-source', 'clojure', 'Clojure JVM source', cljbase + [PROGRAMS / 'clojure_source.clj'],
                cljbase + ['-e', '(println (clojure-version))'],
                note='Persistent maps; future producer with Java bounded queue; normal boxed Clojure.'),
        Variant('clojure-aot', 'clojure', 'Clojure JVM AOT classes', cljaot,
                cljbase + ['-e', '(println (clojure-version))'],
                [cljbase + ['-e', f'(binding [*compile-path* "{BUILD / "clojure"}"] (compile (quote core-workloads)) (compile (quote workload)))']],
                [BUILD / 'clojure'], missing='' if java and jars else 'Clojure AOT needs COMPARE_JAVA and COMPARE_CLOJURE_CP'),
        Variant('babashka', 'babashka', 'Clojure · Babashka', [bb, '-cp', PROGRAMS, PROGRAMS / 'clojure_source.clj'], [bb, '--version'],
                note='Babashka SCI execution, same Clojure sources; not a native compilation of the workload.'),
    ])
    # Keep source JIT module lookup away from objects emitted by the AOT build.
    jank_source_options = [jank, '--module-path', PROGRAMS, '--target-dir', BUILD / 'jank-jit', '-O2']
    jank_options = [jank, '--module-path', PROGRAMS, '--target-dir', BUILD / 'jank-aot', '-O2']
    result.extend([
        Variant('jank-source', 'jank', 'jank source (JIT)', jank_source_options + ['run', PROGRAMS / 'jank_source.jank', '--'],
                [jank, 'print-binary-version'], workloads=COMMON,
                note='Same core Clojure workloads; C++ interop supplies only the monotonic clock.'),
        Variant('jank-native', 'jank', 'jank native AOT (-O2)', [BUILD / 'jank-aot/comparison'], [jank, 'print-binary-version'],
                [jank_options + ['--name', 'comparison', 'compile', 'jank-workload']], [BUILD / 'jank-aot/comparison'], workloads=COMMON,
                note='Static AOT runtime; same source as jank JIT. No Java queue substitute.'),
    ])
    for variant in result:
        variant.command = list(map(str, variant.command))
        variant.version_command = list(map(str, variant.version_command))
        if not variant.version_command[0] and not variant.missing:
            variant.missing = f'{variant.family} executable not found; see README for overrides'
        variant.env = {**({'JAVA_TOOL_OPTIONS': '-Djdk.virtualThreadScheduler.parallelism=4'} if variant.family == 'java' else {}), **variant.env}
    return result


def environment(variant):
    # Ambient compiler profiling and runtime tuning would contaminate measurements.
    env = {k: v for k, v in os.environ.items() if not k.startswith('LANG_')}
    for name in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'NODE_OPTIONS', 'PYTHONPATH', 'PYTHONSTARTUP', 'LUA_INIT', 'LUA_INIT_5_5'):
        env.pop(name, None)
    env.update(variant.env)
    return env


def measure(variant, name, n, seed, warmups, repeats, timeout):
    result = execute(variant.command + list(map(str, (name, n, seed, warmups, repeats))),
                     env=environment(variant), timeout=timeout)
    result['samples'] = parse_samples(result['stdout'], warmups + repeats, oracle(name, n, seed))
    result.update(n=n, seed=seed, warmups=warmups, repeats=repeats)
    return result


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def artifact_bytes(paths):
    return sum(p.stat().st_size for path in paths for p in ([path] if path.is_file() else path.rglob('*')) if p.is_file())


def metadata(args):
    files = [*PROGRAMS.glob('*'), HERE / 'run.py', HERE / 'report.py']
    compiler = [ROOT / 'Cargo.lock', ROOT / 'Cargo.toml', *sorted((ROOT / 'crates').rglob('*.rs')), *sorted((ROOT / 'crates').rglob('*.lang'))]
    sources = {str(p.relative_to(ROOT)): digest(p) for p in files + compiler if p.is_file()}
    cpu = Path('/proc/cpuinfo')
    model = next((line.split(':', 1)[1].strip() for line in cpu.read_text().splitlines() if line.startswith('model name')), platform.processor()) if cpu.exists() else platform.processor()
    return dict(utc=datetime.now(timezone.utc).isoformat(), revision=execute(['git', 'rev-parse', 'HEAD'])['stdout'].strip(),
                git_status=execute(['git', 'status', '--short'])['stdout'], platform=platform.platform(),
                cpu=model, logical_cpus=os.cpu_count(), affinity=sorted(os.sched_getaffinity(0)) if hasattr(os, 'sched_getaffinity') else None,
                load_average=os.getloadavg() if hasattr(os, 'getloadavg') else None,
                arguments=vars(args), source_sha256=sources,
                excluded_environment_names=sorted(k for k in os.environ if k.startswith('LANG_')))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=HERE / 'results/latest', help='Output prefix (.json and .html)')
    parser.add_argument('--languages', nargs='+', help='Families or variant IDs; default all')
    parser.add_argument('--workloads', nargs='+', choices=ALL, default=list(ALL))
    parser.add_argument('--smoke', action='store_true', help='257 items, 1 fresh process, 1 warmup, 2 samples')
    parser.add_argument('--fresh', type=int, default=5, help='Independent fresh processes per cell')
    parser.add_argument('--warmups', type=int, default=5)
    parser.add_argument('--samples', type=int, default=7, help='Measured iterations in a single warmed process')
    parser.add_argument('--build-samples', type=int, default=3)
    parser.add_argument('--timeout', type=float, default=120, help='Seconds per process (whole group killed)')
    parser.add_argument('--seed', type=int, default=37, help='Shared workload input seed (0..1000000)')
    parser.add_argument('--order-seed', type=int, default=20261016)
    parser.add_argument('--list', action='store_true')
    parser.add_argument('--require-all', action='store_true', help='Fail if a selected runtime is missing')
    parser.add_argument('--report-only', type=Path, help='Regenerate HTML from a saved JSON result')
    args = parser.parse_args()
    if args.report_only:
        from report import render
        data = json.loads(args.report_only.read_text())
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.with_suffix('.html').write_text(render(data))
        return 0
    if min(args.fresh, args.warmups, args.samples, args.build_samples) < 1 or args.timeout <= 0 or not 0 <= args.seed <= 1_000_000:
        parser.error('counts and timeout must be positive; seed must be 0..1000000')
    if args.smoke:
        args.fresh, args.warmups, args.samples, args.build_samples = 1, 1, 2, 1
    BUILD.mkdir(parents=True, exist_ok=True)
    for directory in ('java', 'clojure'):
        (BUILD / directory).mkdir(exist_ok=True)
    selected = variants()
    if args.languages:
        unknown = set(args.languages) - {v.family for v in selected} - {v.id for v in selected}
        if unknown:
            parser.error(f'unknown languages/variants: {sorted(unknown)}')
        selected = [v for v in selected if v.id in args.languages or v.family in args.languages]
    if args.list:
        for v in selected:
            print(f'{v.id:24} {v.label}: {v.missing or "detected (not validated)"}')
        return 0
    data = dict(schema=1, metadata=metadata(args), workloads={k: dict(n=257 if args.smoke else WORKLOADS[k][0], description=WORKLOADS[k][1]) for k in args.workloads},
                variants=[], results=[])
    # Paths in metadata must be JSON serializable.
    data['metadata']['arguments'] = {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    from report import render

    def save():
        args.out.with_suffix('.json').write_text(json.dumps(data, indent=2) + '\n')
        args.out.with_suffix('.html').write_text(render(data))

    runnable = []
    setup_done = set()
    failures = 0
    for v in selected:
        info = dict(id=v.id, family=v.family, label=v.label, command=v.command, env=v.env,
                    note=v.note, supported_workloads=list(v.workloads), status='missing' if v.missing else 'preparing', reason=v.missing, builds=[], setup=[])
        data['variants'].append(info)
        if v.missing:
            print(f'MISSING {v.id}: {v.missing}', flush=True)
            continue
        try:
            version = execute(v.version_command, env=environment(v), timeout=20)
            info['version'] = (version['stdout'] + version['stderr']).strip()
            executable = Path(v.version_command[0])
            info['tool_executable'] = str(executable)
            if executable.is_file():
                info['tool_sha256'] = digest(executable)
            # A different program named bb must never stand in for Babashka.
            if v.family == 'babashka' and 'babashka' not in info['version'].lower():
                info.update(status='missing', reason='bb on PATH is not Babashka; set COMPARE_BB')
                print(f'MISSING {v.id}: {info["reason"]}', flush=True)
                continue
            for command in v.setup:
                key = tuple(map(str, command))
                if key not in setup_done:
                    info['setup'].append(execute(command, env=environment(v), timeout=args.timeout))
                    setup_done.add(key)
            for _ in range(args.build_samples if v.builds else 0):
                info['builds'].append([execute(command, env=environment(v), timeout=args.timeout) for command in v.builds])
            if v.artifacts:
                info['artifact_bytes'] = artifact_bytes(v.artifacts)
                info['artifact_sha256'] = {str(p): digest(p) for path in v.artifacts for p in ([path] if path.is_file() else path.rglob('*')) if p.is_file()}
            info['build_stats'] = stats([sum(step['wall_ns'] for step in sample) for sample in info['builds']])
            info['status'] = 'ready'
            runnable.append(v)
            print(f'READY {v.id}', flush=True)
        except (BenchError, OSError) as error:
            info.update(status='failed', reason=str(error))
            failures += 1
            print(f'FAILED {v.id}: {error}', flush=True)
        save()
    jobs = []
    for v in runnable:
        for name, spec in data['workloads'].items():
            row = dict(variant=v.id, workload=name, status='pending', fresh=[], warm=None, validation=[])
            data['results'].append(row)
            if name not in v.workloads:
                row.update(status='unsupported', reason='No comparable standard task/channel implementation in this port.')
            else:
                jobs.append((v, name, spec['n'], row))
    rng = random.Random(args.order_seed)
    # Validate two additional small inputs/seeds before ranking a port. Includes
    # singleton and snapshot/batch boundaries; both warmup and measured lines checked.
    rng.shuffle(jobs)
    for v, name, n, row in jobs:
        try:
            for verify_n, verify_seed in ((1, 0), (65, 991)):
                row['validation'].append(measure(v, name, verify_n, verify_seed, 0, 1, args.timeout))
            row['status'] = 'validated'
        except (BenchError, OSError) as error:
            row.update(status='failed', reason=str(error))
            failures += 1
            print(f'FAILED oracle {v.id}/{name}: {error}', flush=True)
        save()
    for trial in range(args.fresh):
        rng.shuffle(jobs)
        for v, name, n, row in jobs:
            if row['status'] == 'failed':
                continue
            try:
                row['fresh'].append(measure(v, name, n, args.seed, 0, 1, args.timeout))
            except (BenchError, OSError) as error:
                row.update(status='failed', reason=str(error))
                failures += 1
            save()
        print(f'Fresh-process round {trial + 1}/{args.fresh} complete', flush=True)
    rng.shuffle(jobs)
    for v, name, n, row in jobs:
        if row['status'] == 'failed':
            continue
        try:
            row['warm'] = measure(v, name, n, args.seed, args.warmups, args.samples, args.timeout)
            row['wall_stats'] = stats([r['wall_ns'] for r in row['fresh']])
            row['first_stats'] = stats([r['samples'][0]['elapsed_ns'] for r in row['fresh']])
            row['warm_stats'] = stats([s['elapsed_ns'] for s in row['warm']['samples'][args.warmups:]])
            row['status'] = 'ok'
            print(f'OK {v.id}/{name}: warm {row["warm_stats"]["median_ns"] / 1e6:.3f} ms', flush=True)
        except (BenchError, OSError) as error:
            row.update(status='failed', reason=str(error))
            failures += 1
            print(f'FAILED {v.id}/{name}: {error}', flush=True)
        save()
    data['metadata']['completed_utc'] = datetime.now(timezone.utc).isoformat()
    data['metadata']['load_average_end'] = os.getloadavg() if hasattr(os, 'getloadavg') else None
    changed = [p for p, sha in data['metadata']['source_sha256'].items() if not (ROOT / p).is_file() or digest(ROOT / p) != sha]
    data['metadata']['changed_sources_during_run'] = changed
    if changed:
        failures += 1
        print(f'FAILURE: sources changed during measurement: {changed}', flush=True)
        for row in data['results']:
            if row['status'] == 'ok':
                row.update(status='failed', reason='Source files changed during measurement; raw samples retained, ranking invalidated.')
    data['summary'] = dict(failures=failures, ok=sum(r['status'] == 'ok' for r in data['results']), missing=sum(v['status'] == 'missing' for v in data['variants']))
    save()
    print(f'Report: {args.out.with_suffix(".html")}\n{data["summary"]}', flush=True)
    return 1 if failures or (args.require_all and data['summary']['missing']) else 0


if __name__ == '__main__':
    raise SystemExit(main())
