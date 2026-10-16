# Editor setup

`lang lsp` speaks the Language Server Protocol on stdin/stdout. It provides
diagnostics (as `lang check` reports them), hover (types and signatures),
go to definition, find references, document highlights, rename, document
and workspace symbols, formatting, and completion (triggered by `.`). It
needs no configuration: a file inside a directory with `lang.toml` is
checked as part of that project; any other file is checked on its own.

## Neovim (0.11+)

```lua
vim.filetype.add({ extension = { lang = "lang" } })
vim.lsp.config("lang", {
  cmd = { "lang", "lsp" },
  filetypes = { "lang" },
  root_markers = { "lang.toml", ".git" },
})
vim.lsp.enable("lang")
```

## Helix (`languages.toml`)

```toml
[language-server.lang]
command = "lang"
args = ["lsp"]

[[language]]
name = "lang"
scope = "source.lang"
file-types = ["lang"]
roots = ["lang.toml"]
comment-token = "//"
indent = { tab-width = 4, unit = "    " }
language-servers = ["lang"]
```

## Emacs (eglot)

```elisp
(define-derived-mode lang-mode prog-mode "Lang")
(add-to-list 'auto-mode-alist '("\\.lang\\'" . lang-mode))
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(lang-mode "lang" "lsp")))
```

## VS Code

Any generic LSP client extension can start `lang lsp` for `*.lang` files;
for example, with a minimal extension's `extension.js`:

```js
const { LanguageClient } = require("vscode-languageclient/node");
exports.activate = () => new LanguageClient("lang", "lang",
  { command: "lang", args: ["lsp"] },
  { documentSelector: [{ scheme: "file", pattern: "**/*.lang" }] }).start();
```
