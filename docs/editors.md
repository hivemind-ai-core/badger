# Editor setup

`badger lsp` speaks the Language Server Protocol on stdin/stdout. It provides
diagnostics (as `badger check` reports them), hover (types and signatures),
go to definition, find references, document highlights, rename, document
and workspace symbols, formatting, completion (triggered by `.`), and a
quick fix that adds the import of a standard module used without one, and
one that adds the arm a non-exhaustive `match` misses (its body panics
until it is written). It
needs no configuration: a file inside a directory with `badger.toml` is
checked as part of that project; any other file is checked on its own.

## Neovim (0.11+)

```lua
vim.filetype.add({ extension = { bdg = "badger" } })
vim.lsp.config("badger", {
  cmd = { "badger", "lsp" },
  filetypes = { "badger" },
  root_markers = { "badger.toml", ".git" },
})
vim.lsp.enable("badger")
```

## Helix (`languages.toml`)

```toml
[language-server.badger]
command = "badger"
args = ["lsp"]

[[language]]
name = "badger"
scope = "source.badger"
file-types = ["bdg"]
roots = ["badger.toml"]
comment-token = "//"
indent = { tab-width = 4, unit = "    " }
language-servers = ["badger"]
```

## Emacs (eglot)

```elisp
(define-derived-mode badger-mode prog-mode "Badger")
(add-to-list 'auto-mode-alist '("\\.bdg\\'" . badger-mode))
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(badger-mode "badger" "lsp")))
```

## VS Code

Any generic LSP client extension can start `badger lsp` for `*.bdg` files;
for example, with a minimal extension's `extension.js`:

```js
const { LanguageClient } = require("vscode-languageclient/node");
exports.activate = () => new LanguageClient("badger", "badger",
  { command: "badger", args: ["lsp"] },
  { documentSelector: [{ scheme: "file", pattern: "**/*.bdg" }] }).start();
```

## Formatting

The server advertises `textDocument/formatting`, so a client can offer to
format a document. A request is answered with the standard `TextEdit[]`: one
edit whose range covers the whole document and whose new text is the
canonical layout — the same one `badger format` writes, so a file formatted
in the editor and one formatted on the command line agree. The client asks
for the formatting and applies the edits; the server never rewrites a file
on its own.

In VS Code, with the Badger language client running:

```json
{
  "editor.formatOnSave": true
}
```

In Neovim (0.11+), on a save:

```lua
vim.api.nvim_create_autocmd("BufWritePost", {
  pattern = "*.bdg",
  callback = function() vim.lsp.buf.format({ async = false }) end,
})
```

In Helix, with `editor.format-on-save = true` in `config.toml`.

## Syntax highlighting

`editors/tree-sitter-badger` is a tree-sitter grammar with highlight
queries; its [README](../editors/tree-sitter-badger/README.md) explains how
to install it in Neovim and Helix, and where it differs from the compiler's
parser.
