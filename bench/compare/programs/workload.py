"""Ordinary Python objects, dict snapshots, and bounded threaded queues."""
import queue
import sys
import threading
import time


def cpu(n, seed):
    total = 0
    for i in range(n):
        x = (i + seed) % 1000003
        for _ in range(8):
            x = (x * 17 + 23) % 1000003
        total += x
    return total


def records(n, seed):
    original = [dict(id=i, value=(i + seed) % 1000, active=i % 3 != 0) for i in range(n)]
    enriched = [dict(e, value=e['value'] * 3 + 7) for e in original]
    # Read both versions after enrichment; input records must remain unchanged.
    return sum(e['value'] for e in enriched if e['active']) + sum(e['value'] for e in original)


def snapshots(n, seed):
    current = {}
    versions = []
    for i in range(n):
        current[i % 256] = i + seed
        if (i + 1) % 64 == 0:
            versions.append(current.copy())
    versions.append(current.copy())
    return sum((k + 1) * v for version in versions for k, v in version.items())


def handoff(n, seed, batch):
    # At most 256 queued elements: 256 scalar messages or four 64-item lists.
    q = queue.Queue(maxsize=256 // batch)

    def produce():
        for start in range(0, n, batch):
            if batch == 1:
                q.put(start + seed)
            else:
                q.put([i + seed for i in range(start, min(start + batch, n))])
        q.put(None)

    producer = threading.Thread(target=produce)
    producer.start()
    total = 0
    while True:
        item = q.get()
        if item is None:
            break
        total += item if batch == 1 else sum(item)
    producer.join()
    return total


def run(name, n, seed):
    if name in ('handoff', 'batches'):
        return handoff(n, seed, 1 if name == 'handoff' else 64)
    return {'cpu': cpu, 'records': records, 'snapshots': snapshots}[name](n, seed)


if __name__ == '__main__':
    name, n, seed, warmups, repeats = sys.argv[1:]
    n, seed, warmups, repeats = map(int, (n, seed, warmups, repeats))
    for iteration in range(warmups + repeats):
        started = time.perf_counter_ns()
        checksum = run(name, n, seed)
        elapsed = time.perf_counter_ns() - started
        # Warmup output is also checked by the runner.
        print('BENCH', iteration, elapsed, checksum)
