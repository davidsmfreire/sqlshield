# sqlshield (Python bindings)

Python bindings for [sqlshield](https://github.com/davidsmfreire/sqlshield) —
a schema-aware SQL linter that validates raw SQL strings inside source
code without touching a database.

## Install

```sh
pip install sqlshield
```

## Usage

### Validate a single query

```python
import sqlshield

errors = sqlshield.validate_query(
    "SELECT id, missing FROM users",
    "CREATE TABLE users (id INT, name VARCHAR(255))",
)
for err in errors:
    print(err)
# Column `missing` not found in table `users`
```

### Walk a directory tree

```python
import sqlshield

errors = sqlshield.validate_files("./src", "./schema.sql")
for err in errors:
    print(f"{err.location}: {err.description}")
# ./src/queries.py:7: Column `nickname` not found in table `users`
```

`validate_files` walks `.py`, `.rs`, `.go`, `.js`, `.ts`, and `.tsx`
source files, extracts every embedded SQL string, and validates each
against the schema. Generated and vendored directories (`.git`,
`target`, `node_modules`, `.venv`, `__pycache__`, …) are skipped
automatically.

Each `SqlValidationError` exposes:

- `location: str` — `path:line` where the SQL string starts.
- `description: str` — the validation error in plain text.

## What gets checked

`SELECT` / `INSERT` / `UPDATE` / `DELETE` against the schema, including
`WHERE` / `HAVING` / `GROUP BY` / `ORDER BY`, `JOIN ON` / `USING`,
projection aliases, CTEs (`WITH` / `WITH RECURSIVE`, explicit column
lists), derived tables, subqueries (each in their own scope),
`UNION` / `INTERSECT` / `EXCEPT`, function arguments, `CASE`, `CAST`,
and arithmetic. `ALTER TABLE ADD/DROP/RENAME COLUMN`, `CREATE VIEW`,
and `CREATE TABLE … AS SELECT` are honored when loading the schema.

Identifier comparisons are ASCII case-insensitive.

## Schema templating

Format-string placeholders in your queries are stripped before
validation:

- f-strings: `f"SELECT … WHERE id = {x}"` → `WHERE id = 1`
- `.format()` strings: same; `{{` and `}}` round-trip as literal braces.

Dynamic table or column names (e.g. `f"SELECT {col} FROM t"`) silently
pass the column check — sqlshield can't reason about names it doesn't
know.

## See also

- [README](https://github.com/davidsmfreire/sqlshield#readme) — overview,
  CLI usage, configuration, editor integration.
- [`sqlshield-lsp`](https://github.com/davidsmfreire/sqlshield/tree/main/sqlshield-lsp) —
  Language Server for inline editor diagnostics.

## License

[MIT](https://github.com/davidsmfreire/sqlshield/blob/main/LICENSE).
