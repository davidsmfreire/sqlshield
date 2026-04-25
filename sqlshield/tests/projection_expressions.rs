//! Projection items beyond bare identifiers — function calls, CASE, CAST,
//! arithmetic. The WHERE-side walker already covers these; previously the
//! projection check used a `direct_col_ref` shortcut that only matched
//! Identifier / 2-segment CompoundIdentifier and silently passed everything
//! else.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255), age INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn function_call_on_valid_column() {
    assert!(run("SELECT LENGTH(name) FROM users").is_empty());
}

#[test]
fn function_call_on_unknown_column_is_reported() {
    let errs = run("SELECT LENGTH(bogus) FROM users");
    assert!(errs.iter().any(|e| e.contains("`bogus`")), "got: {errs:?}");
}

#[test]
fn case_when_branch_unknown_column_is_reported() {
    let errs = run("SELECT CASE WHEN id > 0 THEN bogus ELSE name END FROM users");
    assert!(errs.iter().any(|e| e.contains("`bogus`")), "got: {errs:?}");
}

#[test]
fn case_condition_unknown_column_is_reported() {
    let errs = run("SELECT CASE WHEN typo > 0 THEN name END FROM users");
    assert!(errs.iter().any(|e| e.contains("`typo`")), "got: {errs:?}");
}

#[test]
fn cast_unknown_column_is_reported() {
    let errs = run("SELECT CAST(bogus AS TEXT) FROM users");
    assert!(errs.iter().any(|e| e.contains("`bogus`")), "got: {errs:?}");
}

#[test]
fn arithmetic_unknown_column_is_reported() {
    let errs = run("SELECT id + bogus FROM users");
    assert!(errs.iter().any(|e| e.contains("`bogus`")), "got: {errs:?}");
}

#[test]
fn nested_function_call() {
    let errs = run("SELECT LENGTH(UPPER(bogus)) FROM users");
    assert!(errs.iter().any(|e| e.contains("`bogus`")), "got: {errs:?}");
}

#[test]
fn function_with_aliased_qualifier_resolves() {
    // `u.name` inside a function — qualified identifier in nested context.
    assert!(run("SELECT LENGTH(u.name) FROM users u").is_empty());
}

#[test]
fn function_with_aliased_qualifier_unknown_column() {
    let errs = run("SELECT LENGTH(u.bogus) FROM users u");
    assert!(
        errs.iter()
            .any(|e| e.contains("`bogus`") && e.contains("`users`")),
        "got: {errs:?}"
    );
}

#[test]
fn count_star_does_not_error() {
    assert!(run("SELECT COUNT(*) FROM users").is_empty());
}
