//! WITH-clause CTEs visible inside INSERT/UPDATE bodies.
//! sqlparser 0.43 wraps `WITH … INSERT/UPDATE` as Query { with, body:
//! SetExpr::Insert/Update(...) } — the validator must recurse through
//! that path and thread the surrounding extras into the inner DML.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn cte_visible_to_insert_select() {
    let sql = "
        WITH active AS (SELECT id FROM users)
        INSERT INTO receipt (user_id, total) SELECT id, 0 FROM active
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn cte_visible_to_update_where_subquery() {
    let sql = "
        WITH stale AS (SELECT id FROM users)
        UPDATE receipt SET total = 0 WHERE user_id IN (SELECT id FROM stale)
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn bad_column_in_cte_used_by_update_is_reported() {
    let sql = "
        WITH stale AS (SELECT nope FROM users)
        UPDATE receipt SET total = 0 WHERE user_id IN (SELECT nope FROM stale)
    ";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`nope`")), "got: {errs:?}");
}

#[test]
fn insert_inside_with_validates_target_column_list() {
    // CTE is fine. The INSERT itself uses a column that doesn't exist on
    // receipt — currently this error is dropped because SetExpr::Insert
    // isn't dispatched.
    let sql = "
        WITH active AS (SELECT id FROM users)
        INSERT INTO receipt (user_id, nonsense) SELECT id, 0 FROM active
    ";
    let errs = run(sql);
    assert!(
        errs.iter().any(|e| e.contains("`nonsense`")),
        "INSERT body should be validated even when wrapped in WITH; got: {errs:?}"
    );
}

#[test]
fn update_inside_with_validates_assignment_columns() {
    let sql = "
        WITH active AS (SELECT id FROM users)
        UPDATE receipt SET nonsense = 1 WHERE user_id IN (SELECT id FROM active)
    ";
    let errs = run(sql);
    assert!(
        errs.iter().any(|e| e.contains("`nonsense`")),
        "UPDATE body should be validated even when wrapped in WITH; got: {errs:?}"
    );
}

#[test]
fn cte_referencing_unknown_table_inside_insert_with() {
    let sql = "
        WITH bogus AS (SELECT id FROM ghosts)
        INSERT INTO receipt (user_id) SELECT id FROM bogus
    ";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`ghosts`")));
}
