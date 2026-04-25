//! Derived tables in FROM — `SELECT ... FROM (SELECT ...) alias`.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn valid_derived_table() {
    let sql = "SELECT x.id FROM (SELECT id FROM users) x";
    assert!(run(sql).is_empty());
}

#[test]
fn derived_table_inner_missing_column_is_reported() {
    let sql = "SELECT x.id FROM (SELECT email FROM users) x";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`email`")), "got: {errs:?}");
}

#[test]
fn derived_table_inner_missing_table_is_reported() {
    let sql = "SELECT x.id FROM (SELECT id FROM ghosts) x";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`ghosts`")), "got: {errs:?}");
}

#[test]
fn outer_references_derived_column_valid() {
    let sql = "SELECT x.user_id FROM (SELECT user_id FROM receipt) x";
    assert!(run(sql).is_empty());
}

#[test]
fn outer_references_column_not_in_derived_projection() {
    // The derived table only projects `id`; `total` is not visible to the outer query.
    let sql = "SELECT x.total FROM (SELECT id FROM users) x";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`total`")), "got: {errs:?}");
}

#[test]
fn derived_alias_in_where() {
    let sql = "SELECT u.id FROM users u JOIN (SELECT user_id FROM receipt) r ON r.user_id = u.id WHERE r.user_id > 0";
    assert!(run(sql).is_empty());
}

#[test]
fn derived_with_exprwithalias_exposes_alias_name() {
    let sql = "SELECT x.amount FROM (SELECT total AS amount FROM receipt) x";
    assert!(run(sql).is_empty());
}

#[test]
fn nested_derived_tables() {
    let sql = "
        SELECT outer_q.id FROM (
            SELECT inner_q.id FROM (SELECT id FROM users) inner_q
        ) outer_q
    ";
    assert!(run(sql).is_empty());
}
