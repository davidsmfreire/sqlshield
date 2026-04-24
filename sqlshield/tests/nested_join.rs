//! TableFactor::NestedJoin — parenthesized join groups in FROM.
//! `extract_derived_from_factors` and `collect_visible_relations`
//! previously stopped at the top-level relation/joins of each
//! TableWithJoins, missing anything inside a NestedJoin wrapper.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
    CREATE TABLE country (id INT, code VARCHAR(2));
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn nested_join_columns_resolve_in_outer_scope() {
    let sql = "
        SELECT u.id
        FROM (users u JOIN receipt r ON r.user_id = u.id)
        JOIN country c ON c.id = u.id
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn unknown_column_inside_nested_join_on_is_reported() {
    let sql = "
        SELECT u.id
        FROM (users u JOIN receipt r ON r.bogus = u.id)
        JOIN country c ON c.id = u.id
    ";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`bogus`")), "got: {errs:?}");
}

#[test]
fn derived_table_inside_nested_join_is_extracted() {
    // Parenthesized join group containing a derived table. The outer
    // reference to `d.id` should resolve against the derived table's
    // projection.
    let sql = "
        SELECT d.id
        FROM ((SELECT id FROM users) d JOIN receipt r ON r.user_id = d.id)
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn outer_select_can_qualify_table_from_nested_join() {
    let sql = "
        SELECT r.total
        FROM (users u JOIN receipt r ON r.user_id = u.id)
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}
