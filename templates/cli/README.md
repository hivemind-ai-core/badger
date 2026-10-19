# @name@

A command-line program written in Badger.

The options are a record type (`std.cli`): every field is an option,
spelled in kebab case (`--dry-run` for `dry_run`), and `args` collects the
positional arguments. `cli.parse_env_program` parses the program's own
arguments into it, taking the values that are not given from the defaults.

```sh
badger run . -- --name Ada --count 2 --verbose
```

```sh
badger check    # check the program
badger test     # run its tests
```

Next: replace `Options` with the options the program really takes, and
`main` with what it does with them.
