//! INSERT / UPDATE / DELETE validation.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255), age INT);
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

// -------- INSERT --------

#[test]
fn insert_valid() {
    let errs = run("INSERT INTO users (id, name) VALUES (1, 'alice')");
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn insert_into_unknown_table_is_reported() {
    let errs = run("INSERT INTO ghosts (id) VALUES (1)");
    assert!(errs.iter().any(|e| e.contains("`ghosts`")));
}

#[test]
fn insert_unknown_column_is_reported() {
    let errs = run("INSERT INTO users (id, email) VALUES (1, 'a@b.c')");
    assert!(errs
        .iter()
        .any(|e| e.contains("`email`") && e.contains("`users`")));
}

#[test]
fn insert_from_select_validates_source() {
    let errs = run("INSERT INTO receipt (user_id, total) SELECT id, age FROM users");
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn insert_from_select_with_unknown_source_column() {
    let errs = run("INSERT INTO receipt (user_id, total) SELECT id, salary FROM users");
    assert!(errs.iter().any(|e| e.contains("`salary`")));
}

// -------- UPDATE --------

#[test]
fn update_valid() {
    let errs = run("UPDATE users SET name = 'bob' WHERE id = 1");
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn update_unknown_table_is_reported() {
    let errs = run("UPDATE ghosts SET name = 'a' WHERE id = 1");
    assert!(errs.iter().any(|e| e.contains("`ghosts`")));
}

#[test]
fn update_unknown_assignment_column_is_reported() {
    let errs = run("UPDATE users SET nickname = 'a' WHERE id = 1");
    assert!(errs
        .iter()
        .any(|e| e.contains("`nickname`") && e.contains("`users`")));
}

#[test]
fn update_where_clause_unknown_column_is_reported() {
    let errs = run("UPDATE users SET name = 'a' WHERE email = 'x'");
    assert!(errs.iter().any(|e| e.contains("`email`")));
}

#[test]
fn update_assignment_rhs_unknown_column_is_reported() {
    let errs = run("UPDATE users SET name = bogus WHERE id = 1");
    assert!(errs.iter().any(|e| e.contains("`bogus`")));
}

// -------- DELETE --------

#[test]
fn delete_valid() {
    let errs = run("DELETE FROM users WHERE id = 1");
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn delete_unknown_table_is_reported() {
    let errs = run("DELETE FROM ghosts WHERE id = 1");
    assert!(errs.iter().any(|e| e.contains("`ghosts`")));
}

#[test]
fn delete_where_clause_unknown_column_is_reported() {
    let errs = run("DELETE FROM users WHERE email = 'x'");
    assert!(errs.iter().any(|e| e.contains("`email`")));
}

#[test]
fn delete_where_clause_qualified_unknown_column() {
    let errs = run("DELETE FROM users u WHERE u.email = 'x'");
    assert!(errs
        .iter()
        .any(|e| e.contains("`email`") && e.contains("`users`")));
}
