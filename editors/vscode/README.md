# sqlshield for VS Code

First-party VS Code integration for [sqlshield](https://github.com/davidsmfreire/sqlshield) —
a schema-aware static linter for raw SQL embedded in `.py`, `.rs`, and `.sql`
files.

## Requirements

Platform-specific builds of this extension bundle the `sqlshield-lsp`
binary, so installing the extension is enough on Linux x64/arm64,
macOS, and Windows x64/arm64. On unsupported platforms (or for users
who'd rather use their own build), install the binary manually:

```bash
cargo install sqlshield-lsp
```

If `sqlshield-lsp` is not on `PATH` and the bundled binary isn't being
picked up, set `sqlshield.serverPath` to the absolute path.

## How it works

The extension spawns `sqlshield-lsp` over stdio and forwards diagnostics
to the editor as you type. Configuration comes from two sources, with
editor settings winning per-field:

1. The `sqlshield.*` VS Code settings below.
2. A `.sqlshield.toml` discovered by walking up from the workspace root.

## Commands

* **sqlshield: Restart Language Server** — restart the server without
  reloading the window. Useful after switching the binary path.

## Settings

* `sqlshield.schema` — path to the schema SQL file (relative to
  workspace root or absolute). Overrides `schema` in `.sqlshield.toml`.
* `sqlshield.dialect` — SQL dialect (`postgres`, `mysql`, `sqlite`,
  `mssql`, `snowflake`, `bigquery`, `redshift`, `clickhouse`, `duckdb`,
  `hive`, `ansi`, `generic`). Overrides `dialect` in `.sqlshield.toml`.
* `sqlshield.serverPath` — binary path. Leave blank for the bundled
  binary; falls back to `sqlshield-lsp` on `PATH`.
* `sqlshield.trace.server` — log LSP traffic for debugging.
