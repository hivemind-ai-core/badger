# Editor setup

`badger lsp` speaks the Language Server Protocol on stdin/stdout. It provides
diagnostics (as `badger check` reports them), hover (types and signatures),
go to definition, find references, document highlights, rename, document
and workspace symbols, formatting, and completion (triggered by `.`). It
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
