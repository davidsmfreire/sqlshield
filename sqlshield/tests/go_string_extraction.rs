//! Extraction of SQL strings from Go source — interpreted strings (`"…"`),
//! raw strings (`` `…` ``), and `fmt.Sprintf`-style format verbs.

use sqlshield::finder::find_queries_in_code;

fn extract(source: &str) -> Vec<Vec<sqlparser::ast::Statement>> {
    find_queries_in_code(source.as_bytes(), "go")
        .unwrap()
        .into_iter()
        .map(|q| q.statements)
        .collect()
}

#[test]
fn interpreted_string_is_extracted() {
    let source = r#"
        package main
        func f() {
            _ = "SELECT id FROM users"
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    assert!(!queries[0].is_empty());
}

#[test]
fn interpreted_string_with_escaped_quotes_preserves_them() {
    let source = r#"
        package main
        func f() {
            _ = "SELECT * FROM \"user\""
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    let rendered = queries[0][0].to_string();
    assert!(
        rendered.contains(r#""user""#),
        "quoted identifier should survive extraction; got: {rendered}"
    );
}

#[test]
fn raw_string_is_extracted() {
    let source = r#"
        package main
        func f() {
            _ = `SELECT id FROM users`
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    assert!(!queries[0].is_empty());
}

#[test]
fn raw_string_with_embedded_quotes_preserves_them() {
    let source = r#"
        package main
        func f() {
            _ = `SELECT * FROM "user"`
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    let rendered = queries[0][0].to_string();
    assert!(
        rendered.contains(r#""user""#),
        "quoted identifier should survive raw-string extraction; got: {rendered}"
    );
}

#[test]
fn fmt_sprintf_format_verbs_are_replaced() {
    // `%s` gets swapped for `1` so the placeholder doesn't break SQL parsing.
    let source = r#"
        package main
        import "fmt"
        func f() {
            _ = fmt.Sprintf("SELECT %s FROM users WHERE id = %d", "name", 1)
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1, "expected the format string to parse");
}

#[test]
fn double_percent_is_preserved_as_literal() {
    let source = r#"
        package main
        func f() {
            _ = "SELECT name FROM users WHERE name LIKE 'a%%'"
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
}
