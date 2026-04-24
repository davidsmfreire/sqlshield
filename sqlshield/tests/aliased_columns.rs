//! Validation of aliased projection items (`SELECT expr AS alias`).

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn aliased_valid_column() {
    assert!(run("SELECT name AS username FROM users").is_empty());
}

#[test]
fn aliased_unknown_column_is_reported() {
    let errs = run("SELECT email AS addr FROM users");
    assert!(errs.iter().any(|e| e.contains("`email`")), "got: {errs:?}");
}

#[test]
fn aliased_qualified_valid_column() {
    assert!(run("SELECT u.name AS who FROM users u").is_empty());
}

#[test]
fn aliased_qualified_unknown_column() {
    let errs = run("SELECT u.email AS addr FROM users u");
    assert!(errs
        .iter()
        .any(|e| e.contains("`email`") && e.contains("`users`")));
}

#[test]
fn mix_of_aliased_and_unaliased_items() {
    let sql = "SELECT id, name AS who, email AS addr FROM users";
    let errs = run(sql);
    // Only the bad one should be reported.
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("`email`"));
}

#[test]
fn cte_column_can_be_aliased_in_outer_query() {
    let sql = "
        WITH sub AS (SELECT user_id, total FROM receipt)
        SELECT s.total AS amount FROM sub s
    ";
    assert!(run(sql).is_empty());
}
