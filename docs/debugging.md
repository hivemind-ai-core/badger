# Debugging and profiling

## Debugging with gdb (or any DWARF debugger)

`lang run --debug` registers debug information for the generated code
through the GDB JIT interface: source line tables, one function per
definition, every named local and parameter (kept in a stack slot, so it is
readable anywhere in its function and can be watched), and unwind tables,
so backtraces pass from generated frames into the runtime.

```sh
gdb --args lang run --debug app.lang
(gdb) set breakpoint pending on       # the code exists once compiled
(gdb) break app.lang:12               # by file and line
(gdb) break app.lang:20 if total > 100
(gdb) run
(gdb) bt                              # app.area#1 (w=2, h=10) at app.lang:5 ...
(gdb) info args
(gdb) info locals
(gdb) next / step / finish            # by source line; into and out of calls
(gdb) watch total                     # a hardware watchpoint on a local
(gdb) continue
```

Functions appear under their instance names (`app.area#1`, `app.main`,
`app.main.lambda42`). Values print by representation: integers, floats,
booleans as bytes, and managed values (strings, records, lists) as
addresses (`ref`). Code compiled with `--release` keeps line tables, but
optimized values may not match their source variables between stores.

### VS Code

With the C/C++ extension's GDB support, a launch configuration:

```json
{
  "name": "lang: debug",
  "type": "cppdbg",
  "request": "launch",
  "program": "/path/to/lang",
  "args": ["run", "--debug", "${file}"],
  "cwd": "${workspaceFolder}",
  "MIMode": "gdb",
  "setupCommands": [{ "text": "set breakpoint pending on" }]
}
```

Breakpoints set in `.lang` files in the editor then bind once the program
is compiled.

## Profiling

`LANG_PROFILE=out.folded lang run --release app.lang` samples CPU time at
1 kHz and writes collapsed stacks (`outer;...;leaf count`) for flamegraph
tools (`inferno-flamegraph < out.folded > out.svg`, or speedscope), and
prints the hottest functions by self and total time. It covers compilation
too. Generated functions keep frame pointers; build the toolchain with
`RUSTFLAGS="-C force-frame-pointers=yes"` to also see who calls runtime
functions.

`LANG_PERF_MAP=1` writes `/tmp/perf-PID.map` so `perf record` / `perf
report` name generated functions.

## From agents (MCP)

The `run` tool of `lang mcp` takes `breakpoints` (`file:line`, optionally
`if CONDITION`) and reports each stop's frames, arguments, and locals (or
the gdb `commands` given), up to `max_stops`; and `profile: true` adds the
CPU profile summary. See [agents.md](agents.md).
