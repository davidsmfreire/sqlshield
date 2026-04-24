# Roadmap

## Done

- Schema-aware validation across SELECT / INSERT / UPDATE / DELETE,
  CTEs (incl. `WITH RECURSIVE`), set ops, derived tables, JOIN ON /
  USING, scope-aware subqueries.
- Schema ingestion: `CREATE TABLE`, `ALTER TABLE` (ADD/DROP/RENAME
  COLUMN), `CREATE VIEW`, `CREATE TABLE … AS SELECT`.
- 12 SQL dialects via `--dialect`.
- ASCII case-insensitive identifier matching.
- Output formats: text + JSON; split exit codes; `--stdin` mode.
- `.sqlshield.toml` configuration with CLI override layering.
- Parallel file walker (rayon) with default ignore list.
- Language Server (`sqlshield-lsp`) for inline editor diagnostics
  in `.py` / `.rs` / `.sql`.
- Python bindings (`sqlshield-py`).

## Considering

- **Live database introspection** — connect to Postgres / MySQL /
  Sqlite and read the schema directly, no SQL dump required.
- **Postgres quoted-vs-unquoted identifier folding** — currently we
  treat all identifiers as case-insensitive; a quoted-aware mode
  would match Postgres semantics more precisely.
- **`MERGE` support** — would round out the DML coverage.
- **More language extractors** — Go, TypeScript, Java string literals.
  Each is a small `finder/<lang>.rs` module + tree-sitter grammar.
- **First-party VS Code extension** — currently the LSP is wired via
  generic LSP-client extensions.

## Not planned

- Anything that requires running queries on a live database (parameter
  type-checking against actual table types, constraint validation).
  sqlshield is deliberately a static linter.
