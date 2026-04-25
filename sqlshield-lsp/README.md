# sqlshield-lsp

Language Server Protocol frontend for [sqlshield](../README.md). Emits
schema-aware SQL diagnostics for embedded queries in `.py` and `.rs` files as
well as plain `.sql` files. Every editor that speaks LSP (VS Code, Neovim,
Helix, Emacs, Zed, …) can show squiggles on the offending SQL string.

> **Status:** experimental — full-document sync, diagnostics only. No
> completion, hover, or code actions yet.

## Install

```sh
cargo install --path sqlshield-lsp
# or from the workspace root:
cargo build --release --bin sqlshield-lsp
```

## Configure the server

The server walks up from the workspace root looking for a
[`.sqlshield.toml`](../sqlshield-cli/src/config.rs) file. Relative paths
inside it are resolved against the file's directory.

```toml
# .sqlshield.toml
schema = "db/schema.sql"
dialect = "postgres"
```

Without a config file the server still runs, but with an empty schema it can
only flag SQL parse errors — missing-table/column diagnostics rely on the
schema being loaded.

## Wire it into an editor

### Neovim (nvim-lspconfig)

```lua
local configs = require("lspconfig.configs")
if not configs.sqlshield_lsp then
  configs.sqlshield_lsp = {
    default_config = {
      cmd = { "sqlshield-lsp" },
      filetypes = { "python", "rust", "sql" },
      root_dir = require("lspconfig.util").root_pattern(".sqlshield.toml", ".git"),
      settings = {},
    },
  }
end
require("lspconfig").sqlshield_lsp.setup({})
```

### VS Code

Any generic LSP extension (for example, `llllvvuu.llmvm-lsp-client`) can
launch `sqlshield-lsp`. A first-party VS Code extension is future work.

## Debugging

Set `RUST_LOG=sqlshield_lsp=debug` to get chatty stderr logs from the
server. Logs go to stderr so they don't interfere with the stdio JSON-RPC
transport.
