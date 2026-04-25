//! Regression for finding #1 ([OPEN]) — projection qualifier path was
//! inconsistent with the WHERE-path resolver. `SELECT users.bogus FROM users`
//! (unaliased table, qualified column) used to silently pass in the
//! projection while WHERE flagged the same reference.

use sqlshield::validate_query;

const SCHEMA: &str = "
    CREATE TABLE users (id INT, name VARCHAR(255));
    CREATE TABLE receipt (id INT, user_id INT, total INT);
";

fn run(sql: &str) -> Vec<String> {
    validate_query(sql, SCHEMA).expect("SQL/schema should parse")
}

#[test]
fn qualified_projection_with_table_name_on_unaliased_table() {
    // The bug: `users` is the table name, no alias. The projection used to
    // skip checking `users.bogus` because the (None alias, Some col-qualifier)
    // case fell through to "don't check".
    let errs = run("SELECT users.bogus FROM users");
    assert!(
        errs.iter()
            .any(|e| e.contains("`bogus`") && e.contains("`users`")),
        "got: {errs:?}"
    );
}

#[test]
fn qualified_projection_with_table_name_on_unaliased_table_valid() {
    // Confirm we didn't regress the happy path.
    let errs = run("SELECT users.id FROM users");
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn unaliased_table_qualified_projection_with_aliased_other_side() {
    // Mixing styles in a JOIN: one side bare, one side aliased.
    let errs = run("SELECT users.bogus, r.total FROM users JOIN receipt r ON users.id = r.user_id");
    assert!(
        errs.iter()
            .any(|e| e.contains("`bogus`") && e.contains("`users`")),
        "got: {errs:?}"
    );
}

#[test]
fn alias_qualifier_still_works_for_projection() {
    // Existing behavior: aliased table with matching column qualifier.
    let errs = run("SELECT u.email FROM users u");
    assert!(
        errs.iter()
            .any(|e| e.contains("`email`") && e.contains("`users`")),
        "got: {errs:?}"
    );
}

#[test]
fn three_part_qualified_column_unknown_is_flagged() {
    // `schema.table.col` form: the table-qualifier is the second segment.
    // Validates against a schema-qualified CREATE TABLE so the FROM clause
    // resolves; the projection uses the 3-part form.
    let qualified_schema = "CREATE TABLE public.users (id INT, name VARCHAR(64));";
    let errs = sqlshield::validate_query(
        "SELECT public.users.bogus FROM public.users",
        qualified_schema,
    )
    .expect("SQL/schema should parse");
    assert!(
        errs.iter()
            .any(|e| e.contains("`bogus`") && e.contains("`users`")),
        "got: {errs:?}"
    );
}

#[test]
fn three_part_qualified_column_valid() {
    let qualified_schema = "CREATE TABLE public.users (id INT, name VARCHAR(64));";
    let errs =
        sqlshield::validate_query("SELECT public.users.id FROM public.users", qualified_schema)
            .expect("SQL/schema should parse");
    assert!(errs.is_empty(), "got: {errs:?}");
}
