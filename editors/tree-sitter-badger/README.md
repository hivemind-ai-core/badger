# tree-sitter-badger

A [tree-sitter](https://tree-sitter.github.io/) grammar for Badger (`.bdg`):
syntax highlighting and structural queries in editors with tree-sitter
support. Editors still use `badger lsp` for diagnostics, types, and
navigation (see [docs/editors.md](../../docs/editors.md)).

The generated parser (`src/parser.c`, `src/grammar.json`,
`src/node-types.json`) is checked in, so an editor can compile it without
the tree-sitter CLI. `src/scanner.c` handles nested block comments.

## Regenerating and testing

```sh
cd editors/tree-sitter-badger
npx tree-sitter-cli@0.25.10 generate
npx tree-sitter-cli@0.25.10 test                    # test/corpus
git -C ../.. ls-files '*.bdg' | sed 's#^#../../#' > /tmp/bdg-files.txt
npx tree-sitter-cli@0.25.10 parse --quiet --paths /tmp/bdg-files.txt
```

Every `.bdg` file in the repository parses without errors except the
deliberately malformed ones under `tests/reject/` and `tests/adversarial/`,
and the one known limitation below.

## Differences from the compiler's parser

The grammar follows `crates/badger_syntax/src/grammar.rs`, with two
simplifications. The compiler is the authority on what a program means.

- **Newlines are whitespace here.** The compiler's layout pass makes a
  newline end a statement; this grammar does not reproduce it, so a rare
  multi-line form (a line starting with `(` or `[` after a complete
  expression) may be read as a call or index rather than a new statement.
- **`map`, `set`, `system`, and `pipeline` followed by `{`** are always
  their literal forms. A *variable* with one of these names directly
  before a block (`match set { ... }`) is not parsed correctly; the
  compiler backtracks there and this grammar cannot.

## Using it

Neovim (the `master` branch API of nvim-treesitter):

```lua
local parsers = require("nvim-treesitter.parsers").get_parser_configs()
parsers.badger = {
  install_info = { url = "/path/to/repo/editors/tree-sitter-badger", files = { "src/parser.c", "src/scanner.c" } },
  filetype = "badger",
}
vim.filetype.add({ extension = { bdg = "badger" } })
```

Copy `queries/highlights.scm` to `queries/badger/highlights.scm` on your
runtime path.

Helix (`languages.toml`):

```toml
[[grammar]]
name = "badger"
source = { path = "/path/to/repo/editors/tree-sitter-badger" }
```

then `hx --grammar build`, and copy `queries/highlights.scm` to
`runtime/queries/badger/highlights.scm`.
