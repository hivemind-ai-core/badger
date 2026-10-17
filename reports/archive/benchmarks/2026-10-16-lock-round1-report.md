# Lock optimization, round 1

Dated 16 October 2026. Work from `LOCK_OPTIMIZATION.md`, measured with
`bench/baseline.py` on the same machine as the baseline
(`2026-10-16-lock-before`, revision `7c0d4be`) against
`2026-10-16-lock-round1`.

## Results (`--bench` run time, ms)

| Program | Workers | Before | After |
|---|---|---|---|
| parallel_alloc | 1 / 2 / 4 / 8 | 33.8 / 38.5 / 48.9 / 50.5 | 33.7 / 28.6 / 25.4 / 19.1 |
| independent_channels | 1 / 2 / 4 / 8 | 10.5 / 7.9 / 7.6 / 10.2 | 10.5 / 7.6 / 5.7 / 6.3 |
| messages | 1 / 2 / 4 / 8 | 25.6 / 28.5 / 30.6 / 32.9 | 24.8 / 22.3 / 24.9 / 26.9 |
| collections | 1 / 2 / 4 / 8 | 158.3 / 186.7 / 182.6 / 187.9 | 160.3 / 162.5 / 161.8 / 163.7 |
| runtime | 1 / 2 / 4 / 8 | 21.4 / 22.5 / 23.4 / 25.0 | 21.0 / 22.4 / 23.3 / 23.5 |
| records, heap, parallel | all | unchanged | unchanged (controls) |

Nothing slows down with more workers any more except `messages` at 8
(its round-trip half pins both tasks to one worker) and `runtime`
(spawn-heavy, still under the global lock); one-worker times are
unchanged. Short benchmarks vary by about 10% between runs; the channel
decisions below used medians of 15 runs.

## Changes

1. **Attribution** (slice 0): `LANG_RUNTIME_STATS=1` names the reason for
   every global-lock acquisition (the runtime function generated code
   called, or an internal reason) with sampled hold time.
2. **Task-owned data without the lock**: buffers (affine) and reads of a
   held batch lease ran under the lock per element: 2,000,000 acquisitions
   in `collections`, 400,000 in `messages`.
3. **Allocation pressure published in blocks** (slice 7): every
   allocation updated one shared counter; each worker now publishes 64 KiB
   at a time. This is the `parallel_alloc` gain.
4. **Wait generations** (slice 1): two soundness bugs found on the way, now
   fixed: a stale registration of a cancelled wait could deliver a value
   to a later wait on another channel (type confusion), and a send to a
   cancelled receiver during its cleanup was lost.
5. **Lock-free handoff** (slices 3 and 6, first part): a send to a parked
   receiver and a receive that admits a parked sender finish on the fast
   path; the ready queue and parked-to-ready transitions have a short lock
   of their own. Global-lock acquisitions: `messages` 486,000 -> 46,000,
   `independent_channels` 6,700 -> 1,700.

Tried and rejected: spinning before parking on an empty or full channel
(10x slower round trips when both partners share a worker).

## Remaining global-lock users (4 workers, after)

### messages (4 workers)
```text
lock: 45507 fast, 737 after spinning, 4 after parking (0.622 ms parked); tasks run: 41656; idle waits: 48; idle wakeups: 46
41556 x channel slow path                        ~13.504 ms held
2144 x batch slow path                          ~0.628 ms held
787 x lang_rt_i_batch_recv                     ~0.945 ms held
786 x lang_rt_i_iter_close                     ~0.088 ms held
786 x lang_rt_i_batch_release                  ~0.334 ms held
```
### independent_channels (4 workers)
```text
lock: 1699 fast, 3188 after spinning, 11 after parking (1.186 ms parked); tasks run: 1501; idle waits: 6; idle wakeups: 4
4837 x channel slow path                        ~20.090 ms held
9 x return from compiled callback            ~0.146 ms held
8 x lang_rt_i_task_join                      ~10.215 ms held
8 x lang_rt_i_task_spawn                     ~0.047 ms held
6 x scheduler (park, wake, idle)             ~0.131 ms held
```
### runtime (4 workers)
```text
lock: 2977 fast, 6 after spinning, 5 after parking (5.542 ms parked); tasks run: 406; idle waits: 7; idle wakeups: 5
2501 x lang_rt_poll                             ~0.445 ms held
201 x lang_rt_i_task_spawn                     ~0.323 ms held
200 x channel slow path                        ~4.604 ms held
22 x lang_rt_i_int_to_string                  ~0.097 ms held
10 x lang_rt_i_atom_get                       ~0.017 ms held
```
### parallel_alloc (4 workers)
```text
lock: 267 fast, 4 after spinning, 14 after parking (12.152 ms parked); tasks run: 8; idle waits: 6; idle wakeups: 3
236 x lang_rt_poll                             ~0.223 ms held
14 x collection                               ~63.508 ms held
6 x scheduler (park, wake, idle)             ~0.091 ms held
5 x return from compiled callback            ~1.279 ms held
4 x worker startup                           ~17.799 ms held
```

- Parking (registering a wait) on an empty or full channel, selection,
  close, batch publication and release, spawn and join, scopes, timers,
  and collection (stop the world) still take the global lock.
- Not done from the plan: GC coordination independent of the lock (slice
  2), per-channel guards for the remaining slow paths and selection
  (slices 3-4 rest), batch credit reservation (slice 5), worker-owned
  ready deques and timers (slice 6 rest), output outside the lock (slice
  7 rest), specialized rings (slice 8).
- With the lock out of the way, the channel benchmarks are now dominated
  by allocating an `Option`/`Result` wrapper per receive (`make_enum`,
  `gc::alloc`: a third of `independent_channels` time), which an internal
  return convention for channel operations would remove.
