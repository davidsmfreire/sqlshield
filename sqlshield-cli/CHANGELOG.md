# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1](https://github.com/davidsmfreire/sqlshield/releases/tag/sqlshield-cli-v0.0.1) - 2026-04-25

### Added

- dialect-aware folding, MERGE, live introspection, VS Code, audit fixes
- *(cli)* .sqlshield.toml configuration file (phase 3)
- *(cli)* JSON output, stdin mode, and split exit codes (phase 3)
- add --dialect CLI flag and dialect threading (phase 2)
- [**breaking**] typed errors, lazy regexes, no panics (phase 1)
- generalize query finder and implement rust query finding
- improve sqlshield lib, add python tests and dev dependencies
- cargo workspace and python bindings

### Other

- rewrite root README + add per-crate READMEs + refresh ROADMAP
- add release-plz, dependabot, cargo-deny (phase 5 safety net)
- harden tooling and CI (phase 0)
