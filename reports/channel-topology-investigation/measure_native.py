"""Run queue-only probes sequentially, after language measurements.

Build with:
cargo build --manifest-path reports/channel-topology-investigation/Cargo.toml --release --offline --target-dir /tmp/lang-channel-topology-target
"""
from pathlib import Path
import hashlib
import json
import statistics
import subprocess

HERE = Path(__file__).resolve().parent
BINARY = Path('/tmp/lang-channel-topology-target/release/channel-topology-probe')


def main():
    result = {
        "method": "Queue-only u64 throughput; 2000000 transfers, exact in-order checks and checksum; one warmup plus three measured fresh processes. Producer pinned to CPU 1 and consumer to CPU 2 (different physical cores here). Preallocated capacity, busy-spin when unavailable. No GC, task scheduler, close, byte budgets or selection. Batch is maximum items per commit, not a requirement to fill it.",
        "binary_sha256": hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        "source_sha256": hashlib.sha256((HERE / 'src/main.rs').read_bytes()).hexdigest(),
        "crossbeam_queue": "0.3.14",
        "runs": [], "summary": [], "smoke": [],
    }
    for capacity in (1, 2, 8):
        for mode, batch in [('lock', 1), ('spsc', 1), ('spsc', 32), ('lock', 32), ('mpmc', 1)]:
            cmd = [str(BINARY), mode, '100003', str(capacity), str(batch)]
            p = subprocess.run(cmd, text=True, capture_output=True, timeout=20, check=True)
            result['smoke'].append(json.loads(p.stdout))
    for capacity in (64, 1024):
        for mode, batch in [('lock', 1), ('mpmc', 1), ('spsc', 1), ('lock', 32), ('spsc', 32)]:
            rows = []
            for repeat in range(4):
                cmd = [str(BINARY), mode, '2000000', str(capacity), str(batch)]
                p = subprocess.run(cmd, text=True, capture_output=True, timeout=30, check=True)
                row = json.loads(p.stdout)
                result['runs'].append({**row, 'cmd': cmd, 'warmup': repeat == 0})
                if repeat:
                    rows.append(row['ns_per_message'])
            summary = {'mode': mode, 'batch': batch, 'capacity': capacity, 'median_ns': statistics.median(rows), 'min_ns': min(rows), 'max_ns': max(rows)}
            result['summary'].append(summary)
            print(json.dumps(summary), flush=True)
            (HERE / 'native.json').write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
