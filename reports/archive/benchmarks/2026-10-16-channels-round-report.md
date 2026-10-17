# Channel optimization round (after the topology investigation)

Dated 16 October 2026. Follows `reports/2026-10-16-channel-topology-investigation.md`
and lock round 1. Benchmarks: `2026-10-16-channels-round.{md,json}`;
topology fixtures (`reports/channel-topology-investigation/fixtures`):
medians of 7 fresh runs, `--bench` time in ms.

## Topology fixtures (200,000 messages)

| Use | Capacity | Investigation (1 / 2 / 4 workers) | Now (1 / 2 / 4) |
|---|---|---|---|
| 1:1 | 1 | 21.9 / 29.6 / 28.6 | 14.7 / 18.1 / 17.6 |
| 1:1 | 64 | 11.2 / 11.1 / 11.8 | 4.7 / 5.7 / 5.6 |
| 1:1 | 1024 | 10.4 / 9.7 / 10.3 | 4.1 / 5.1 / 4.9 |
| 1:1, 512-byte budget | 1024 | 11.7 / 15.5 / 15.6 | 5.2 / 6.1 / 6.2 |
| 4:1 | 1024 | 10.6 / 10.7 / 10.5 | 4.2 / 6.6 / 5.3 |
| 1:4 | 1024 | 10.5 / 11.2 / 10.5 | 4.2 / 8.2 / 6.4 |
| 4:4 | 1024 | 10.6 / 19.2 / 23.2 | 4.3 / 8.4 / 10.3 |

Every case is 2-2.5x faster on one worker and 1.6-2.5x faster on four.

## What changed

1. **No lock convoy**: a fast path that met a channel claimed by the
   runtime lock's holder fell back to the lock, and its own claim made all
   others fall back too (4:4: 77,000 acquisitions; now under 1,000).
2. **Byte-budget sends on the fast path** (the size is measured before
   taking the channel).
3. **Fast paths on one worker** (they were limited to several workers).
4. **No result object per receive** (`lang_mir::split`, optimized builds):
   a receive whose `Option`/`TryRecv` is only matched on returns the tag
   and leaves the payload with the task; this was a third of channel time
   (allocation and the collections it caused).

## Reading the remaining multi-worker cost

One-worker times halved, so tasks communicating through one channel from
different workers are now slower than on one worker by the cost of moving
the channel's state between cores (about 4-30 ns per message): 1:1 at
capacity 1024 costs 4.1 ms on one worker and 4.9 ms on four; one channel
shared by eight tasks (4:4) 4.3 and 10.3 ms. This is no longer lock
contention (the global lock is taken a few hundred times per run). The
investigation's remedies apply from here: proven SPSC indices (the native
probe's 3-5 ns against 28-34 ns behind a shared lock word) and keeping
communicating tasks on one worker, both of which need the compiler
topology analysis and scheduler support it describes; capacity-1 streams
pay a park and wake per message.

Not done from the investigation: the topology analysis and specialized
queues, producer-owned batch credits, and a bulk `recv_many`.

## Benchmarks (`--bench` ms, 1 / 2 / 4 / 8 workers)

| Program | Lock baseline | Now |
|---|---|---|
| messages | 25.6 / 28.5 / 30.6 / 32.9 | 15.6 / 16.4 / 19.2 / 20.8 |
| independent_channels | 10.5 / 7.9 / 7.6 / 10.2 | 4.2 / 3.5 / 2.9 / 3.4 |
| parallel_alloc | 33.8 / 38.5 / 48.9 / 50.5 | 33.5 / 29.0 / 26.3 / 20.7 |
| collections | 158.3 / 186.7 / 182.6 / 187.9 | 156.1 / 160.0 / 158.9 / 155.4 |
| runtime | 21.4 / 22.5 / 23.4 / 25.0 | 21.0 / 22.7 / 23.4 / 23.7 |
| records, heap, parallel | unchanged | unchanged |
