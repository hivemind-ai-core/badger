# Typed SQL

SQL stays SQL. Write queries in a `.sql` file, name them, and
`badger generate sql` writes a module of typed functions next to it, with
types from the database itself (in the style of sqlc and pgtyped).

```sql
-- name: create_user :one
-- Add a user; returns its id.
INSERT INTO users (name, email) VALUES (:name, :email?) RETURNING id;

-- name: find_user :opt
SELECT id, name, email FROM users WHERE id = :id;

-- name: users_named :many
SELECT id, name FROM users WHERE name LIKE :pattern ORDER BY name;

-- name: rename :exec
UPDATE users SET name = :name WHERE id = :id;
```

```sh
badger generate sql --database postgres://localhost/app queries.sql   # Postgres
badger generate sql queries.sql                                      # SQLite, [database] schema
badger generate sql --check queries.sql                              # CI: fail if stale
```

Each query becomes a function taking a connection (a pool or a
transaction) and its parameters:

```badger fragment
pub type FindUserRow = { id: Int, name: String, email: Option<String> }

pub fn find_user<C>(db: C, id: Int) -> Result<Option<FindUserRow>, postgres.Error> !{Io, Wait} where C: postgres.Connection
```

## Queries

- `-- name: NAME :RESULT` starts a query. `NAME` becomes the function
  (`FindUser` is written `find_user`) and its row type (`FindUserRow`).
- The result is `:one` (exactly one row; none or several is an error),
  `:opt` (`Option` of a row), `:many` (`List` of rows), or `:exec` (the
  changed row count).
- `--` lines right after the name line document the function, except a
  `-- params: id: Int, name: String` line, which gives parameter types and
  the order of the function's parameters (otherwise they follow their
  first use).
- Parameters are `:name`; `:name?` may be NULL (an `Option`). A name used
  twice is one parameter. `::` casts, strings, quoted identifiers,
  comments, and `$$` bodies are left alone.

## Types

Postgres describes each statement: parameter types, column types, and
whether a column read straight from a table can be NULL. A column the
server cannot vouch for (an expression, an aggregate, an outer join's
side) is an `Option`. Override that in the column's alias: `"total!"` is
never NULL, `"note?"` may be, and `"total!: Int"` also gives the type.

| Postgres | Language |
|---|---|
| `int2`, `int4`, `int8` | `Int` |
| `float4`, `float8` | `Float` |
| `bool` | `Bool` |
| `numeric` | `String` (exact) |
| text types, `uuid`, enums, `bytea` | `String` |
| `date`, `time`, `timestamp`, `timestamptz` | `String` (ISO 8601) |
| `json`, `jsonb` | `Data` |
| `T[]` | `List<T>` |

Other types are refused with advice to cast them (`::text`).

SQLite prepares each statement against the project's schema (see
[below](#sqlite-the-project-schema)): declared column types give `Int`,
`Float`, or `String` (other declared types, and expressions, are `Data`
unless the alias gives a type), and the schema gives nullability. SQLite
does not know parameter types: they are `Data` unless `-- params:` gives
them.

## SQLite: the project schema

Postgres answers type questions itself. SQLite has no server to ask, so a
project names a schema file in `badger.toml`:

```toml
[package]
name = "app"
entry = "src/main.bdg"

[database]
schema = "schema.sql"     # relative to the directory of badger.toml
```

`schema` is the only key of `[database]`, and the section only matters
for SQLite. The file is plain SQL: the `CREATE TABLE`, `CREATE INDEX`, and
`CREATE VIEW` statements of the database the program uses.

What reads it:

- `badger check` (and `run`, `test`, the language server, and MCP
  `check`) runs the file into an empty in-memory SQLite database, then
  prepares every literal SQL string the program passes to `stdx.sqlite`
  against it. Nothing touches a real database file. An unknown table or
  column is E0180, a wrong parameter count E0181, and a `query_as` row
  type that does not fit the columns E0182.
- `badger generate sql` uses it when there is no `--schema FILE`. It looks
  for the `badger.toml` nearest above each `.sql` file.

The program's own literal `CREATE` and `ALTER` statements are run after
the file, in source order, so a program that creates its tables at start
is checked without a schema file. Without either, only SQL syntax is
checked. A schema file that cannot be read, or whose SQL fails, is E0180.

### Getting the schema

From an existing database file, with the `sqlite3` command-line tool:

```sh
sqlite3 app.db '.schema --nosys' > schema.sql
```

`--nosys` leaves out SQLite's internal tables. Plain `.schema` includes
`CREATE TABLE sqlite_sequence(...)` when a table uses `AUTOINCREMENT`, and
that statement fails when the file is loaded ("object name reserved for
internal use").

Without the `sqlite3` tool, the same text is in the database itself:

```sql
SELECT sql || ';' FROM sqlite_schema
WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%';
```

From migrations: the schema is every migration applied in order, so
concatenate them (`cat migrations/*.sql > schema.sql`, when the file
names sort in order), or apply them to a scratch database and dump that:

```sh
rm -f /tmp/scratch.db
for f in migrations/*.sql; do sqlite3 /tmp/scratch.db < "$f"; done
sqlite3 /tmp/scratch.db '.schema --nosys' > schema.sql
```

Regenerate `schema.sql` whenever the database changes, then run
`badger check` and `badger generate sql --check` to find the code that no
longer fits.

The generated module is ordinary code: `badger check` also checks its
statements against the schema (for SQLite), and it can be read, diffed,
and committed. Regenerate it after changing the queries or the schema;
`--check` in CI catches a stale one.
