# Project templates

The templates `badger init --template=NAME` offers, one directory each:

| Template | The project it starts |
|---|---|
| `cli` | a program that parses its options (`std.cli`) |
| `service` | a service with routes (`stdx.http`, `stdx.route`) |
| `framework` | the shape of `examples/framework/`: a store, a service, and routes |
| `library` | a library package (`kind = "library"`, `src/lib.bdg`) |
| `worker` | a background worker (`stdx.jobs`) |

The files are embedded in the `badger` binary (`include_str!` in
`crates/badger_cli/src/templates.rs`), so a project can be created
without this repository being there: the binary does not read these
files at run time.

Every file may use the placeholder `@name@`, which is replaced with the
package name of the new project (in the path of a file, too).

To add a template: make a directory here, add the files, and list them
in `crates/badger_cli/src/templates.rs` (the `include_str!` of each
file, with the path it takes in the project). A template must check and
run: `badger check` on the new project passes, and `badger run` runs it
(a library package has no `main` to run; `badger test` runs its tests).
