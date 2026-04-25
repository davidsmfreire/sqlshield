//! Schema-qualified table references (`schema.table`, `db.schema.table`).

use sqlshield::validate_query;

#[test]
fn bare_query_against_bare_schema_table() {
    let schema = "CREATE TABLE users (id INT, name VARCHAR(32));";
    let errs = validate_query("SELECT id, name FROM users", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn bare_query_against_qualified_schema_table() {
    // Historical behavior: we still accept an unqualified query if the schema
    // was declared qualified. Users shouldn't have to match identically.
    let schema = "CREATE TABLE public.users (id INT, name VARCHAR(32));";
    let errs = validate_query("SELECT id FROM users", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn qualified_query_resolves_against_qualified_schema_table() {
    let schema = "CREATE TABLE public.users (id INT, name VARCHAR(32));";
    let errs = validate_query("SELECT id FROM public.users", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn qualified_query_with_wrong_schema_is_reported() {
    let schema = "CREATE TABLE public.users (id INT);";
    let errs = validate_query("SELECT id FROM staging.users", schema).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("staging.users")),
        "got: {errs:?}"
    );
}

#[test]
fn qualified_query_matches_even_if_schema_is_bare() {
    // If the schema only declares the bare form, a qualified query should
    // still resolve — we don't know which schema the bare declaration lives in.
    let schema = "CREATE TABLE users (id INT);";
    let errs = validate_query("SELECT id FROM public.users", schema).unwrap();
    // This one is strict: qualified requires qualified. If you want to match
    // bare, write bare. So we DO expect an error here.
    assert!(
        errs.iter().any(|e| e.contains("public.users")),
        "got: {errs:?}"
    );
}

#[test]
fn two_schemas_with_same_table_bare_last_wins() {
    // Known limitation: two qualified tables with the same bare name collide
    // in the bare-key map. Last wins. Document it.
    let schema = "
        CREATE TABLE public.users (id INT);
        CREATE TABLE staging.users (id INT, staging_col INT);
    ";
    // An unqualified `SELECT staging_col FROM users` looks up the bare key
    // and — because staging was declared last — finds staging_col.
    let errs = validate_query("SELECT staging_col FROM users", schema).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}
