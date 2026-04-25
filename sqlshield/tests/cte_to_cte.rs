//! CTE-to-CTE references: a CTE later in the WITH list can reference
//! earlier ones, and (for WITH RECURSIVE) a CTE can reference itself.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn second_cte_can_reference_first() {
    let sql = "
        WITH a AS (SELECT id FROM users),
             b AS (SELECT id FROM a)
        SELECT id FROM b
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn third_cte_can_reference_second() {
    let sql = "
        WITH a AS (SELECT id FROM users),
             b AS (SELECT id FROM a),
             c AS (SELECT id FROM b)
        SELECT id FROM c
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn second_cte_referencing_unknown_table_still_errors() {
    let sql = "
        WITH a AS (SELECT id FROM users),
             b AS (SELECT id FROM ghosts)
        SELECT id FROM b
    ";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`ghosts`")), "got: {errs:?}");
}

#[test]
fn explicit_cte_column_list_is_honored() {
    // `WITH renamed(a, b) AS (SELECT id, name FROM users)` declares the CTE's
    // columns as `a` and `b` — references to `renamed.id` should fail, and
    // references to `renamed.a` should succeed.
    let valid = "
        WITH renamed(a, b) AS (SELECT id, name FROM users)
        SELECT a, b FROM renamed
    ";
    assert!(run(valid).is_empty(), "got: {:?}", run(valid));

    let invalid = "
        WITH renamed(a, b) AS (SELECT id, name FROM users)
        SELECT id FROM renamed
    ";
    let errs = run(invalid);
    assert!(
        errs.iter().any(|e| e.contains("`id`")),
        "explicit column list should hide the body's column names; got: {errs:?}"
    );
}

#[test]
fn recursive_cte_can_reference_itself() {
    let sql = "
        WITH RECURSIVE tree(id) AS (
            SELECT id FROM users
            UNION ALL
            SELECT id FROM tree
        )
        SELECT id FROM tree
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn cte_only_visible_to_later_ctes_not_earlier() {
    // `a` is defined AFTER `b` — `b` shouldn't see it. This is CTE order
    // semantics, not a correctness test for us, but the error we emit here
    // should be 'table a not found' in b's body, not silence.
    let sql = "
        WITH b AS (SELECT id FROM a),
             a AS (SELECT id FROM users)
        SELECT id FROM b
    ";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`a`")), "got: {errs:?}");
}
