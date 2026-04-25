//! Schema ingestion: CREATE VIEW (with + without explicit column list)
//! and CREATE TABLE … AS SELECT.

use sqlshield::validate_query;

#[test]
fn view_with_explicit_column_list_publishes_those_names() {
    let schema = "
        CREATE TABLE users (id INT, name VARCHAR(255));
        CREATE VIEW user_summary (uid, full_name) AS
            SELECT id, name FROM users;
    ";
    // Explicit column names are visible.
    let valid = validate_query("SELECT uid, full_name FROM user_summary", schema).unwrap();
    assert!(valid.is_empty(), "got: {valid:?}");

    // The body's original column names are NOT visible through the view.
    let invalid = validate_query("SELECT id FROM user_summary", schema).unwrap();
    assert!(
        invalid.iter().any(|e| e.contains("`id`")),
        "got: {invalid:?}"
    );
}

#[test]
fn view_without_explicit_columns_infers_from_body() {
    let schema = "
        CREATE TABLE users (id INT, name VARCHAR(255));
        CREATE VIEW active_users AS SELECT id, name FROM users;
    ";
    let valid = validate_query("SELECT id, name FROM active_users", schema).unwrap();
    assert!(valid.is_empty(), "got: {valid:?}");

    let invalid = validate_query("SELECT email FROM active_users", schema).unwrap();
    assert!(
        invalid.iter().any(|e| e.contains("`email`")),
        "got: {invalid:?}"
    );
}

#[test]
fn view_with_aliased_projection() {
    // `SELECT id AS uid` projects `uid` (alias) — the view's column should be
    // `uid`, not the underlying `id`.
    let schema = "
        CREATE TABLE users (id INT);
        CREATE VIEW renamed AS SELECT id AS uid FROM users;
    ";
    let valid = validate_query("SELECT uid FROM renamed", schema).unwrap();
    assert!(valid.is_empty(), "got: {valid:?}");
}

#[test]
fn create_table_as_select_publishes_projected_columns() {
    let schema = "
        CREATE TABLE users (id INT, name VARCHAR(255));
        CREATE TABLE archive AS SELECT id, name FROM users;
    ";
    let valid = validate_query("SELECT id, name FROM archive", schema).unwrap();
    assert!(valid.is_empty(), "got: {valid:?}");

    let invalid = validate_query("SELECT email FROM archive", schema).unwrap();
    assert!(invalid.iter().any(|e| e.contains("`email`")));
}

#[test]
fn view_can_be_referenced_by_qualified_name() {
    let schema = "
        CREATE TABLE users (id INT);
        CREATE VIEW analytics.user_view AS SELECT id FROM users;
    ";
    let valid = validate_query("SELECT id FROM analytics.user_view", schema).unwrap();
    assert!(valid.is_empty(), "got: {valid:?}");
}
