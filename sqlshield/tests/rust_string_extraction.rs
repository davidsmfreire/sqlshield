//! Extraction of SQL strings from Rust source — regular strings with
//! embedded quotes, and raw strings (`r#"…"#`) used by sqlx-style macros.

use sqlshield::finder::find_queries_in_code;

fn extract(source: &str) -> Vec<Vec<sqlparser::ast::Statement>> {
    find_queries_in_code(source.as_bytes(), "rs")
        .unwrap()
        .into_iter()
        .map(|q| q.statements)
        .collect()
}

#[test]
fn regular_string_is_extracted() {
    let source = r#"
        fn f() {
            let _ = "SELECT id FROM users";
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    assert!(!queries[0].is_empty());
}

#[test]
fn regular_string_with_escaped_quotes_preserves_them() {
    // The Rust source contains `\"user\"`; after extraction this should be
    // the SQL `"user"` (a quoted identifier), not `user` with the quotes
    // silently dropped.
    let source = r#"
        fn f() {
            let _ = "SELECT * FROM \"user\"";
        }
    "#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    // Re-render the parsed AST as SQL and assert the quoted identifier survived.
    let rendered = queries[0][0].to_string();
    assert!(
        rendered.contains(r#""user""#),
        "quoted identifier should survive extraction; got: {rendered}"
    );
}

#[test]
fn raw_string_is_extracted() {
    let source = r##"
        fn f() {
            let _ = r#"SELECT id FROM users"#;
        }
    "##;
    let queries = extract(source);
    assert_eq!(
        queries.len(),
        1,
        "raw string was silently dropped by the extractor"
    );
    assert!(!queries[0].is_empty());
}

#[test]
fn raw_string_with_embedded_quotes_preserves_them() {
    // sqlx-idiomatic: raw string literal containing a quoted identifier.
    let source = r##"
        fn f() {
            let _ = r#"SELECT * FROM "user""#;
        }
    "##;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    let rendered = queries[0][0].to_string();
    assert!(
        rendered.contains(r#""user""#),
        "quoted identifier should survive raw-string extraction; got: {rendered}"
    );
}
