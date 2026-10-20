# Testing and contracts

Tests are `test_*` functions run by `badger test`; `std.test` adds assertions, a controlled clock, seeded scheduling, and property checks.
Contracts (`requires`, `ensures`) state conditions the compiler proves, refutes, or checks at run time, and `badger run --record` captures a run for offline replay.

## Write and run tests

```badger
import std.test

fn add(a: Int, b: Int) -> Int { a + b }

fn parse_port(text: String) -> Result<Int, String> {
    match text.parse_int() {
        .some(n) => if n > 0 && n < 65536 { ok(n) } else { err($"out of range: {n}") },
        .none => err($"not a number: {text}"),
    }
}

fn test_add() {
    test.assert_eq(add(2, 3), 5)
}

fn test_port_errors() {
    let e = test.expect_err(parse_port("99999"))
    test.assert_eq(e, "out of range: 99999")
}

fn test_port_ok() -> Result<Unit, String> {
    let port = parse_port("8080")?
    test.assert_true(port == 8080, "parsed 8080")
    ok(())
}

fn test_broken() {
    test.assert_eq(add(1, 1), 3)
}
```

```text
$ badger test ports.bdg
test test_add ... ok
test test_port_errors ... ok
test test_port_ok ... ok
test test_broken ... FAILED (assertion failed: expected 3, found 2 at ports.bdg:28:5)

test result: FAILED. 3 passed; 1 failed
```

- A test is a function whose name starts with `test_`, with no parameters.
  It returns `Unit` or a `Result`; returning `err` fails it.
- Tests live next to the code they test and may call private functions.
- `badger test` with no path tests the current project, including the
  project's modules the entry imports (reported as `util.test_name`), but
  not dependencies.
- `badger test --filter FILTER` runs only the tests whose names contain
  `FILTER`. `badger test ports.bdg --filter port` runs the two port
  tests.
- `badger test --json` prints the results as one JSON object.
- The exit status is 0 when every test passed and 1 otherwise.

| How a test fails | Reported as |
|---|---|
| `test.assert_eq(a, b)` | `assertion failed: expected B, found A at FILE:LINE:COL` |
| `test.assert_true(c, message)` | `assertion failed: MESSAGE at ...` |
| `test.fail(message)` | `MESSAGE at ...` |
| `assert(c)` (prelude) | `assertion failed at ...` |
| `panic(message)` | `MESSAGE at ...` |
| Returning `err(e)` | `returned an error: E` (the error displayed) |

Assertion failures report the caller's location, not the helper's.

## Controlled time, scheduling, and properties

```badger
import std.random
import std.task
import std.test
import std.time

fn retry_delay(attempt: Int) -> Duration {
    time.millis(100 * attempt)
}

fn test_sleep_uses_virtual_time() {
    test.manual_clock(0)
    let start = time.now()
    task.sleep(retry_delay(30))
    test.assert_eq(time.as_millis(time.now() - start), 3000)
}

fn clamp(x: Int) -> Int {
    if x < 0 { 0 } else { x }
}

fn test_clamp_never_negative() {
    test.property(100, 1, fn(r: random.Random) -> Int { r.int(-50, 50) }, fn(x: Int) -> Result<Unit, String> {
        if clamp(x) >= 0 { ok(()) } else { err("negative") }
    })
}

fn test_counter_under_many_schedules() {
    test.explore(20, fn() -> Result<Unit, String> {
        let hits = Atom.new(0)
        task.scope(fn(s) -> Result<Unit, String> {
            let a = s.spawn(fn() { let _ = hits.update(fn(n) { n + 1 }) })
            let b = s.spawn(fn() { let _ = hits.update(fn(n) { n + 1 }) })
            a.join()
            b.join()
            ok(())
        })?
        if hits.get() == 2 { ok(()) } else { err($"lost an update: {hits.get()}") }
    })
}
```

```text
test test_sleep_uses_virtual_time ... ok
test test_clamp_never_negative ... ok
test test_counter_under_many_schedules ... ok

test result: ok. 3 passed; 0 failed
```

The three-second sleep completes at once: under `manual_clock`, sleeps run
on virtual time.

| `std.test` helper | What it does |
|---|---|
| `manual_clock(start_nanos)` | Switch `time.now()` to a controlled clock. Sleeps complete by virtual time, advanced by `advance` or automatically once nothing else can run |
| `advance(d)` | Move the controlled clock forward, waking sleepers |
| `real_clock()` | Return to the real clock. Each test starts on the real clock anyway |
| `schedule_seed(seed)` | Pick among ready tasks pseudo-randomly from `seed` (one worker), so an interleaving is reproducible. `BADGER_SCHED_SEED` sets it for every test |
| `record_schedule()`, `recorded_schedule()`, `replay_schedule(decisions)` | Record the scheduling decisions of a run and replay them exactly |
| `fifo_schedule()` | Return to first-in first-out scheduling |
| `explore(runs, body)` | Run `body` under seeds `1..=runs`; a failure reports the seed and its decisions |
| `property(runs, seed, gen, check)` | Run `check` on `runs` values made by `gen` from seeded random sources; a failure reports the value and its seed |
| `assert_eq`, `assert_true`, `expect_err`, `fail` | Assertions (above) |

Failures from `explore` and `property` look like this:

```text
test test_explore_reports_failing_seed ... FAILED (failed under schedule seed 1 (decisions [1, 1, 0, 0]): b ran first: ["b1", "b2", "a1", "a2"] at tests/testing/simulation.bdg:62:5)
test test_property_reports_counterexample ... FAILED (property failed for 95 (run 2, seed 3): too large at tests/testing/simulation.bdg:75:5)
```

Seeded randomness outside tests comes from `random.seeded(seed)`
([std.random](../api/random.md)). To swap a system's provider in a test,
use `system.replace` ([chapter 8](08-systems-and-pipelines.md#select-and-replace-nodes)).
To test an HTTP handler without a socket, use `http.call`
([chapter 10](10-services.md#http-server)).

## Contracts

```badger
import std.io

requires amount > 0, "withdraw a positive amount"
requires amount <= balance, "cannot overdraw"
ensures result >= 0
fn withdraw(balance: Int, amount: Int) -> Int {
    balance - amount
}

pub fn main() -> Result<Unit, io.Error> {
    io.println($"{withdraw(100, 30)}")?
    ok(())
}
```

```text
70
```

- `requires COND` is checked when the function is entered.
- `ensures COND` is checked at every return: the end of the body,
  `return`, and `?` alike. `result` names the value being returned.
- Conditions are `Bool` expressions over the parameters (and `result`).
  The optional string explains the clause. A condition may be a bound
  (`requires name.len() <= 40`) or call a predicate function; one whose
  body is a single expression (`fn short(s: String) -> Bool => s.len()
  <= 40`) is evaluated at each call, so `requires short(name)` is proven
  or refuted like the bound it states.
- Clauses go on the lines before a function or method with a body. They
  cannot `return` or use `?` (E0140), and a protocol method's signature
  cannot carry them; put them on each implementation.
- Contracts are checked in every build, like overflow checks.

A failed clause is a defect. A failed precondition is reported at the call
site, since the caller broke it:

```badger
import std.env
import std.io

requires amount > 0, "withdraw a positive amount"
fn withdraw(balance: Int, amount: Int) -> Int {
    balance - amount
}

pub fn main() -> Result<Unit, io.Error> {
    let amount = env.args().get(0).unwrap_or("0").parse_int().unwrap_or(0)
    io.println($"{withdraw(100, amount)}")?
    ok(())
}
```

```text
$ badger run bank.bdg -- -5
panic: precondition of `withdraw` failed: amount > 0 (withdraw a positive amount); contract at bank.bdg:4:1, violated by the call at bank.bdg:11:19
```

A failed postcondition is reported at the clause, since the function broke
it.

### What the compiler decides

```text
$ badger contracts withdraw.bdg
withdraw.bdg:3: withdraw requires `amount > 0`: proven (holds at all 1 call(s))
withdraw.bdg:4: withdraw requires `amount <= balance`: proven (holds at all 1 call(s))
withdraw.bdg:5: withdraw ensures `result >= 0`: runtime (not proven at every return)
3 clause(s): 2 proven, 1 checked at run time, 0 refuted
```

| Status | Meaning |
|---|---|
| `proven` | The check is compiled out |
| `runtime` | The check runs; the reason says why it was not proven |
| `refuted` | The compiler showed it cannot hold: E0145 or E0146 |

The analysis tracks integer ranges, booleans, texts, and list lengths. It
learns from constants, immutable bindings, enclosing `if` conditions and
early returns, the function's own preconditions, and the postconditions of
the functions it calls. It does not relate two variables to each other, so
`result >= 0` above is checked at run time even though the preconditions
imply it.

A precondition is compiled out only when it is proven at every call and
the function is only called directly (never used as a value). `badger
contracts --json` gives the same report to tools.

| Code | Meaning | Severity |
|---|---|---|
| E0145 | A call's arguments cannot satisfy the callee's `requires` | Error when every run reaches the call; warning when enclosing conditions decide it |
| E0146 | A returned value cannot satisfy the function's `ensures` | Error when every run returns it; warning when enclosing conditions decide it |

```badger fragment
requires amount > 0, "withdraw a positive amount"
fn withdraw(balance: Int, amount: Int) -> Int { balance - amount }

pub fn main() -> Unit {
    let _ = withdraw(100, 0)   // E0145: this call breaks the precondition `amount > 0`
}
```

### Contracts on providers and stages

`requires` and `ensures` lines also go before a `provider` or a `stage`
([chapter 8](08-systems-and-pipelines.md)):

- A provider's clauses are its initializer's: `config` and `scope` are its
  parameters and `result` its `Result`. They run when the node starts, and
  the compiler checks each `system { }` node's configuration against them
  (E0145 for one that cannot satisfy a precondition).
- A stage's clauses are its factory's: they constrain the factory's
  parameters, as for any function call.

```badger fragment
requires config.size > 0, "a pool needs at least one slot"
provider pool(config: { size: Int }) -> Result<Pool, AppError> { ... }

let plan = system { pool: pool({ size: 0 }) }   // E0145: the node's configuration cannot satisfy it
```

A contract states what the program guarantees; a broken one is a defect.
Configuration read from outside (files, the environment) is validated with
an ordinary check that returns an error.

### Contracts and tests together

`badger contracts --tests PATH` runs the tests while observing the
program. Each clause reports how often the tests evaluated it, and
integer values that stayed within a bound in every observed call become
candidate clauses. From `tests/contracts/inventory.bdg`:

```text
tests/contracts/inventory.bdg:5: restock requires `qty > 0`: proven (holds at all 3 call(s)); held all 3 time(s) the tests evaluated it
tests/contracts/inventory.bdg:16: reorder_after requires `days > 0`: runtime (no direct calls (an entry point, or called from tests)); never exercised by a test
tests/contracts/inventory.bdg:10: take could state `ensures result >= 0` (result was 0..=7 in 3 call(s))
```

A clause no test reaches is a gap. Candidates are suggestions drawn from
the tests' data: accept, sharpen, or refute them with another test.

## Record and replay

```badger
import std.env
import std.fs
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let name = env.get("USER").unwrap_or("nobody")
    let text = fs.read_text("notes.txt")?
    io.println($"{name}: {text.trim()}")?
    ok(())
}
```

```text
$ USER=ada badger run --record run.bundle report.bdg
ada: first draft
recorded 2 operations to run.bundle

$ rm notes.txt
$ badger replay run.bundle
ada: first draft
replay: 2 recorded operation(s) answered from the bundle
replay: reproduced the recorded outcome (exit 0)
```

`badger run --record FILE` writes a reproduction bundle (JSON). It holds:

- the program's identity: toolchain version and executable digest, each
  source file's digest, arguments, and a digest of the configuration;
- the outcome of every external operation: HTTP requests, subprocesses,
  file reads, writes, and listings, environment variables, both clocks,
  and OS-seeded random sources;
- the program's output and how it ended (exit code, failure, failing line).

A recorded run uses one worker. Environment variables whose names look
like credentials (`TOKEN`, `SECRET`, `PASSWORD`, `API_KEY`, `AUTH`, ...)
and credential-like URL query values are not stored.

`badger replay FILE` answers every operation from the bundle: nothing is
requested, run, or written, so the replay above works after `notes.txt`
is gone.

| `badger replay` exit status | Meaning |
|---|---|
| 0 | The recorded outcome (exit code, output, failure) was reproduced |
| 1 | The outcome differs, or the program changed (E0016) |
| 3 | The program asked for an operation the bundle does not hold, sent different content, or needed a redacted value, a database, or an HTTP server |

When the source changed, plain replay refuses with E0016. `--compare`
runs the changed program against the recording, reports what differs, and
exits with the program's own status:

```text
$ badger replay --compare run.bundle
comparison: report.bdg changed
ada wrote: first draft
comparison: 2 recorded operation(s) answered from the bundle
comparison: the outcome differs from the recording
  output differs from line 1
```

Operations match by identity, in recorded order per identity, so tasks may
interleave differently on replay. A bundle reproduces the program's
computation from recorded inputs, not the current state of the outside
world.

Next: [Services](10-services.md).
