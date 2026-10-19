# A migration cookbook

Four chapters, one for each of Go, Python, Rust and TypeScript. Each has a
small program you will recognise, the Badger equivalent, and the
differences that will surprise you when you move the code.

Every `badger` program on this page is complete and type-checks: the
documentation test checks each one, and every output shown is what
`badger run` printed.

If the language is new to you, read the [tour](tour.md) first. The
[guide](guide/README.md) has the detail behind every point here, and
[Idiomatic Badger](idiomatic.md) has the conventions to follow once the
code runs.

## What is the same

Most of what you already know carries over. These are the ideas that do:

| Idea | In Badger | Where it comes from |
|---|---|---|
| A file is a module | One file is one module. In a project the path under `src/` is the module path, so `src/orders/store.bdg` is the module `orders.store`. | Python and TypeScript: a file is a module. Go is the exception: a package is a directory of files. |
| `let` binds, `var` rebinds | `let` is immutable, and a later `let` shadows it. `var` makes the binding reassignable; the type stays the same. | Go: `:=` creates a variable, `=` assigns it. TypeScript: `const` and `let`. |
| Records are structural | A record's type is its shape. A function that reads `book.title` and says nothing else takes any record with a `title` field, and the rest of the record passes through untouched. | TypeScript is structural too. Go and Python have the same idea as convention — duck typing — but Badger checks it. |
| `?` propagates a failure | `expr?` returns the `err` from the enclosing function. The error type converts through `From` when there is exactly one conversion to it. | Rust's `?` is the same idea. Go writes `if err != nil` by hand; Python and TypeScript have no equivalent, because they raise. |
| A provider is a dependency | A `provider` returns what the node provides, and `ref(#other)` in a config wires one node to another. The plan is checked before anything starts. | Go's `NewStore(cfg)` and the wiring in `main`; Python's constructor injection; a TypeScript DI container. The same shape, with the wiring checked. |
| `match` covers every case | A `match` must handle every variant of an enum, and the compiler says which one is missing. | Rust's `match` on an enum. |
| The last expression is the value | A function body ends with an expression; `return` leaves early. A block is an expression as well. | Rust's tail expression. |

## Go

### The program in Go

```go
package main

import "fmt"

type Book struct {
	Title  string
	Author string
	Year   int
}

func booksByAuthor(books []Book, author string) ([]Book, error) {
	var found []Book
	for _, b := range books {
		if b.Author == author {
			found = append(found, b)
		}
	}
	if len(found) == 0 {
		return nil, fmt.Errorf("no books by %q", author)
	}
	return found, nil
}

func main() {
	books := []Book{
		{Title: "Dune", Author: "Herbert", Year: 1965},
		{Title: "Children of Dune", Author: "Herbert", Year: 1976},
		{Title: "Neuromancer", Author: "Gibson", Year: 1984},
	}
	found, err := booksByAuthor(books, "Herbert")
	if err != nil {
		fmt.Println("error:", err)
		return
	}
	for _, b := range found {
		fmt.Println(b.Title)
	}
}
```

It prints:

```text
Dune
Children of Dune
```

### The Badger version

```badger
import std.io

type Book = { title: String, author: String, year: Int }

pub enum AppError {
    no_books(author: String),
    io(source: io.Error),
} derive(Error, From)

fn books_by_author(books: List<Book>, author: String) -> Result<List<Book>, AppError> {
    let found = books.filter(fn(book) => book.author == author)
    if found.is_empty() {
        return err(AppError.no_books(author))
    }
    ok(found)
}

pub fn main() -> Result<Unit, AppError> {
    let books: List<Book> = [
        { title: "Dune", author: "Herbert", year: 1965 },
        { title: "Children of Dune", author: "Herbert", year: 1976 },
        { title: "Neuromancer", author: "Gibson", year: 1984 },
    ]
    let found = books_by_author(books, "Herbert")?
    for book in found {
        io.println(book.title)?
    }
    ok(())
}
```

It prints the same:

```text
Dune
Children of Dune
```

Two things are worth noticing. The failure is a value in an enum, so `?`
can return it and the compiler knows every failure the function has.
`derive(From)` gives `AppError` a conversion from `io.Error`, which is
why `io.println(...)?` works in `main` without a `match`. And the record
is not a declaration in the Go sense: `type Book = { ... }` names a
shape, so any record with exactly a `title`, an `author` and a `year`
fits, and one with a field missing or to spare is a type error. Write
the type as `{ title: String, ..Rest }` when you mean "these fields, and
perhaps others".

The error path is not exercised here. When `main` returns `err`, the
program prints a report on standard error and exits with status 1; the
report is in [Errors](guide/03-errors.md#when-main-returns-an-error).

### Another way to write it

```badger fragment
// The explicit loop: the same result, without the method call.
var found: List<Book> = []
for book in books {
    if book.author == author {
        found = found.append(book)
    }
}
```

Use `filter` when the test is a single condition on the element. Use the
loop, or `list.build`, when you are collecting from a larger
computation: `append` makes a new list each time, so growing a long list
one item at a time copies it every time.

### Differences to know about

| Difference | In Go | In Badger |
|---|---|---|
| Errors are values you check by hand | `error` is an interface. Every call returns one, and a forgotten check still compiles. | `Result<T, E>` is an enum, and a `Result` must be used (E0337). `?` returns the error for you, converting it through `From` when the error type allows. See [Errors](guide/03-errors.md). |
| A missing value is a nil pointer | `(*Book, error)`; a nil pointer that is dereferenced panics at run time. | `Option<T>` for absence and `Result<T, E>` for failure. There is no null: reading a field a record does not have is a type error (E0301). See [Errors](guide/03-errors.md) and [Types and records](guide/02-types-and-records.md). |
| A struct is nominal, and you edit it in place | `Book` is `Book`. Two structs with the same fields are still different types, and a method with a pointer receiver changes the value it was given. | A record is structural, and records are values: `{ ..book, year: 2020 }` makes a new one and leaves the old one alone. Use a `struct` when the type needs private fields or an invariant. See [Types and records](guide/02-types-and-records.md). |
| Goroutines are not tied to anything | `go f()` starts a goroutine; `sync.WaitGroup` and `context` manage it. Nothing checks that a goroutine has finished. | Every task belongs to a scope: `use scope <- task.scope()`, `scope.spawn(fn() { ... })`, and the scope waits for its children or cancels them. Values crossing a task must be `Transfer`. There is no `async`. See [Tasks and channels](guide/06-tasks-and-channels.md). |

Two smaller things. `Int` is always 64 bits, where Go's `int` is
whatever the platform has, and Badger checks arithmetic for overflow in
every build — a Go program that overflows quietly wraps. And the build is
a `badger.toml` and a `badger.lock` rather than a `go.mod`: `badger
check` type-checks, `badger run` compiles and runs, and `badger release`
packages a program to run elsewhere. See
[Modules and projects](guide/05-modules-and-projects.md) and
[Releases](releases.md).

## Python

### The program in Python

```python
CATALOGUE = [
    {"title": "Dune", "author": "Herbert", "year": 1965},
    {"title": "Children of Dune", "author": "Herbert", "year": 1976},
    {"title": "Neuromancer", "author": "Gibson", "year": 1984},
    {"title": "The Dispossessed", "author": None, "year": 1974},
]


def books_by_author(books, author):
    return [book for book in books if book.get("author") == author]


for book in books_by_author(CATALOGUE, "Herbert"):
    print(book["title"])
```

It prints:

```text
Dune
Children of Dune
```

### The Badger version

```badger
import std.io

type Book = { title: String, author: Option<String>, year: Int }

fn books_by_author(books: List<Book>, author: String) -> List<Book> {
    books.filter(fn(book) => book.author == some(author))
}

pub fn main() -> Result<Unit, io.Error> {
    let books: List<Book> = [
        { title: "Dune", author: some("Herbert"), year: 1965 },
        { title: "Children of Dune", author: some("Herbert"), year: 1976 },
        { title: "Neuromancer", author: some("Gibson"), year: 1984 },
        { title: "The Dispossessed", author: none, year: 1974 },
    ]
    for book in books_by_author(books, "Herbert") {
        io.println(book.title)?
    }
    ok(())
}
```

It prints the same:

```text
Dune
Children of Dune
```

`None` is `Option<String>` now: `some("Herbert")` for a book with an
author, `none` for one without. There is no null, and the compiler
insists you say which of the two you mean. The list is one value, and the
`filter` gives you another.

### Another way to write it

```badger fragment
fn books_by_author(books: List<Book>, author: String) -> List<Book> {
    books.filter(fn(book) => match book.author {
        .some(a) => a == author,
        .none => false,
    })
}
```

Use `book.author == some(author)` when the option is only compared with
a value. Use `match` when the two cases need different handling, or when
you need the author itself rather than a yes or no.

### Differences to know about

| Difference | In Python | In Badger |
|---|---|---|
| Failure is an exception | `raise` unwinds the stack; the caller catches it or it reaches the top. | Expected failures are values: `Result<T, E>`, propagated with `?`. A defect — `panic`, a failed assertion, an overflow — is the bug case, and it stops the task. See [Errors](guide/03-errors.md). |
| `None` is everywhere | `None` means "no value" and every type can hold it, so the check happens at run time. | `Option<T>` is a separate type. `some(x)` and `none` are its two cases, and a function that takes `Option<T>` says so. See [Errors](guide/03-errors.md). |
| Nothing is checked until it runs | A type hint is optional and never checked, so `book["title"]` on a list of strings is found at run time. | Every program is checked before it runs, and a wrong shape is a diagnostic, not a run-time surprise. `badger check` does it on its own. See [Values and functions](guide/01-values-and-functions.md). |
| A list is mutated in place | `found.append(book)` changes the list you already have. | `List` is immutable: `append` makes a new list and leaves the old one as it was. Use `list.build` when you are assembling a list, and `Buffer` when you genuinely want a mutable one. See [Collections](guide/04-collections.md). |

Two smaller things. A Python `dict` with string keys is a `Map<String, V>`
in Badger, but a record with fixed field names is a different thing: it
has a known shape and the compiler checks it. And the packaging is
`badger.toml` with path or git dependencies, not `pip` and a
`requirements.txt`; `import std.list` is the standard library, and
everything else is imported by name. See
[Modules and projects](guide/05-modules-and-projects.md).

## Rust

### The program in Rust

```rust
struct Book {
    title: String,
    author: String,
    year: u16,
}

fn books_by_author<'a>(books: &'a [Book], author: &str) -> Result<Vec<&'a Book>, String> {
    let found: Vec<&Book> = books.iter().filter(|b| b.author == author).collect();
    if found.is_empty() {
        return Err(format!("no books by {author}"));
    }
    Ok(found)
}

fn main() {
    let books = vec![
        Book { title: "Dune".into(), author: "Herbert".into(), year: 1965 },
        Book { title: "Children of Dune".into(), author: "Herbert".into(), year: 1976 },
        Book { title: "Neuromancer".into(), author: "Gibson".into(), year: 1984 },
    ];
    match books_by_author(&books, "Herbert") {
        Ok(found) => {
            for book in found {
                println!("{}", book.title);
            }
        }
        Err(e) => eprintln!("error: {e}"),
    }
}
```

It prints:

```text
Dune
Children of Dune
```

### The Badger version

```badger
import std.io

pub enum AppError {
    no_books(author: String),
    io(source: io.Error),
} derive(Error, From)

pub struct Book {
    title: String,
    author: String,
    year: Int,
}

fn book(title: String, author: String, year: Int) -> Book {
    { title, author, year }
}

impl Book {
    pub fn label(self: &Self) -> String { self.title }
    pub fn written_by(self: &Self, author: String) -> Bool { self.author == author }
}

fn books_by_author(books: List<Book>, author: String) -> Result<List<Book>, AppError> {
    let found = books.filter(fn(b) => b.written_by(author))
    if found.is_empty() {
        return err(AppError.no_books(author))
    }
    ok(found)
}

pub fn main() -> Result<Unit, AppError> {
    let books = [
        book("Dune", "Herbert", 1965),
        book("Children of Dune", "Herbert", 1976),
        book("Neuromancer", "Gibson", 1984),
    ]
    for b in books_by_author(books, "Herbert")? {
        io.println(b.label())?
    }
    ok(())
}
```

It prints the same:

```text
Dune
Children of Dune
```

`Book` is a `struct` here, not a record, so it is nominal: only this
module can read its fields, and the `book` function is the only way to
make one. `impl` adds the two methods. That is the Rust shape — a type
with private state and a constructor that keeps it honest — and Badger
has it too. The error type is an enum with `derive(Error, From)`, and
`?` returns it, exactly as it does in Rust.

### Another way to write it

```badger fragment
type Book = { title: String, author: String, year: Int }

fn books_by_author(books: List<Book>, author: String) -> Result<List<Book>, AppError> {
    let found = books.filter(fn(b) => b.author == author)
    if found.is_empty() {
        return err(AppError.no_books(author))
    }
    ok(found)
}
```

Use the `struct` when the type has an invariant its constructor
enforces, or when other code must not add fields to it. Use the record
when the data is plain: a record is structural, so any record with
exactly a `title`, an `author` and a `year` fits, and other code may
pass extra fields through where the type is written open, as
`{ title: String, ..Rest }`. The record version is shorter too, because
there are no methods to write.

### Differences to know about

| Difference | In Rust | In Badger |
|---|---|---|
| Ownership applies to everything | Every `String`, `Vec` and struct is moved or borrowed, and the compiler tracks it. | Only affine values move: buffers, builders, a `once fn`. Records, lists and maps are values that can be copied and shared freely. See [Effects and ownership](guide/07-effects-and-ownership.md). |
| Collections are mutated in place | `Vec::push` changes the vector; `books.retain(..)` changes the book list in the same way. | `List` is immutable: `append` and `filter` make a new list and leave the old one alone. `Buffer` is the mutable one, and it has one owner. See [Collections](guide/04-collections.md). |
| `unwrap` and `expect` | `result.unwrap()` panics; `option.expect("...")` panics with your message. | There is no `unwrap` and no `expect`. Match the value, or say that it is a bug with `panic`. See [Errors](guide/03-errors.md). |
| `async`, `Future`, `Send` and `Sync` | `async fn` returns a `Future`; the executor decides when it runs, and `Send`/`Sync` say whether it may cross threads. | There is no `async`: any function may suspend, and its effects say so — `!{Io, Wait, State}`. `Transfer` and `Share` replace `Send` and `Sync`, and the compiler derives them. See [Effects and ownership](guide/07-effects-and-ownership.md). |

Two smaller things. `Result`, `Option` and `?` work as they do in Rust,
including the conversion through `From` — but the error type of an
application is usually one enum, and `derive(Error, From)` writes it for
you. And Rust traits become Badger protocols, with two differences: a
protocol is implemented on a `struct` or an `enum`, never on a record,
and `derive` only knows six protocols — there are no user-defined
derivations or macros. See
[Types and records](guide/02-types-and-records.md#derive) and
[Idiomatic Badger](idiomatic.md#one-error-type).

## TypeScript

### The program in TypeScript

```ts
type Book = { title: string; author: string; year: number };

interface Library {
  titlesByAuthor(author: string): string[];
}

class Catalogue implements Library {
  constructor(private readonly books: Book[]) {}

  titlesByAuthor(author: string): string[] {
    return this.books.filter((b) => b.author === author).map((b) => b.title);
  }
}

const catalogue = new Catalogue([
  { title: "Dune", author: "Herbert", year: 1965 },
  { title: "Children of Dune", author: "Herbert", year: 1976 },
  { title: "Neuromancer", author: "Gibson", year: 1984 },
]);

for (const title of catalogue.titlesByAuthor("Herbert")) {
  console.log(title);
}
```

It prints:

```text
Dune
Children of Dune
```

### The Badger version

```badger
import std.io
import std.system

type Book = { title: String, author: String, year: Int }

pub enum AppError {
    system(source: system.Error),
    io(source: io.Error),
} derive(Error, From)

provider library(config: { author: String }) -> Result<_, AppError> {
    let books: List<Book> = [
        { title: "Dune", author: "Herbert", year: 1965 },
        { title: "Children of Dune", author: "Herbert", year: 1976 },
        { title: "Neuromancer", author: "Gibson", year: 1984 },
    ]
    ok({
        titles: fn() => books.filter(fn(b) => b.author == config.author).map(fn(b) => b.title),
    })
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        library: library({ author: "Herbert" }),
    }
    use running <- system.run(plan)
    for title in running.library.titles() {
        io.println(title)?
    }
    ok(())
}
```

It prints the same:

```text
Dune
Children of Dune
```

The constructor becomes a `provider`. The provider returns what the
component provides — here a record of functions — and the `system { }`
block says which components there are and how they are wired. The plan
is checked before anything starts, and `system.run` starts the nodes in
dependency order and halts them in reverse. A provider that owns a
database pool, a server socket or a background job declares that in its
scope, and the system takes care of starting and stopping it. This is
the DI container, with the wiring checked; it is
[Systems and pipelines](guide/08-systems-and-pipelines.md).

### Another way to write it

```badger fragment
fn titles_by_author(books: List<Book>, author: String) -> List<String> {
    books.filter(fn(b) => b.author == author).map(fn(b) => b.title)
}
```

Use the plain function when the catalogue is built once and passed
around. Use a provider when the component owns a resource, or wants
background work of its own — then the system starts it, supervises it,
and stops it when the program halts.

### Differences to know about

| Difference | In TypeScript | In Badger |
|---|---|---|
| Types are erased at run time | `Book` is gone once the code is compiled; `b.author` is whatever was there. A wrong shape is found in production. | The shape is in the value. A `List<Book>` holds books, and the compiler checks every record that goes in. See [Types and records](guide/02-types-and-records.md). |
| `null` and `undefined` | Both mean "no value", and both can be assigned to any type unless `strictNullChecks` is on. | `Option<T>` is a type of its own, with `some` and `none` as its two cases. There is no null. See [Errors](guide/03-errors.md). |
| Failure is an exception | `throw` unwinds to the nearest `try`; a rejected promise is the same thing in a different costume. | Expected failures are `Result` values, propagated with `?`; a defect stops the task. See [Errors](guide/03-errors.md). |
| `any` and `as` get you out of trouble | `any` turns off the checking, and `as` asserts a shape the compiler has not checked. | There is no `any` and no assertion. Every value has a type, and there are no implicit conversions: `Int.from(x)` widens, `U8.try_from(x)` narrows and can fail. See [Values and functions](guide/01-values-and-functions.md). |

Two smaller things. The event loop becomes tasks: a function that waits
is a task that suspends, and `badger run --workers N` spreads the tasks
over N threads, with the compiler checking what may cross between them.
And the packaging is `badger.toml` and `badger.lock` rather than
`package.json` and `tsconfig.json`: there is no bundler and no compiler
options file to configure, and `import std.list` is the standard library.
See [Modules and projects](guide/05-modules-and-projects.md).

## What is different

The three differences that matter most, per language.

| Language | The one you notice first | The one that changes the design | The one that catches you out |
|---|---|---|---|
| Go | `error` is a value you check by hand; Badger's `Result` must be used, and `?` returns it for you. | A nil pointer becomes `Option`; there is no null. | A `struct` is nominal and a record is structural: two records with the same fields are the same type. |
| Python | Exceptions become `Result` values, and `?` propagates them. | `None` becomes `Option`, and the type says so. | Lists are immutable: `append` gives you a new list. |
| Rust | `unwrap` and `expect` are gone; match the value, or `panic`. | Ownership is for affine values only; lists and records are values you copy and share. | There is no `async`: a function that waits says `!{Wait}`, and the compiler checks it. |
| TypeScript | Types are checked, and there is no `any` to hide behind. | `null` and `undefined` become `Option`. | Exceptions become `Result` values, so a function that can fail says so in its type. |

## Where to go next

- The [tour](tour.md) for the whole language in one pass.
- The [guide](guide/README.md) for the detail behind each point here.
- [Idiomatic Badger](idiomatic.md) for the conventions: one error type,
  validation, configuration, routes, pipeline stages, background workers,
  and tests.
- [Collections: choosing and costs](collections.md) for which collection
  to reach for.
