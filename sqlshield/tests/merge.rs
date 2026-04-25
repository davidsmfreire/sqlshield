//! MERGE INTO ... USING ... ON ... WHEN [NOT] MATCHED ... validation.

use sqlshield::{validate_query_with_dialect, Dialect};

const SCHEMA: &str = "
    CREATE TABLE accounts (id INT, balance INT);
    CREATE TABLE updates (id INT, delta INT);
";

#[test]
fn merge_with_known_tables_and_columns_passes() {
    let q = "
        MERGE INTO accounts a
        USING updates u
        ON a.id = u.id
        WHEN MATCHED THEN UPDATE SET balance = a.balance + u.delta
        WHEN NOT MATCHED THEN INSERT (id, balance) VALUES (u.id, u.delta);
    ";
    let errs = validate_query_with_dialect(q, SCHEMA, Dialect::Snowflake).unwrap();
    assert!(errs.is_empty(), "expected no errors, got: {errs:?}");
}

#[test]
fn merge_unknown_target_table_is_flagged() {
    let q = "
        MERGE INTO ghosts a
        USING updates u
        ON a.id = u.id
        WHEN MATCHED THEN UPDATE SET balance = u.delta;
    ";
    let errs = validate_query_with_dialect(q, SCHEMA, Dialect::Snowflake).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("ghosts")),
        "expected `ghosts` flagged, got: {errs:?}"
    );
}

#[test]
fn merge_unknown_source_table_is_flagged() {
    let q = "
        MERGE INTO accounts a
        USING ghosts u
        ON a.id = u.id
        WHEN MATCHED THEN UPDATE SET balance = u.delta;
    ";
    let errs = validate_query_with_dialect(q, SCHEMA, Dialect::Snowflake).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("ghosts")),
        "expected `ghosts` flagged, got: {errs:?}"
    );
}

#[test]
fn merge_invalid_assignment_column_is_flagged() {
    let q = "
        MERGE INTO accounts a
        USING updates u
        ON a.id = u.id
        WHEN MATCHED THEN UPDATE SET totally_fake = u.delta;
    ";
    let errs = validate_query_with_dialect(q, SCHEMA, Dialect::Snowflake).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("totally_fake")),
        "expected `totally_fake` flagged, got: {errs:?}"
    );
}

#[test]
fn merge_invalid_insert_column_is_flagged() {
    let q = "
        MERGE INTO accounts a
        USING updates u
        ON a.id = u.id
        WHEN NOT MATCHED THEN INSERT (id, totally_fake) VALUES (u.id, u.delta);
    ";
    let errs = validate_query_with_dialect(q, SCHEMA, Dialect::Snowflake).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("totally_fake")),
        "expected `totally_fake` flagged, got: {errs:?}"
    );
}

#[test]
fn merge_on_predicate_unknown_column_flagged() {
    let q = "
        MERGE INTO accounts a
        USING updates u
        ON a.no_such_col = u.id
        WHEN MATCHED THEN UPDATE SET balance = u.delta;
    ";
    let errs = validate_query_with_dialect(q, SCHEMA, Dialect::Snowflake).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("no_such_col")),
        "expected `no_such_col` flagged, got: {errs:?}"
    );
}

#[test]
fn merge_matched_delete_predicate_validates_columns() {
    let q = "
        MERGE INTO accounts a
        USING updates u
        ON a.id = u.id
        WHEN MATCHED AND a.balance < 0 THEN DELETE;
    ";
    let errs = validate_query_with_dialect(q, SCHEMA, Dialect::Snowflake).unwrap();
    assert!(errs.is_empty(), "{errs:?}");
}
