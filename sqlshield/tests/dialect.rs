//! Dialect-aware parsing.

use std::str::FromStr;

use sqlshield::{validate_query_with_dialect, Dialect};

const SCHEMA: &str = "CREATE TABLE users (id INT, name VARCHAR(255));";

#[test]
fn default_dialect_is_generic() {
    assert_eq!(Dialect::default(), Dialect::Generic);
}

#[test]
fn dialect_parses_common_aliases() {
    assert_eq!(Dialect::from_str("postgres").unwrap(), Dialect::Postgres);
    assert_eq!(Dialect::from_str("POSTGRESQL").unwrap(), Dialect::Postgres);
    assert_eq!(Dialect::from_str("pg").unwrap(), Dialect::Postgres);
    assert_eq!(Dialect::from_str("mysql").unwrap(), Dialect::MySql);
    assert_eq!(Dialect::from_str("bq").unwrap(), Dialect::BigQuery);
    assert_eq!(Dialect::from_str("SQLite").unwrap(), Dialect::Sqlite);
}

#[test]
fn unknown_dialect_returns_error() {
    assert!(Dialect::from_str("oracle").is_err());
}

#[test]
fn generic_dialect_validates_select() {
    let errs = validate_query_with_dialect("SELECT id FROM users", SCHEMA, Dialect::Generic)
        .expect("valid");
    assert!(errs.is_empty());
}

#[test]
fn postgres_dialect_accepts_double_colon_cast() {
    // Dialect is threaded all the way through parsing — `expr::type` parses
    // under Postgres.
    let result =
        validate_query_with_dialect("SELECT id::TEXT FROM users", SCHEMA, Dialect::Postgres);
    assert!(result.is_ok(), "postgres should accept ::, got {result:?}");
}

#[test]
fn all_dialects_validate_simple_select_without_error() {
    for d in [
        Dialect::Generic,
        Dialect::Postgres,
        Dialect::MySql,
        Dialect::Sqlite,
        Dialect::MsSql,
        Dialect::Snowflake,
        Dialect::BigQuery,
        Dialect::Redshift,
        Dialect::ClickHouse,
        Dialect::DuckDb,
        Dialect::Hive,
        Dialect::Ansi,
    ] {
        let errs = validate_query_with_dialect("SELECT id FROM users", SCHEMA, d)
            .unwrap_or_else(|e| panic!("{d:?} failed to parse basic SELECT: {e}"));
        assert!(errs.is_empty(), "dialect {d:?} produced: {errs:?}");
    }
}
