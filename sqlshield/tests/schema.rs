//! Tests for schema loading edge cases.

use sqlshield::schema::{load_schema, load_schema_from_file};
use sqlshield::SqlShieldError;

#[test]
fn unsupported_schema_type_returns_typed_error() {
    let err = load_schema(b"", "json").unwrap_err();
    assert!(matches!(err, SqlShieldError::UnsupportedSchemaType(s) if s == "json"));
}

#[test]
fn malformed_schema_sql_returns_parse_error() {
    let err = load_schema(b"CREATE TABLE [[[", "sql").unwrap_err();
    assert!(matches!(err, SqlShieldError::SqlParse(_)));
}

#[test]
fn empty_schema_loads_to_empty_map() {
    let tables = load_schema(b"", "sql").unwrap();
    assert!(tables.is_empty());
}

#[test]
fn schema_parses_multiple_tables() {
    let schema = b"
        CREATE TABLE users (id INT, name VARCHAR(32));
        CREATE TABLE receipt (id INT, user_id INT);
    ";
    let tables = load_schema(schema, "sql").unwrap();
    assert_eq!(tables.len(), 2);
    assert!(tables.contains_key("users"));
    assert!(tables.contains_key("receipt"));
    let users = &tables["users"];
    assert!(users.contains("id"));
    assert!(users.contains("name"));
}

#[test]
fn missing_file_returns_io_error() {
    let err = load_schema_from_file(std::path::Path::new("/nonexistent/schema.sql")).unwrap_err();
    assert!(matches!(err, SqlShieldError::Io { .. }));
}

#[test]
fn file_without_extension_returns_missing_extension_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema_no_ext");
    std::fs::write(&path, b"CREATE TABLE t (id INT);").unwrap();

    let err = load_schema_from_file(&path).unwrap_err();
    assert!(matches!(err, SqlShieldError::MissingExtension(_)));
}
