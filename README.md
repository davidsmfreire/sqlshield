# sqlshield

> Schema-aware SQL linter for embedded queries. Catches missing tables,
> missing columns, and broken JOINs in raw SQL strings inside Python and
> Rust source — at edit time, not at runtime.

```python
def fetch_user(uid):
    return db.execute(f"SELECT id, nickname FROM users WHERE id = {uid}")
```

```text
$ sqlshield --directory src --schema schema.sql
src/queries.py:2: error: Column `nickname` not found in table `users`
```

## Why

Raw SQL strings inside application code are a common blind spot. Type
checkers don't read them. Database connections are mocked in unit tests.
The error surfaces only when the query runs — which is fine on the happy
path and miserable everywhere else. sqlshield reads your source files,
extracts every SQL string, and validates each one against your schema —
without touching a database.

It works on:

- Plain `"…"` and raw `r#"…"#` Rust string literals (sqlx-idiomatic).
- Python f-strings (`f"…{x}…"`) and `.format()` strings (`{{` / `}}`
  escapes preserved).
- Standalone `.sql` files (via the LSP).

It checks:

- Tables and columns referenced anywhere — projection, `WHERE`, `HAVING`,
  `GROUP BY`, `ORDER BY`, `JOIN ON` / `USING`, function arguments, `CASE`
  branches, `CAST`, arithmetic, set operations.
- `INSERT` / `UPDATE` / `DELETE` target tables and column lists.
- `WITH` / CTEs (including `WITH RECURSIVE` and explicit column lists),
  derived tables in `FROM`, parenthesized join groups, scalar / `IN` /
  `EXISTS` subqueries — each in their own scope.
- Schema-qualified table names (`public.users`) — strict for qualified
  queries, permissive for bare ones.

## Install

```sh
# Rust users
cargo install sqlshield-cli

# Python users
pip install sqlshield
```

Or build from source:

```sh
git clone https://github.com/davidsmfreire/sqlshield
cd sqlshield
cargo build --release
./target/release/sqlshield --help
```

## Quick start

1. Write a schema file (`schema.sql` by default):

   ```sql
   CREATE TABLE users (id INT, name VARCHAR(255), email VARCHAR(255));
   CREATE TABLE orders (id INT, user_id INT, total INT);
   ```

2. Run sqlshield on your source tree:

   ```sh
   sqlshield --directory src --schema schema.sql
   ```

3. Each finding is reported as `path:line: error: <description>`. The
   process exits `0` on clean, `1` if validation errors were found, and
   `2` for IO / config problems (missing schema, malformed config,
   stdin read failure).

### Standalone query mode

```sh
echo "SELECT id, missing FROM users" | sqlshield --stdin --schema schema.sql
# error: Column `missing` not found in table `users`
```

Useful for editor integrations that pipe a single buffer through the
linter.

### JSON output

```sh
sqlshield --directory src --schema schema.sql --format json
```

```json
[
  {
    "location": "src/queries.py:2",
    "description": "Column `nickname` not found in table `users`"
  }
]
```

Stable shape; safe to pipe into `jq` or feed to a CI annotator.

## Configuration

Drop a `.sqlshield.toml` at the project root. CLI flags override the
config; the config overrides defaults.

```toml
# .sqlshield.toml
schema = "db/schema.sql"
directory = "src"
dialect = "postgres"
```

Supported dialects: `generic` (default), `postgres` / `postgresql` / `pg`,
`mysql`, `sqlite`, `mssql` / `sqlserver`, `snowflake`, `bigquery` / `bq`,
`redshift`, `clickhouse`, `duckdb`, `hive`, `ansi`. The dialect controls
how the SQL parser handles vendor-specific syntax (Postgres `::` casts,
MySQL backticks, …).

The walker prunes `target/`, `.git/`, `node_modules/`, `.venv/`, `venv/`,
`__pycache__/`, `.pytest_cache/`, `.mypy_cache/`, `.ruff_cache/`, `.tox/`,
`dist/`, `build/`, `.idea/`, and `.vscode/` automatically.

## Editor integration

[`sqlshield-lsp`](sqlshield-lsp/README.md) is a Language Server that
publishes diagnostics for embedded SQL on every `didOpen` / `didChange`.
Any LSP-aware editor (VS Code, Neovim, Helix, Emacs, Zed) can show inline
squiggles on the offending SQL string. The crate's README has the wiring
recipes.

## Python integration

[`sqlshield-py`](sqlshield-py/) exposes `validate_query` and
`validate_files` as Python functions:

```python
import sqlshield

errors = sqlshield.validate_query(
    "SELECT email FROM users",
    "CREATE TABLE users (id INT, name VARCHAR(255))",
)
# ['Column `email` not found in table `users`']
```

## Feature support

| Clause / construct                                  | Status |
| --------------------------------------------------- | :----: |
| `SELECT` projection                                  |   ✅   |
| `WHERE` / `HAVING` / `GROUP BY` / `ORDER BY`         |   ✅   |
| Projection alias references in `HAVING` / `ORDER BY` |   ✅   |
| `JOIN` `ON` / `USING`                                |   ✅   |
| Parenthesized join groups                            |   ✅   |
| `WITH` / CTE, `WITH RECURSIVE`, explicit `(a, b)` lists |   ✅   |
| Derived tables (`FROM (SELECT …) alias`)             |   ✅   |
| Subqueries (`IN`, `EXISTS`, scalar) — own scope      |   ✅   |
| `UNION` / `INTERSECT` / `EXCEPT`                     |   ✅   |
| `INSERT` (incl. `INSERT … SELECT`)                   |   ✅   |
| `UPDATE` (assignments, `WHERE`, `FROM`)              |   ✅   |
| `DELETE` (`USING`, `WHERE`)                          |   ✅   |
| `WITH … INSERT/UPDATE`                               |   ✅   |
| Schema-qualified names (`public.users`)              |   ✅   |
| `ALTER TABLE ADD/DROP/RENAME COLUMN` ingestion       |   ✅   |
| `CREATE VIEW` / `CREATE TABLE … AS SELECT`           |   ✅   |
| Function args / `CASE` / `CAST` / arithmetic         |   ✅   |
| Case-insensitive identifier matching                 |   ✅   |
| 12 SQL dialects via `--dialect`                      |   ✅   |
| Live database introspection                          |   ✗    |
| Quoted-vs-unquoted identifier folding (Postgres rules) |   ✗  |
| `MERGE`                                              |   ✗    |

## Limitations

- **Identifier matching is ASCII case-insensitive.** sqlshield treats
  `Id` and `id` as the same column. Postgres-style "quoted identifiers
  are case-sensitive" semantics aren't modeled.
- **Dynamic table / column names** (`SELECT {col} FROM t`) substitute the
  placeholder with `1`. Column-position placeholders silently pass; table-
  position placeholders break the parse and the query is dropped.
- **Two qualified tables sharing a bare name** (`schema_a.users` and
  `schema_b.users`) collide on the bare key — last declaration wins for
  unqualified queries. Qualified references resolve strictly.
- **Schema is parsed once.** Triggers, stored procedures, and INSTEAD OF
  rules aren't tracked.
- **Per-file errors are silently swallowed** during a directory scan
  (parse failures, missing-extension errors). Use `--stdin` to surface
  them for a single query.

## Architecture

```text
Source file (*.py, *.rs)
   │ tree-sitter extracts string literals (decoded escapes / raw strings)
   ▼
SQL string (with `{…}` placeholders replaced by `1`)
   │ sqlparser parses with the chosen dialect
   ▼
AST (Vec<Statement>)
   │ recursive walker: scope-aware Expr resolution + clause validators
   ▼
Vec<SqlValidationError>
```

Workspace layout:

- [`sqlshield/`](sqlshield/) — core library. Public surface:
  `validate_query`, `validate_files`, `Dialect`, `SqlShieldError`.
- [`sqlshield-cli/`](sqlshield-cli/) — clap-based CLI wrapper.
- [`sqlshield-py/`](sqlshield-py/) — PyO3 bindings.
- [`sqlshield-lsp/`](sqlshield-lsp/) — `tower-lsp` Language Server.

## Similar tools

- [`postguard`](https://github.com/andywer/postguard) — Postgres-only,
  ts-only, runs against a live database.
- [`schemasafe`](https://github.com/schemasafe/schemasafe) —
  query-checker for TypeScript / JavaScript.
- [`sqlc`](https://github.com/sqlc-dev/sqlc) — code generator that
  type-checks SQL against a schema; reads the schema, then generates
  bindings rather than linting existing code.
- [`squawk`](https://github.com/sbdchd/squawk) — Postgres migration
  linter; complementary, not overlapping (squawk lints DDL, sqlshield
  lints embedded DML/SELECT).

sqlshield's niche: language-agnostic extraction (Python + Rust today,
extensible) with a multi-dialect parser, no database connection
required.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the dev setup, the
ClauseValidation extension recipe, and the release process.

## License

[MIT](LICENSE).
