# Roadmap

## Done

- Schema-aware validation across SELECT / INSERT / UPDATE / DELETE /
  MERGE, CTEs (incl. `WITH RECURSIVE`), set ops, derived tables,
  JOIN ON / USING / NATURAL, scope-aware subqueries.
- Schema ingestion: `CREATE TABLE`, `ALTER TABLE` (ADD/DROP/RENAME
  COLUMN), `CREATE VIEW`, `CREATE TABLE … AS SELECT`.
- 12 SQL dialects via `--dialect`; Postgres quoted-vs-unquoted
  identifier folding (other dialects keep ASCII case-insensitive
  matching).
- 3-part qualified column refs (`schema.table.col`).
- Ambiguity detection for unqualified columns visible in 2+ relations.
- Set-op (UNION / INTERSECT / EXCEPT) column-arity check.
- `SELECT *` / `SELECT t.*` projection expansion in CTEs and derived
  tables (resolves outer references and outer `ORDER BY`).
- Output formats: text + JSON; split exit codes; `--stdin` mode.
- `.sqlshield.toml` configuration with CLI override layering.
- Parallel file walker (rayon) with default ignore list.
- Language Server (`sqlshield-lsp`) for inline editor diagnostics
  in `.py` / `.rs` / `.sql`; auto-reload on schema-file changes.
- First-party VS Code extension (`editors/vscode`) wrapping
  `sqlshield-lsp` over stdio.
- Live database introspection (`sqlshield-introspect`, exposed via
  `--db-url`): read schema directly from a running Postgres or SQLite
  instance.
- Python bindings (`sqlshield-py`).

## Considering

- **MySQL live introspection** — pending: `mysql_common` uses unstable
  Rust features that haven't reached the project's pinned toolchain.
  A toolchain bump or a different sync driver would unblock this.
- **More language extractors** — Go, TypeScript, Java string literals.
  Each is a small `finder/<lang>.rs` module + tree-sitter grammar.

## Not planned

- Anything that requires running queries on a live database (parameter
  type-checking against actual table types, constraint validation).
  sqlshield is deliberately a static linter.
