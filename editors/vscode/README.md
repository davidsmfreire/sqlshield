# sqlshield for VS Code

First-party VS Code integration for [sqlshield](https://github.com/davidsmfreire/sqlshield) —
a schema-aware static linter for raw SQL embedded in `.py`, `.rs`, and `.sql`
files.

## Requirements

The extension wraps the `sqlshield-lsp` binary; install it first:

```bash
cargo install sqlshield-lsp
```

If `sqlshield-lsp` is not on `PATH`, set `sqlshield.serverPath` to the
absolute path.

## How it works

The extension spawns `sqlshield-lsp` over stdio and forwards diagnostics
to the editor as you type. A `.sqlshield.toml` at the workspace root
configures schema location and dialect; see the project README for the
full set of options.

## Commands

* **sqlshield: Restart Language Server** — restart the server without
  reloading the window. Useful after editing the schema or
  `.sqlshield.toml` from outside VS Code.

## Settings

* `sqlshield.serverPath` — binary path (default: `sqlshield-lsp`).
* `sqlshield.trace.server` — log LSP traffic for debugging.
