//! Column-reference validation for WHERE, HAVING, and GROUP BY clauses.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255), age INT);
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

// -------- WHERE --------

#[test]
fn where_with_valid_unqualified_column() {
    assert!(run("SELECT id FROM users WHERE age > 18").is_empty());
}

#[test]
fn where_with_unknown_unqualified_column() {
    let errs = run("SELECT id FROM users WHERE email = 'a'");
    assert!(errs.iter().any(|e| e.contains("`email`")), "got: {errs:?}");
}

#[test]
fn where_with_valid_qualified_column() {
    assert!(run("SELECT u.id FROM users u WHERE u.age > 18").is_empty());
}

#[test]
fn where_with_unknown_qualified_column() {
    let errs = run("SELECT u.id FROM users u WHERE u.email = 'a'");
    assert!(errs
        .iter()
        .any(|e| e.contains("`email`") && e.contains("`users`")));
}

#[test]
fn where_with_binary_and_boolean_chain() {
    let errs = run("SELECT id FROM users WHERE age > 18 AND name = 'a' OR id > 0");
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn where_with_between_is_validated() {
    let errs = run("SELECT id FROM users WHERE age BETWEEN 18 AND 65");
    assert!(errs.is_empty());

    let errs = run("SELECT id FROM users WHERE yearz BETWEEN 18 AND 65");
    assert!(errs.iter().any(|e| e.contains("`yearz`")));
}

#[test]
fn where_with_in_list_is_validated() {
    let errs = run("SELECT id FROM users WHERE age IN (18, 21, 30)");
    assert!(errs.is_empty());

    let errs = run("SELECT id FROM users WHERE nope IN (1, 2)");
    assert!(errs.iter().any(|e| e.contains("`nope`")));
}

#[test]
fn where_with_is_null_is_validated() {
    let errs = run("SELECT id FROM users WHERE name IS NULL");
    assert!(errs.is_empty());

    let errs = run("SELECT id FROM users WHERE nickname IS NOT NULL");
    assert!(errs.iter().any(|e| e.contains("`nickname`")));
}

#[test]
fn where_across_join_valid() {
    let sql = "SELECT u.id FROM users u JOIN receipt r ON r.user_id = u.id WHERE r.total > 100";
    assert!(run(sql).is_empty());
}

#[test]
fn where_across_join_unknown_column() {
    let sql = "SELECT u.id FROM users u JOIN receipt r ON r.user_id = u.id WHERE r.gratuity > 0";
    let errs = run(sql);
    assert!(errs
        .iter()
        .any(|e| e.contains("`gratuity`") && e.contains("`receipt`")));
}

// -------- HAVING --------

#[test]
fn having_with_valid_column() {
    let sql = "SELECT user_id FROM receipt GROUP BY user_id HAVING total > 0";
    assert!(run(sql).is_empty());
}

#[test]
fn having_with_unknown_column() {
    let sql = "SELECT user_id FROM receipt GROUP BY user_id HAVING discount > 0";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`discount`")));
}

// -------- GROUP BY --------

#[test]
fn group_by_with_valid_column() {
    let sql = "SELECT user_id FROM receipt GROUP BY user_id";
    assert!(run(sql).is_empty());
}

#[test]
fn group_by_with_unknown_column() {
    let sql = "SELECT id FROM users GROUP BY department";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`department`")));
}

#[test]
fn group_by_qualified_column_valid() {
    let sql = "SELECT r.user_id FROM receipt r GROUP BY r.user_id";
    assert!(run(sql).is_empty());
}

// -------- edge cases --------

#[test]
fn empty_schema_does_not_cause_spurious_where_errors() {
    let errs = validate_query("SELECT id FROM ghosts WHERE whatever > 1", "").unwrap();
    // Table-not-found is expected; no column errors since we can't know.
    assert!(errs.iter().all(|e| !e.contains("Column")));
}

#[test]
fn unknown_qualifier_is_not_reported_as_column_error() {
    // bogus.id references an alias that doesn't exist: we intentionally don't
    // report it yet to avoid noise; table-level checks will catch the real issue.
    let errs = run("SELECT id FROM users WHERE bogus.id = 1");
    assert!(errs
        .iter()
        .all(|e| !e.contains("`id`") || !e.contains("`bogus`")));
}

#[test]
fn function_call_in_where_does_not_false_positive() {
    // LENGTH(name) references the name column inside a function.
    let errs = run("SELECT id FROM users WHERE LENGTH(name) > 0");
    // LENGTH is a function, not a column; we should not error on it.
    // The `name` inside should validate fine.
    assert!(!errs.iter().any(|e| e.contains("`name`")), "got: {errs:?}");
}
