# @name@

A library package written in Badger.

A library has no `main`: it is used by other packages, which import it
and call its functions. `badger test` runs the `test_*` functions it
holds. (`badger run` refuses a library package: it has no `main` to
run.)

```sh
badger check    # check the library
badger test     # run its tests
```

Next: replace the functions with the ones the library really offers, and
publish it (a `badger.toml` with the version and the dependencies it
needs).
