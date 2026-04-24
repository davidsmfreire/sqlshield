//! Table-driven tests for the `validate_query` public entry point.

use sqlshield::{validate_query, SqlShieldError};

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, content VARCHAR(128));
";

#[test]
fn valid_simple_select_produces_no_errors() {
    let errors = validate_query("SELECT id, name FROM users", SCHEMA).unwrap();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn unknown_table_is_reported() {
    let errors = validate_query("SELECT id FROM ghosts", SCHEMA).unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Table `ghosts` not found"));
}

#[test]
fn unknown_column_is_reported() {
    let errors = validate_query("SELECT email FROM users", SCHEMA).unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Column `email` not found in table `users`"));
}

#[test]
fn join_happy_path() {
    let sql = "SELECT u.name, r.content FROM users u JOIN receipt r ON r.user_id = u.id";
    let errors = validate_query(sql, SCHEMA).unwrap();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn join_with_missing_column() {
    let sql = "SELECT u.email, r.content FROM users u JOIN receipt r ON r.user_id = u.id";
    let errors = validate_query(sql, SCHEMA).unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("`email`"));
    assert!(errors[0].contains("`users`"));
}

#[test]
fn cte_projects_columns_for_outer_query() {
    let sql = "
        WITH sub AS (SELECT user_id, content FROM receipt)
        SELECT s.user_id, u.name FROM users u JOIN sub s ON s.user_id = u.id
    ";
    let errors = validate_query(sql, SCHEMA).unwrap();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn cte_referencing_unknown_table_is_reported() {
    let sql = "
        WITH sub AS (SELECT user_id FROM admin)
        SELECT s.user_id FROM sub s
    ";
    let errors = validate_query(sql, SCHEMA).unwrap();
    assert!(
        errors.iter().any(|e| e.contains("`admin`")),
        "expected admin table error, got: {errors:?}"
    );
}

#[test]
fn malformed_sql_returns_parse_error() {
    let result = validate_query("SELECT FROM WHERE", SCHEMA);
    assert!(matches!(result, Err(SqlShieldError::SqlParse(_))));
}

#[test]
fn malformed_schema_returns_parse_error() {
    let result = validate_query("SELECT id FROM users", "this is not sql at all {{{");
    assert!(matches!(result, Err(SqlShieldError::SqlParse(_))));
}

#[test]
fn empty_schema_tolerates_any_query_missing_tables() {
    let errors = validate_query("SELECT id FROM users", "").unwrap();
    assert!(errors.iter().any(|e| e.contains("`users`")));
}
