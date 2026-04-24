//! Regression tests for scope-aware expression validation.
//!
//! These exercise two bugs in the first-pass WHERE/HAVING/ORDER BY visitor:
//! 1. Identifiers inside subqueries (IN / EXISTS / scalar) were being resolved
//!    against the OUTER relations, producing spurious "column not found" errors.
//! 2. Projection-alias references in HAVING / ORDER BY / GROUP BY were
//!    treated as unknown columns.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

// -------- subquery scope --------

#[test]
fn in_subquery_resolves_in_its_own_scope() {
    let sql = "SELECT id FROM users WHERE id IN (SELECT user_id FROM receipt)";
    assert!(run(sql).is_empty(), "got: {:?}", run(sql));
}

#[test]
fn exists_subquery_resolves_in_its_own_scope() {
    let sql =
        "SELECT id FROM users u WHERE EXISTS (SELECT 1 FROM receipt r WHERE r.user_id = u.id)";
    assert!(run(sql).is_empty(), "got: {:?}", run(sql));
}

#[test]
fn scalar_subquery_resolves_in_its_own_scope() {
    let sql = "SELECT id, (SELECT total FROM receipt WHERE receipt.user_id = users.id) FROM users";
    // Outer column `id` is fine; inner subquery's `total` should resolve
    // against `receipt`, not `users`.
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn in_subquery_with_unknown_inner_column_is_still_reported() {
    // Inner subquery references a column that doesn't exist — should error.
    let sql = "SELECT id FROM users WHERE id IN (SELECT nope FROM receipt)";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`nope`")), "got: {errs:?}");
}

#[test]
fn in_subquery_with_unknown_inner_table_is_still_reported() {
    let sql = "SELECT id FROM users WHERE id IN (SELECT id FROM ghosts)";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`ghosts`")));
}

// -------- projection aliases --------

#[test]
fn having_can_reference_projection_alias() {
    let sql = "SELECT user_id, COUNT(*) AS n FROM receipt GROUP BY user_id HAVING n > 5";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn order_by_can_reference_projection_alias() {
    let sql = "SELECT id AS pk FROM users ORDER BY pk DESC";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn group_by_can_reference_projection_alias() {
    let sql = "SELECT user_id AS owner, total FROM receipt GROUP BY owner, total";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn alias_reference_still_flags_truly_missing_columns() {
    // `missing` isn't an alias or a real column — should still error.
    let sql = "SELECT id AS pk FROM users ORDER BY missing";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`missing`")));
}
