# sqlshield-cli

Command-line linter for [sqlshield](https://github.com/davidsmfreire/sqlshield) —
validates raw SQL strings embedded in Python and Rust source files
against a declared schema, without touching a database.

## Install

```sh
cargo install sqlshield-cli
```

## Usage

```sh
sqlshield --directory src --schema schema.sql
# src/queries.py:2: error: Column `nickname` not found in table `users`
```

### Flags

```text
sqlshield [OPTIONS]

  -d, --directory <DIRECTORY>  Default: "."
  -s, --schema    <SCHEMA>     Default: "schema.sql"
      --dialect   <DIALECT>    generic | postgres | mysql | sqlite |
                               mssql | snowflake | bigquery | redshift |
                               clickhouse | duckdb | hive | ansi
                               (default: generic)
      --format    <FORMAT>     text | json (default: text)
      --stdin                  Read one SQL query from stdin instead of
                               walking a directory; ignores --directory.
  -h, --help
  -V, --version
```

### Configuration

A `.sqlshield.toml` at the project root supplies defaults; CLI flags
override it. See the
[main README](https://github.com/davidsmfreire/sqlshield#configuration)
for full details.

```toml
schema = "db/schema.sql"
directory = "src"
dialect = "postgres"
```

### Exit codes

| Code | Meaning                                                           |
| :--: | :---------------------------------------------------------------- |
| `0`  | No validation errors found.                                        |
| `1`  | Validation errors found (or stdin SQL parse failure).              |
| `2`  | IO / configuration error: missing schema, malformed config, etc.   |

### JSON output

```json
[
  {
    "location": "src/queries.py:2",
    "description": "Column `nickname` not found in table `users`"
  }
]
```

Suitable for `jq` pipelines and CI annotators. Stable shape.

### Stdin mode

```sh
echo "SELECT id, missing FROM users" | sqlshield --stdin --schema schema.sql
# error: Column `missing` not found in table `users`
```

Stdin mode omits `location` from JSON output (the source is the buffer).

## What gets checked

See the [main README](https://github.com/davidsmfreire/sqlshield#feature-support)
for the full feature matrix. Headline coverage: `SELECT` /
`INSERT` / `UPDATE` / `DELETE` against the schema, including
`WHERE` / `HAVING` / `GROUP BY` / `ORDER BY`, `JOIN ON` / `USING`, CTEs,
derived tables, subqueries (each in their own scope),
`UNION` / `INTERSECT` / `EXCEPT`, function arguments, `CASE` / `CAST`,
schema-qualified names, and `ALTER TABLE` / `CREATE VIEW` / `CTAS`
during schema ingestion. Identifier matching is ASCII case-insensitive.

## License

[MIT](https://github.com/davidsmfreire/sqlshield/blob/main/LICENSE).
