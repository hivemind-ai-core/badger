# @name@

A background worker written in Badger.

- `src/worker.bdg` — the job kind (`stdx.jobs`): its state, the step that
  does the work, and the supervised worker that runs it.
- `src/main.bdg` — the entry: it submits a job and waits for it to be
  done.

```sh
badger check    # check the program
badger run      # submit a job and wait for it
badger test     # run its tests
```

Next: replace `Job` and `step` with the job's own state and work, and
point the job store at a file (`jobs.store({ path: "jobs.db" })`) so the
jobs survive a restart.
