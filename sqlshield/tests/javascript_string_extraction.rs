//! Extraction of SQL strings from JavaScript and TypeScript source —
//! single/double-quoted strings, template literals, and template
//! substitutions.

use sqlshield::finder::find_queries_in_code;

fn extract(source: &str, ext: &str) -> Vec<Vec<sqlparser::ast::Statement>> {
    find_queries_in_code(source.as_bytes(), ext)
        .unwrap()
        .into_iter()
        .map(|q| q.statements)
        .collect()
}

// -------- JavaScript --------

#[test]
fn js_double_quoted_string() {
    let source = r#"
        function f() {
            const _ = "SELECT id FROM users";
        }
    "#;
    let queries = extract(source, "js");
    assert_eq!(queries.len(), 1);
    assert!(!queries[0].is_empty());
}

#[test]
fn js_single_quoted_string() {
    let source = r#"
        function f() {
            const _ = 'SELECT id FROM users';
        }
    "#;
    let queries = extract(source, "js");
    assert_eq!(queries.len(), 1);
}

#[test]
fn js_template_literal() {
    let source = r#"
        function f() {
            const _ = `SELECT id FROM users`;
        }
    "#;
    let queries = extract(source, "js");
    assert_eq!(queries.len(), 1);
}

#[test]
fn js_template_substitution_is_replaced() {
    // ${col} gets swapped for `1` so the placeholder doesn't break parsing.
    let source = r#"
        function f(col) {
            const _ = `SELECT ${col} FROM users WHERE id = ${1}`;
        }
    "#;
    let queries = extract(source, "js");
    assert_eq!(
        queries.len(),
        1,
        "template substitution should not break parse"
    );
}

#[test]
fn js_escaped_quote_preserves_quoted_identifier() {
    let source = r#"
        function f() {
            const _ = "SELECT * FROM \"user\"";
        }
    "#;
    let queries = extract(source, "js");
    assert_eq!(queries.len(), 1);
    let rendered = queries[0][0].to_string();
    assert!(
        rendered.contains(r#""user""#),
        "quoted identifier should survive extraction; got: {rendered}"
    );
}

// -------- TypeScript --------

#[test]
fn ts_double_quoted_string() {
    let source = r#"
        function f(): void {
            const q: string = "SELECT id FROM users";
        }
    "#;
    let queries = extract(source, "ts");
    assert_eq!(queries.len(), 1);
}

#[test]
fn ts_template_literal_with_substitution() {
    let source = r#"
        function f(col: string): void {
            const q: string = `SELECT ${col} FROM users`;
        }
    "#;
    let queries = extract(source, "ts");
    assert_eq!(queries.len(), 1);
}

// -------- TSX --------

#[test]
fn tsx_string_in_component() {
    let source = r#"
        const Component = () => {
            const q = "SELECT id FROM users";
            return <div>{q}</div>;
        };
    "#;
    let queries = extract(source, "tsx");
    assert_eq!(queries.len(), 1);
}
