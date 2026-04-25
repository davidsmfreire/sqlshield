//! UNION / INTERSECT / EXCEPT (and ALL variants) validation.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn union_all_valid_branches() {
    let sql = "SELECT id FROM users UNION ALL SELECT id FROM receipt";
    assert!(run(sql).is_empty());
}

#[test]
fn union_left_branch_has_unknown_column() {
    let sql = "SELECT email FROM users UNION SELECT id FROM receipt";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`email`")), "got: {errs:?}");
}

#[test]
fn union_right_branch_has_unknown_column() {
    let sql = "SELECT id FROM users UNION SELECT invoice FROM receipt";
    let errs = run(sql);
    assert!(
        errs.iter().any(|e| e.contains("`invoice`")),
        "got: {errs:?}"
    );
}

#[test]
fn union_both_branches_have_errors() {
    let sql = "SELECT email FROM users UNION SELECT invoice FROM receipt";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`email`")));
    assert!(errs.iter().any(|e| e.contains("`invoice`")));
}

#[test]
fn intersect_valid() {
    let sql = "SELECT id FROM users INTERSECT SELECT user_id FROM receipt";
    assert!(run(sql).is_empty());
}

#[test]
fn except_valid() {
    let sql = "SELECT id FROM users EXCEPT SELECT user_id FROM receipt";
    assert!(run(sql).is_empty());
}

#[test]
fn nested_union() {
    let sql = "
        SELECT id FROM users
        UNION
        SELECT id FROM receipt
        UNION
        SELECT user_id FROM receipt
    ";
    assert!(run(sql).is_empty());
}

#[test]
fn union_inside_cte() {
    let sql = "
        WITH combined AS (
            SELECT id FROM users UNION SELECT user_id FROM receipt
        )
        SELECT id FROM combined
    ";
    let errs = run(sql);
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn union_with_wrong_table_in_left_branch() {
    let sql = "SELECT id FROM ghosts UNION SELECT id FROM users";
    let errs = run(sql);
    assert!(errs.iter().any(|e| e.contains("`ghosts`")));
}

#[test]
fn union_arity_mismatch_is_flagged() {
    let sql = "SELECT id, name FROM users UNION SELECT id FROM receipt";
    let errs = run(sql);
    assert!(
        errs.iter()
            .any(|e| e.to_lowercase().contains("column count")),
        "got: {errs:?}"
    );
}

#[test]
fn union_arity_match_is_clean() {
    let sql = "SELECT id, name FROM users UNION SELECT id, user_id FROM receipt";
    assert!(run(sql).is_empty());
}

#[test]
fn union_arity_skipped_when_branch_has_wildcard() {
    // Wildcard arity is unknown statically; we should not emit a false
    // positive arity error against a wildcard branch.
    let sql = "SELECT * FROM users UNION SELECT id, name FROM users";
    let errs = run(sql);
    assert!(
        !errs
            .iter()
            .any(|e| e.to_lowercase().contains("column count")),
        "got: {errs:?}"
    );
}

#[test]
fn derived_table_in_one_branch_does_not_leak_to_other() {
    // `d` is a derived table in the left branch; the right branch shouldn't
    // be able to see it — and doesn't reference it here, so this is valid.
    let sql = "
        SELECT d.id FROM (SELECT id FROM users) d
        UNION
        SELECT user_id FROM receipt
    ";
    assert!(run(sql).is_empty());
}
