//! Postgres quoted-vs-unquoted identifier folding.
//!
//! Postgres semantics: unquoted identifiers fold to lowercase; quoted
//! identifiers (`"User"`) preserve case. sqlshield mirrors this when
//! `Dialect::Postgres` is selected; under any other dialect identifiers
//! remain ASCII case-insensitive (legacy behavior).

use sqlshield::{validate_query_with_dialect, Dialect};

#[test]
fn postgres_quoted_create_only_resolves_quoted_query() {
    // CREATE TABLE "Users" → table key is exactly `Users`. An unquoted
    // `users` reference must fold to `users` and miss.
    let schema = r#"CREATE TABLE "Users" ("Id" INT, "Name" VARCHAR(64));"#;

    let errs =
        validate_query_with_dialect(r#"SELECT "Id" FROM "Users""#, schema, Dialect::Postgres)
            .unwrap();
    assert!(errs.is_empty(), "{errs:?}");

    let errs =
        validate_query_with_dialect("SELECT id FROM users", schema, Dialect::Postgres).unwrap();
    assert!(
        errs.iter().any(|e| e.contains("users")),
        "expected unqualified `users` to miss `Users`, got: {errs:?}"
    );
}

#[test]
fn postgres_unquoted_ident_lowercased_at_lookup() {
    // CREATE TABLE Users (no quotes) → key folds to `users`. Both `USERS`
    // and `users` resolve.
    let schema = "CREATE TABLE Users (Id INT);";
    let errs =
        validate_query_with_dialect("SELECT Id FROM USERS", schema, Dialect::Postgres).unwrap();
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn postgres_quoted_column_in_unquoted_table() {
    // Mixed: unquoted table, quoted column. Schema stores `users` and
    // exact `Email` (case preserved). Query must use `"Email"` to hit it.
    let schema = r#"CREATE TABLE users (id INT, "Email" VARCHAR(255));"#;

    let ok = validate_query_with_dialect(r#"SELECT "Email" FROM users"#, schema, Dialect::Postgres)
        .unwrap();
    assert!(ok.is_empty(), "{ok:?}");

    let bad =
        validate_query_with_dialect("SELECT email FROM users", schema, Dialect::Postgres).unwrap();
    assert!(
        bad.iter().any(|e| e.contains("email")),
        "expected unquoted `email` to miss `Email`, got: {bad:?}"
    );
}

#[test]
fn non_postgres_dialect_keeps_legacy_ci_behavior() {
    // MySQL never gets quote-aware folding — quoted identifiers still fold
    // to lowercase, matching the pre-feature behavior.
    let schema = r#"CREATE TABLE "Users" ("Email" VARCHAR(255));"#;
    let errs =
        validate_query_with_dialect("SELECT email FROM users", schema, Dialect::MySql).unwrap();
    assert!(errs.is_empty(), "{errs:?}");
}
