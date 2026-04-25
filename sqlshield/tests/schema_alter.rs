//! Schema ingestion: ALTER TABLE ADD/DROP/RENAME COLUMN.
//! Real-world schemas combine the original CREATE TABLE with later
//! migrations; ignoring ALTER means under-reporting columns.

use sqlshield::validate_query;

#[test]
fn alter_add_column_makes_column_visible() {
    let schema = "
        CREATE TABLE users (id INT);
        ALTER TABLE users ADD COLUMN email VARCHAR(255);
    ";
    let errs = validate_query("SELECT email FROM users", schema).unwrap();
    assert!(
        errs.is_empty(),
        "ADD COLUMN should register `email`; got: {errs:?}"
    );
}

#[test]
fn alter_drop_column_removes_column() {
    let schema = "
        CREATE TABLE users (id INT, legacy INT);
        ALTER TABLE users DROP COLUMN legacy;
    ";
    let errs = validate_query("SELECT legacy FROM users", schema).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("`legacy`")),
        "DROP COLUMN should make `legacy` unknown; got: {errs:?}"
    );
}

#[test]
fn alter_rename_column_renames() {
    let schema = "
        CREATE TABLE users (id INT, full_name VARCHAR(255));
        ALTER TABLE users RENAME COLUMN full_name TO name;
    ";
    // New name is visible.
    let errs = validate_query("SELECT name FROM users", schema).unwrap();
    assert!(
        errs.is_empty(),
        "RENAME should expose new name; got: {errs:?}"
    );
    // Old name is gone.
    let errs = validate_query("SELECT full_name FROM users", schema).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("`full_name`")),
        "RENAME should drop the old name; got: {errs:?}"
    );
}

#[test]
fn multiple_alters_compose() {
    let schema = "
        CREATE TABLE users (id INT);
        ALTER TABLE users ADD COLUMN name VARCHAR(255);
        ALTER TABLE users ADD COLUMN age INT;
        ALTER TABLE users DROP COLUMN age;
    ";
    let valid = validate_query("SELECT id, name FROM users", schema).unwrap();
    assert!(valid.is_empty(), "got: {valid:?}");
    let invalid = validate_query("SELECT age FROM users", schema).unwrap();
    assert!(invalid.iter().any(|e| e.contains("`age`")));
}

#[test]
fn alter_on_unknown_table_is_silently_ignored() {
    // Liberal: skip ALTERs on tables we don't know yet rather than erroring
    // (the schema file might list operations in dependency order).
    let schema = "
        ALTER TABLE ghost ADD COLUMN x INT;
        CREATE TABLE users (id INT);
    ";
    let errs = validate_query("SELECT id FROM users", schema).unwrap();
    assert!(errs.is_empty());
}
