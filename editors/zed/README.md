# sqlshield for Zed

Schema-aware SQL diagnostics inside [Zed](https://zed.dev), powered by
[sqlshield](https://github.com/davidsmfreire/sqlshield).

## Install

This extension is intended to be installed from Zed's Extensions panel
(`zed: extensions`) once published. Until then you can install it as a
dev extension:

```bash
zed: install dev extension
# point at editors/zed/ inside this repo
```

The extension wraps the `sqlshield-lsp` binary. It looks for one in this
order:

1. `lsp.sqlshield.binary.path` in your Zed settings (absolute path).
2. `sqlshield-lsp` on `PATH` (`cargo install sqlshield-lsp`).
3. A prebuilt binary downloaded from
   [GitHub Releases](https://github.com/davidsmfreire/sqlshield/releases)
   matching your platform/architecture.

The recommended path today is `cargo install sqlshield-lsp`; the
GitHub-release fallback only fires once the LSP project ships
prebuilt binaries.

## Configuration

Schema and dialect can be set under `lsp.sqlshield.settings` in your Zed
settings, or via a `.sqlshield.toml` at the workspace root. Editor
settings win per-field; the toml fills any gaps.

```jsonc
{
  "lsp": {
    "sqlshield": {
      "settings": {
        "schema": "schema.sql",
        "dialect": "postgres"
      }
    }
  }
}
```

Valid `dialect` values: `generic`, `postgres`, `mysql`, `sqlite`,
`mssql`, `snowflake`, `bigquery`, `redshift`, `clickhouse`, `duckdb`,
`hive`, `ansi`.

`schema` is resolved relative to the workspace root (or absolute).
