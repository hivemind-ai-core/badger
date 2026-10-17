# Releases

`badger release` packages a program to run somewhere else: no source
checkout, no compiler installation, and no network access are needed to
start it.

```sh
badger release examples/design/service -o release/orders   # check, compile, package
release/orders/run ARGS...                                 # start it (arguments go to the program)
badger start release/orders -- ARGS...                       # the same, with an explicit executable
```

## What a release holds

```text
release.json       what is here: name, Badger version, target, native ABI, digests
run                launcher: exec bin/badger start <this directory> -- ARGS
bin/badger           the executable that built the release (runtime and code generator)
app/NAME.bdgmir   the checked program as binary MIR, with its configuration and runtime settings
app/badger.lock      the project's lockfile, when it has one
lib/badger/libM.so   native modules the program calls that are not linked into `badger`
```

The program is saved as `badger compile` saves it (with the MIR
optimizations of `--release`), and starting it only generates code: a
small program starts in tens of milliseconds. Configuration is the
project's at release time (`--profile NAME` merges a profile, `--workers N`
saves a worker count); `BADGER_<SETTING>` environment variables and
`badger start --workers` still apply when it starts.

## What starting checks

Before any program code runs, `badger start` refuses a release whose

- `release.json` has another format, or names another target (`arch`,
  `os`) or another `badger` version (start it with its own `bin/badger`);
- program, lockfile, or native library differs from its recorded SHA-256
  digest;
- native library would load from somewhere other than the release (a
  `BADGER_NATIVE_PATH` directory that shadows it), cannot be loaded, or has
  another native ABI version;
- linked module (`badger_sqlite`, `badger_http`, ...) is missing from the
  executable.

A release built while a native module the program calls cannot be found
or loaded is not written; an earlier release at the same path is replaced
only when the new one is complete.

Data files the program reads at run time (templates, schemas, static
assets) are not copied: deploy them beside the release and point the
program's configuration at them.
