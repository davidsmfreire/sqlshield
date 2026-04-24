//! JOIN ON / USING constraint validation.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

// -------- ON --------

#[test]
fn join_on_valid() {
    let sql = "SELECT u.id FROM users u JOIN receipt r ON u.id = r.user_id";
    assert!(run(sql).is_empty());
}

#[test]
fn join_on_with_unknown_left_column() {
    let sql = "SELECT u.id FROM users u JOIN receipt r ON u.bogus = r.user_id";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`bogus`")), "got: {errs:?}");
}

#[test]
fn join_on_with_unknown_right_column() {
    let sql = "SELECT u.id FROM users u JOIN receipt r ON u.id = r.typo";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`typo`")), "got: {errs:?}");
}

#[test]
fn join_on_with_function_call() {
    let sql = "SELECT u.id FROM users u JOIN receipt r ON LOWER(u.name) = LOWER(r.total)";
    // `total` is on receipt; both sides resolve — valid.
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn multiple_joins_each_on_is_validated() {
    let sql = "
        SELECT u.id FROM users u
        JOIN receipt r1 ON u.id = r1.user_id
        JOIN receipt r2 ON u.id = r2.nonsense
    ";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`nonsense`")));
}

// -------- USING --------

#[test]
fn join_using_valid() {
    let sql = "SELECT users.id FROM users JOIN receipt USING (id)";
    assert!(run(sql).is_empty());
}

#[test]
fn join_using_unknown_column() {
    let sql = "SELECT users.id FROM users JOIN receipt USING (typo)";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`typo`")));
}
