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

### Changed
- `validate_files` now returns `Result` rather than panicking on schema load.
- Regexes in the finder and lib are compiled once via `std::sync::LazyLock`.
- `validate_files` accepts `&Path` instead of `&PathBuf`.

### Removed
- Duct-tape `REPLACE`-triggered recursion in the finder (no test regressed).

## [0.0.1] — unreleased

Initial pre-release. Basic SELECT/WITH/JOIN validation from Python and Rust
source files.
