//! Python string-template extraction edge cases — escaped braces in
//! `.format()` and f-strings.

use sqlshield::finder::find_queries_in_code;

fn extract(source: &str) -> Vec<Vec<sqlparser::ast::Statement>> {
    find_queries_in_code(source.as_bytes(), "py")
        .unwrap()
        .into_iter()
        .map(|q| q.statements)
        .collect()
}

#[test]
fn single_placeholder_is_replaced_with_literal_one() {
    // Existing behavior: `{x}` substitutes for a value-position placeholder.
    let source = r#"q = "SELECT name FROM users WHERE id = {x}".format(x=1)"#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    // The query should parse and contain the substituted `1`.
    let rendered = queries[0][0].to_string();
    assert!(
        rendered.contains("id = 1"),
        "expected substitution; got: {rendered}"
    );
}

#[test]
fn double_brace_is_preserved_as_literal_brace() {
    // `.format()` uses `{{` for a literal `{`. Without escape handling, the
    // greedy regex eats `{{key}` and leaves a stray `}`, breaking the parse.
    let source = r#"q = "SELECT * FROM users WHERE config = '{{key}}'".format()"#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1, "query should parse cleanly");
    let rendered = queries[0][0].to_string();
    assert!(
        rendered.contains("'{key}'"),
        "double-brace should round-trip to single brace; got: {rendered}"
    );
}

#[test]
fn mixed_escapes_and_placeholders() {
    // `{{literal}}` next to `{real}`: literal preserved, real substituted.
    let source = r#"q = "SELECT * FROM users WHERE meta = '{{tag}}' AND id = {x}".format(x=1)"#;
    let queries = extract(source);
    assert_eq!(queries.len(), 1);
    let rendered = queries[0][0].to_string();
    assert!(rendered.contains("'{tag}'"));
    assert!(rendered.contains("id = 1"));
}
