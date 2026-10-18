# Evaluating changes for LLM authors

These tasks measure whether a change to the language, its diagnostics, its
libraries, or its tools helps a model write working Badger. Each task in
`tasks/NAME` has:

| Path | What it is |
|---|---|
| `task.md` | The instructions the agent is given |
| `start/` | The project the agent starts from |
| `accept/` | Acceptance tests the agent does not see |
| `reference/` | A known solution (files over `start/`): proof the task is solvable |

`run.py` scores solutions. It copies `start/`, applies the solution's
files, and records whether the project checks (and how many diagnostics it
has), its own tests, and the acceptance tests (`accept/src/accept.bdg`
added to the project). A task is solved when the project checks and every
acceptance test passes.

```sh
python3 eval/run.py --reference          # every reference solves its task (CI runs this)
python3 eval/run.py --start              # the unchanged projects (all unsolved)
python3 eval/run.py --candidates runs/2026-10-18-a --json
```

## Running an agent

For each task, give the agent `task.md` and a fresh copy of `start/` with
the toolchain under test (`badger` on the path, and `badger mcp` if the
run measures the MCP tools), but not `accept/` or `reference/`. Use the
same model, prompt, and budget (time, tool calls, or tokens) for every
configuration you compare. When the agent stops, copy the project's
changed and added files into `CANDIDATES/NAME/` (paths relative to the
project) and run `run.py --candidates CANDIDATES`.

Record per run, beside `run.py`'s JSON: the toolchain commit, the model,
the budget, and the agent's own counts (tool calls, `badger check` runs,
tokens, wall time). Compare configurations by solve rate over repeated
runs (at least five per task, as results vary), then by diagnostics left,
lines changed, and the agent's effort. A change that helps must not lower
the solve rate on the other tasks.

## Adding a task

Keep `start/` small and self-contained, write `task.md` as a user would
(what, not how), put every acceptance test in `accept/src/accept.bdg`
(they may use only what `task.md` names), and add a `reference/`; then
`python3 eval/run.py --reference --task NAME` must report it solved and
`--start --task NAME` unsolved.
