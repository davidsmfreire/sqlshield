//! ORDER BY column-reference validation (lives on Query, not Select).

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255), age INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn order_by_valid_unqualified_column() {
    assert!(run("SELECT id FROM users ORDER BY age DESC").is_empty());
}

#[test]
fn order_by_unknown_column() {
    let errs = run("SELECT id FROM users ORDER BY department");
    assert!(
        errs.iter().any(|e| e.contains("`department`")),
        "got: {errs:?}"
    );
}

#[test]
fn order_by_valid_qualified_column() {
    assert!(run("SELECT u.id FROM users u ORDER BY u.name ASC").is_empty());
}

#[test]
fn order_by_unknown_qualified_column() {
    let errs = run("SELECT u.id FROM users u ORDER BY u.email DESC");
    assert!(
        errs.iter()
            .any(|e| e.contains("`email`") && e.contains("`users`")),
        "got: {errs:?}"
    );
}

#[test]
fn order_by_multiple_expressions() {
    assert!(run("SELECT id FROM users ORDER BY age DESC, name ASC").is_empty());
}

#[test]
fn order_by_expression_with_function() {
    // LENGTH(name) — function wrapping a valid column; should not error.
    let errs = run("SELECT id FROM users ORDER BY LENGTH(name) DESC");
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn order_by_empty_schema_is_tolerated() {
    let errs = validate_query("SELECT id FROM ghosts ORDER BY id", "").unwrap();
    assert!(errs.iter().all(|e| !e.contains("Column")));
}
