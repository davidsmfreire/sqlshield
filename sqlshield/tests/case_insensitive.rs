//! Identifier matching is case-insensitive (ASCII fold) — the schema-vs-
//! query case mismatch was a stream of false positives.

use sqlshield::validate_query;

#[test]
fn lowercase_schema_uppercase_query() {
    let schema = "CREATE TABLE users (id INT, name VARCHAR(255));";
    let errs = validate_query("SELECT ID, NAME FROM USERS", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn uppercase_schema_lowercase_query() {
    let schema = "CREATE TABLE Users (Id INT, FullName VARCHAR(255));";
    let errs = validate_query("SELECT id, fullname FROM users", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn mixed_case_qualifier_resolves() {
    let schema = "CREATE TABLE users (id INT);";
    let errs = validate_query("SELECT U.Id FROM Users U", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn truly_missing_column_still_errors() {
    let schema = "CREATE TABLE users (id INT);";
    let errs = validate_query("SELECT BOGUS FROM users", schema).unwrap();
    // Error message reports the column name as the user wrote it.
    assert!(errs.iter().any(|e| e.contains("BOGUS")), "got: {errs:?}");
}

#[test]
fn cte_reference_is_case_insensitive() {
    let schema = "CREATE TABLE users (id INT);";
    let sql = "WITH MyCTE AS (SELECT id FROM users) SELECT mycte.id FROM MYCTE";
    let errs = validate_query(sql, schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn alter_table_case_insensitive() {
    let schema = "
        CREATE TABLE users (id INT);
        ALTER TABLE Users ADD COLUMN Email VARCHAR(255);
    ";
    let errs = validate_query("SELECT email FROM users", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn schema_qualified_table_case_insensitive() {
    let schema = "CREATE TABLE Public.Users (id INT);";
    let errs = validate_query("SELECT id FROM public.users", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn insert_target_columns_case_insensitive() {
    let schema = "CREATE TABLE users (id INT, name VARCHAR(255));";
    let errs = validate_query("INSERT INTO Users (ID, NAME) VALUES (1, 'a')", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}
