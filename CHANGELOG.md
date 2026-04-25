# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

From the first tagged release onward, this file is maintained by
[release-plz](https://release-plz.ieni.dev).

## [Unreleased]

### Added
- `SqlShieldError` typed error enum (`thiserror`) in place of `Result<T, String>`.
- Module-level rustdoc for `lib`, `finder`, `schema`, `validation`, `error`.
- CI matrix across ubuntu/macos/windows with fmt + clippy + cargo-deny gates.
- `dependabot`, `release-plz`, `cargo-deny`, and `CHANGELOG.md` infrastructure.
- MSRV declared as 1.80 (earliest version supporting `std::sync::LazyLock`).
- Go SQL extractor (`finder/go.rs`): raw / interpreted string literals
  and `fmt.Sprintf`-style verbs.
- JavaScript / TypeScript SQL extractor (`finder/javascript.rs`):
  single-, double-, and template-string literals across `.js`, `.ts`,
  and `.tsx`; `${…}` substitutions stripped before parsing.
- `MERGE INTO … USING … ON … WHEN [NOT] MATCHED` validation
  (`validation/clauses/merge.rs`).
- Postgres-aware identifier folding (`schema::sql::fold_ident`):
  unquoted identifiers fold to lower case, quoted identifiers preserve
  case. Other dialects keep ASCII case-insensitive matching.
- `sqlshield-introspect` crate: live schema reader for Postgres and
  SQLite. Wired into the CLI as `--db-url` (and `db_url` in
  `.sqlshield.toml`); mutually exclusive with `--schema`.
- First-party VS Code extension under `editors/vscode/` wrapping
  `sqlshield-lsp` over stdio.
- LSP filetype coverage extended to `go`, `javascript`, `typescript`,
  and `typescriptreact` alongside `python`, `rust`, and `sql`.

### Changed
- `validate_files` now returns `Result` rather than panicking on schema load.
- Regexes in the finder and lib are compiled once via `std::sync::LazyLock`.
- `validate_files` accepts `&Path` instead of `&PathBuf`.

### Removed
- Duct-tape `REPLACE`-triggered recursion in the finder (no test regressed).

## [0.0.1] — unreleased

Initial pre-release. Basic SELECT/WITH/JOIN validation from Python and Rust
source files.
