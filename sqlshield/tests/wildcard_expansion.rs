//! `SELECT *` / `SELECT t.*` projection expansion in CTEs and derived
//! tables. The validator looks through the wildcard to the inner FROM
//! relations so outer references can resolve.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(64));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

// -------- CTEs --------

#[test]
fn cte_with_wildcard_exposes_inner_columns() {
    let errs = run("WITH t AS (SELECT * FROM users) SELECT id FROM t");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn cte_with_wildcard_qualified_outer_ref() {
    let errs = run("WITH t AS (SELECT * FROM users) SELECT t.name FROM t");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn cte_with_wildcard_outer_ref_to_missing_column_is_flagged() {
    let errs = run("WITH t AS (SELECT * FROM users) SELECT bogus FROM t");
    assert!(
        errs.iter().any(|e| e.contains("`bogus`")),
        "expected `bogus` flagged, got: {errs:?}"
    );
}

#[test]
fn cte_with_qualified_wildcard_only_exposes_that_relation() {
    let errs = run(
        "WITH t AS (SELECT u.* FROM users u JOIN receipt r ON u.id = r.user_id)
         SELECT name FROM t",
    );
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn cte_with_qualified_wildcard_blocks_other_relation_columns() {
    // `total` is on receipt; `u.*` exposes only users' columns.
    let errs = run(
        "WITH t AS (SELECT u.* FROM users u JOIN receipt r ON u.id = r.user_id)
         SELECT total FROM t",
    );
    assert!(
        errs.iter().any(|e| e.contains("`total`")),
        "expected `total` flagged, got: {errs:?}"
    );
}

// -------- Derived tables --------

#[test]
fn derived_table_with_wildcard_exposes_inner_columns() {
    let errs = run("SELECT id FROM (SELECT * FROM users) d");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn derived_table_with_wildcard_qualified_outer_ref() {
    let errs = run("SELECT d.name FROM (SELECT * FROM users) d");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn derived_table_with_wildcard_missing_column_is_flagged() {
    let errs = run("SELECT bogus FROM (SELECT * FROM users) d");
    assert!(
        errs.iter().any(|e| e.contains("`bogus`")),
        "expected `bogus` flagged, got: {errs:?}"
    );
}

// -------- Multi-table wildcards --------

#[test]
fn cte_with_wildcard_over_join_exposes_both_relations() {
    let errs = run(
        "WITH t AS (SELECT * FROM users u JOIN receipt r ON u.id = r.user_id)
         SELECT name, total FROM t",
    );
    assert!(errs.is_empty(), "{errs:?}");
}

// -------- Nested --------

#[test]
fn nested_wildcards_propagate() {
    let errs = run("WITH t AS (SELECT * FROM (SELECT * FROM users) d)
         SELECT name FROM t");
    assert!(errs.is_empty(), "{errs:?}");
}

// -------- ORDER BY against wildcard subquery --------

#[test]
fn outer_order_by_resolves_through_inner_wildcard() {
    // The case originally called out in the ROADMAP: a wildcard inner
    // SELECT used to leave the outer ORDER BY without any visible columns.
    let errs = run("SELECT * FROM (SELECT * FROM users) d ORDER BY d.name");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn outer_order_by_against_inner_wildcard_unknown_column_flagged() {
    let errs = run("SELECT * FROM (SELECT * FROM users) d ORDER BY d.bogus");
    assert!(
        errs.iter().any(|e| e.contains("`bogus`")),
        "expected `bogus` flagged, got: {errs:?}"
    );
}
