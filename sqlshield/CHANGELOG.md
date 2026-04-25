# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1](https://github.com/davidsmfreire/sqlshield/releases/tag/sqlshield-v0.0.1) - 2026-04-25

### Added

- Go and JavaScript/TypeScript SQL extractors
- dialect-aware folding, MERGE, live introspection, VS Code, audit fixes
- case-insensitive identifier matching
- *(schema)* ingest CREATE VIEW and CREATE TABLE … AS SELECT
- *(schema)* ingest ALTER TABLE ADD/DROP/RENAME COLUMN
- prune generated/vendored directories from file walker
- validate JOIN ON / USING constraints (findings #3)
- add --dialect CLI flag and dialect threading (phase 2)
- validate UNION / INTERSECT / EXCEPT branches (phase 2)
- validate INSERT / UPDATE / DELETE (phase 2)
- validate subqueries in FROM / derived tables (phase 2)
- support schema-qualified table names (phase 2)
- validate aliased projection items (phase 2)
- validate ORDER BY column references (phase 2)
- validate WHERE/HAVING/GROUP BY column references (phase 2)
- [**breaking**] typed errors, lazy regexes, no panics (phase 1)
- [**breaking**] avoid string cloning
- generalize query finder and implement rust query finding
- improve sqlshield lib, add python tests and dev dependencies
- cargo workspace and python bindings

### Fixed

- walk full projection expressions for column refs
- handle .format() {{ }} escapes in Python string extractor
- recurse into TableFactor::NestedJoin for visible relations + ON walks
- validate WITH-wrapped INSERT/UPDATE bodies + thread CTEs into DML
- unify projection qualifier resolution with WHERE-path (findings #1)
- extract raw and escaped Rust string literals correctly (findings #5)
- thread extras through CTE-to-CTE validation (findings #3)
- scope-aware expression walker (findings #1 and #2)
- better error handling, avoiding panics

### Other

- rewrite root README + add per-crate READMEs + refresh ROADMAP
- parallelize file walker with rayon (phase 8)
- add integration tests for validate_query, finder, schema (phase 4)
- add release-plz, dependabot, cargo-deny (phase 5 safety net)
- harden tooling and CI (phase 0)
- remove nested loop from ast traversal
- rename extract_query_from_node to extract_query_string_from_node
- better schema loading
