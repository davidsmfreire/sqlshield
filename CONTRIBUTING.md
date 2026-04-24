# Contributing to sqlshield

Thanks for taking the time. Contributions of any size are welcome.

## Development setup

```sh
make dev-setup   # creates .venv, installs deps, pre-commit hooks, builds wheels
```

or, for Rust-only work:

```sh
cargo build --workspace
cargo test --workspace
```

The Rust toolchain is pinned via `rust-toolchain.toml`. Rustup will install the
right version automatically when you run `cargo` inside the repo.

## Before you open a PR

CI runs the same checks — failing any of these will fail your PR.

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check                       # licenses + RUSTSEC advisories
```

Commits must follow [Conventional Commits](https://www.conventionalcommits.org/)
(enforced by pre-commit). Common prefixes: `feat`, `fix`, `refactor`, `chore`,
`docs`, `test`, `perf`. Use `!` (e.g. `feat!:`) for breaking changes.

## Adding a clause validator

Every SQL clause that needs schema-aware checks implements
`validation::clauses::ClauseValidation` (see `sqlshield/src/validation/clauses/mod.rs`).
The canonical example is `clauses/select.rs`.

Rough recipe:

1. Add `mod <clause>;` in `clauses/mod.rs`.
2. Create `clauses/<clause>.rs` and `impl ClauseValidation for sqlparser::ast::<Type>`.
3. Wire it into `validation::validate_query_with_schema` (or the statement
   dispatch in `validate_statements_with_schema`).
4. Add fixtures under `sqlshield/tests/fixtures/` and a unit test in the module.

## Architecture at a glance

```
Source file (*.py, *.rs)
   │ tree-sitter extracts string literals
   ▼
SQL string (with {…} placeholders replaced by `1`)
   │ sqlparser parses
   ▼
AST (Vec<Statement>)
   │ ClauseValidation walks the tree
   ▼
Vec<SqlValidationError>
```

- `sqlshield` — core library (no I/O concerns beyond reading files to scan)
- `sqlshield-cli` — thin clap-based CLI wrapper
- `sqlshield-py` — PyO3 bindings exposing `validate_query` / `validate_files`
- `sqlshield-lsp` — Language Server for editor integration

## Releases

Releases are automated via [release-plz](https://release-plz.ieni.dev). Merging a
conventional-commit PR into `main` triggers the workflow to open (or update) a
release PR that bumps versions and generates changelog entries. Merging the
release PR tags the commit and publishes to crates.io + PyPI.
