# sqlshield

Core library for [sqlshield](https://github.com/davidsmfreire/sqlshield) —
a schema-aware SQL linter that validates raw SQL strings inside source
code without touching a database.

This crate is the engine. Most users want one of the front-ends:

- [`sqlshield-cli`](https://crates.io/crates/sqlshield-cli) — CLI binary.
- [`sqlshield-lsp`](https://github.com/davidsmfreire/sqlshield/tree/main/sqlshield-lsp) — Language Server.
- [`sqlshield`](https://pypi.org/project/sqlshield/) (PyPI) — Python bindings.

## Library API

```rust
use sqlshield::{validate_query, Dialect};

let errors = validate_query(
    "SELECT id, missing FROM users",
    "CREATE TABLE users (id INT, name VARCHAR(255))",
)?;
// vec!["Column `missing` not found in table `users`"]

// Dialect-specific parsing:
let errors = sqlshield::validate_query_with_dialect(
    "SELECT id::TEXT FROM users",  // Postgres `::` cast
    "CREATE TABLE users (id INT)",
    Dialect::Postgres,
)?;
```

For walking a directory tree:

```rust
use sqlshield::validate_files;
use std::path::Path;

let errors = validate_files(Path::new("./src"), Path::new("./schema.sql"))?;
for err in errors {
    println!("{err}");
}
```

Public surface:

- `validate_query`, `validate_query_with_dialect`
- `validate_files`, `validate_files_with_dialect`
- `Dialect` (12 dialects: Generic, Postgres, MySql, Sqlite, MsSql,
  Snowflake, BigQuery, Redshift, ClickHouse, DuckDb, Hive, Ansi)
- `SqlShieldError`, `Result`
- Modules: `finder`, `schema`, `validation`, `dialect`, `error`

See the [main README](https://github.com/davidsmfreire/sqlshield#readme)
for the full picture: features, configuration, editor integration,
limitations.

## License

[MIT](https://github.com/davidsmfreire/sqlshield/blob/main/LICENSE).
