//! SQL dialect selection. A thin wrapper around `sqlparser::dialect` so the
//! public API doesn't expose the sqlparser types directly.

use std::str::FromStr;

/// Which SQL dialect to parse with. See [sqlparser::dialect] for specifics
/// of each dialect's grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dialect {
    /// Permissive superset useful for heterogeneous codebases (the default).
    #[default]
    Generic,
    Postgres,
    MySql,
    Sqlite,
    MsSql,
    Snowflake,
    BigQuery,
    Redshift,
    ClickHouse,
    DuckDb,
    Hive,
    Ansi,
}

impl Dialect {
    /// Materialize as a boxed `sqlparser::dialect::Dialect`. The box keeps
    /// the return type homogeneous across variants.
    pub fn as_sqlparser(&self) -> Box<dyn sqlparser::dialect::Dialect> {
        match self {
            Self::Generic => Box::new(sqlparser::dialect::GenericDialect {}),
            Self::Postgres => Box::new(sqlparser::dialect::PostgreSqlDialect {}),
            Self::MySql => Box::new(sqlparser::dialect::MySqlDialect {}),
            Self::Sqlite => Box::new(sqlparser::dialect::SQLiteDialect {}),
            Self::MsSql => Box::new(sqlparser::dialect::MsSqlDialect {}),
            Self::Snowflake => Box::new(sqlparser::dialect::SnowflakeDialect {}),
            Self::BigQuery => Box::new(sqlparser::dialect::BigQueryDialect {}),
            Self::Redshift => Box::new(sqlparser::dialect::RedshiftSqlDialect {}),
            Self::ClickHouse => Box::new(sqlparser::dialect::ClickHouseDialect {}),
            Self::DuckDb => Box::new(sqlparser::dialect::DuckDbDialect {}),
            Self::Hive => Box::new(sqlparser::dialect::HiveDialect {}),
            Self::Ansi => Box::new(sqlparser::dialect::AnsiDialect {}),
        }
    }
}

impl FromStr for Dialect {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "generic" => Ok(Self::Generic),
            "postgres" | "postgresql" | "pg" => Ok(Self::Postgres),
            "mysql" => Ok(Self::MySql),
            "sqlite" => Ok(Self::Sqlite),
            "mssql" | "sqlserver" => Ok(Self::MsSql),
            "snowflake" => Ok(Self::Snowflake),
            "bigquery" | "bq" => Ok(Self::BigQuery),
            "redshift" => Ok(Self::Redshift),
            "clickhouse" => Ok(Self::ClickHouse),
            "duckdb" => Ok(Self::DuckDb),
            "hive" => Ok(Self::Hive),
            "ansi" => Ok(Self::Ansi),
            other => Err(format!(
                "unknown dialect `{other}` (expected one of: generic, postgres, mysql, sqlite, mssql, snowflake, bigquery, redshift, clickhouse, duckdb, hive, ansi)"
            )),
        }
    }
}
