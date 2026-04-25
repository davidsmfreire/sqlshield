//! Tests for `find_queries_in_code` — the language-agnostic extraction entry point.

use sqlshield::finder::find_queries_in_code;
use sqlshield::SqlShieldError;

#[test]
fn unsupported_extension_returns_typed_error() {
    let err = find_queries_in_code(b"print('hi')", "lua").unwrap_err();
    assert!(matches!(err, SqlShieldError::UnsupportedFileExtension(ext) if ext == "lua"));
}

#[test]
fn python_f_string_placeholders_become_literal_ones() {
    let source = br#"
def load(user_id):
    return db.fetch(f"SELECT name FROM users WHERE id = {user_id}")
"#;
    let queries = find_queries_in_code(source, "py").unwrap();
    assert_eq!(queries.len(), 1);
    // parsed statements should be non-empty
    assert!(!queries[0].statements.is_empty());
}

#[test]
fn python_format_placeholders_are_stripped() {
    let source = br#"
def load(user_id):
    return db.fetch("SELECT name FROM users WHERE id = {user_id}".format(user_id=user_id))
"#;
    let queries = find_queries_in_code(source, "py").unwrap();
    assert_eq!(queries.len(), 1);
}

#[test]
fn rust_format_placeholders_are_stripped() {
    let source = br#"
fn main() {
    let _q = format!("SELECT name FROM users WHERE id = {id}", id = 1);
}
"#;
    let queries = find_queries_in_code(source, "rs").unwrap();
    assert_eq!(queries.len(), 1);
}

#[test]
fn python_non_sql_strings_are_skipped() {
    let source = br#"
print("hello, world")
x = "not a query"
"#;
    let queries = find_queries_in_code(source, "py").unwrap();
    // tree-sitter yields strings but sqlparser rejects non-SQL; result is empty.
    assert!(queries.is_empty());
}

#[test]
fn line_numbers_are_one_based() {
    let source = b"# line 1\n# line 2\nq = \"SELECT id FROM users\"\n";
    let queries = find_queries_in_code(source, "py").unwrap();
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].line, 3);
}
