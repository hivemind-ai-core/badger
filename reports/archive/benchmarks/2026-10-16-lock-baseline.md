# Lock baseline (slice 0 of LOCK_OPTIMIZATION.md)

Dated 16 October 2026, revision after `7c0d4be` with lock attribution
(`LANG_RUNTIME_STATS=1`). Benchmarks: `2026-10-16-lock-before.{md,json}`
(`bench/baseline.py`, same machine and method as the earlier snapshots).

`LANG_RUNTIME_STATS=1` now attributes every runtime-lock acquisition: to
the runtime function generated code called (its name), or to an internal
reason (channel or batch slow path, return from a compiled callback, end
of a foreign call, collection, defect, scheduler, startup, shared
constant), with hold time sampled on every 64th acquisition (scaled; a
hold includes time a task spends parked while it logically holds the lock).
Counters are per worker and cache-line separated, and cost nothing when
statistics are off.

## Scaling before the lock work (`--bench` run time, ms)

| Program | 1 worker | 2 workers | 4 workers | 8 workers |
|---|---|---|---|---|
| collections | 158.3 | 186.7 | 182.6 | 187.9 |
| messages | 25.6 | 28.5 | 30.6 | 32.9 |
| runtime | 21.4 | 22.5 | 23.4 | 25.0 |
| independent_channels | 10.5 | 7.9 | 7.6 | 10.2 |
| parallel_alloc | 33.8 | 38.5 | 48.9 | 50.5 |
| parallel (CPU only) | 656.5 | 332.2 | 169.2 | 167.2 |

## Where the lock is taken (4 workers, top reasons)

### collections (4 workers)
```text
lock: 2001593 fast, 6 after spinning, 3 after parking (7.852 ms parked); tasks run: 1; idle waits: 30; idle wakeups: 0
2000000 x lang_rt_i_buffer_push                    ~78.490 ms held
1520 x lang_rt_poll                             ~0.216 ms held
30 x scheduler (park, wake, idle)             ~0.296 ms held
14 x lang_rt_i_time_now                       ~0.024 ms held
14 x lang_rt_i_int_to_string                  ~0.052 ms held
11 x collection                               ~16.250 ms held
7 x lang_rt_i_write_out                      ~0.406 ms held
4 x worker startup                           ~6.059 ms held
```
### messages (4 workers)
```text
lock: 485942 fast, 2231 after spinning, 9 after parking (1.133 ms parked); tasks run: 41715; idle waits: 85; idle wakeups: 83
200782 x lang_rt_i_batch_len                      ~5.324 ms held
200000 x lang_rt_i_batch_get                      ~5.788 ms held
83100 x channel slow path                        ~27.712 ms held
1739 x batch slow path                          ~0.392 ms held
783 x lang_rt_i_batch_recv                     ~0.344 ms held
782 x lang_rt_i_iter_close                     ~0.050 ms held
782 x lang_rt_i_batch_release                  ~0.279 ms held
85 x scheduler (park, wake, idle)             ~0.522 ms held
```
### runtime (4 workers)
```text
lock: 2980 fast, 1 after spinning, 6 after parking (1.784 ms parked); tasks run: 406; idle waits: 6; idle wakeups: 2
2501 x lang_rt_poll                             ~0.213 ms held
201 x lang_rt_i_task_spawn                     ~0.168 ms held
200 x channel slow path                        ~2.816 ms held
22 x lang_rt_i_int_to_string                  ~0.076 ms held
10 x lang_rt_i_atom_get                       ~0.017 ms held
9 x lang_rt_i_time_now                       ~0.035 ms held
8 x lang_rt_i_type_id                        ~0.005 ms held
8 x lang_rt_i_atom_cas                       ~0.015 ms held
```
### independent_channels (4 workers)
```text
lock: 2926 fast, 3823 after spinning, 16 after parking (1.769 ms parked); tasks run: 1510; idle waits: 19; idle wakeups: 17
6691 x channel slow path                        ~5.513 ms held
19 x scheduler (park, wake, idle)             ~0.253 ms held
9 x return from compiled callback            ~0.210 ms held
8 x lang_rt_i_task_join                      ~4.066 ms held
8 x lang_rt_i_task_spawn                     ~0.575 ms held
5 x lang_rt_i_scope_open                     ~0.137 ms held
5 x lang_rt_i_scope_close                    ~0.165 ms held
4 x worker startup                           ~12.477 ms held
```
### parallel_alloc (4 workers)
```text
lock: 266 fast, 1 after spinning, 15 after parking (12.266 ms parked); tasks run: 7; idle waits: 5; idle wakeups: 2
236 x lang_rt_poll                             ~0.143 ms held
14 x collection                               ~63.816 ms held
5 x return from compiled callback            ~1.059 ms held
5 x scheduler (park, wake, idle)             ~0.126 ms held
4 x worker startup                           ~16.270 ms held
4 x lang_rt_i_task_join                      ~5.418 ms held
4 x lang_rt_i_task_spawn                     ~0.053 ms held
3 x lang_rt_i_range_field                    ~0.010 ms held
```
### heap (4 workers)
```text
lock: 41123 fast, 43 after spinning, 9 after parking (0.563 ms parked); tasks run: 20208; idle waits: 26; idle wakeups: 9
20100 x lang_rt_i_task_spawn                     ~6.342 ms held
20001 x return from compiled callback            ~3.498 ms held
903 x lang_rt_poll                             ~0.267 ms held
100 x channel slow path                        ~0.458 ms held
27 x scheduler (park, wake, idle)             ~0.531 ms held
15 x collection                               ~69.350 ms held
7 x lang_rt_i_time_now                       ~0.024 ms held
4 x worker startup                           ~7.857 ms held
```

## Reading

- `collections`: two million `buffer_push` calls each take the lock,
  although a buffer is affine (owned by one task).
- `messages`: reading a leased batch takes the lock per element
  (`batch_len`, `batch_get`: 400,000 of 486,000 acquisitions); the rest is
  the prompt channel's slow path (a waiter to wake, or a full or empty
  queue) and batch publication.
- `independent_channels`: the channel slow path (waking the partner) is
  nearly every acquisition, and most acquisitions spin first: four
  independent pairs contend on one lock.
- `parallel_alloc` and `heap`: collections (stop the world) dominate hold
  time; the rest is polls.
- `runtime`: polls (yields), spawns, and channel waits.
