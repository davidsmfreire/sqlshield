//! Live database introspection for sqlshield.
//!
//! Connects to a running database and builds a [`TablesAndColumns`] map
//! directly from system catalogs — no SQL dump required. Driver is selected
//! by URL scheme:
//!
//! * `postgres://…` / `postgresql://…` — Postgres via the `postgres` crate.
//! * `sqlite:///path/to/file.db` (or a bare path) — SQLite via `rusqlite`.
//!
//! MySQL support is planned but currently blocked on the `mysql_common`
//! crate's use of unstable Rust features that haven't reached the project's
//! pinned toolchain yet.
//!
//! Each driver is optional behind a Cargo feature; the default build pulls
//! both currently-supported drivers.

use std::collections::HashMap;

use sqlshield::schema::TablesAndColumns;

#[derive(Debug, thiserror::Error)]
pub enum IntrospectError {
    #[error("invalid database URL: {0}")]
    InvalidUrl(String),

    #[error("unsupported database URL scheme `{0}` (expected postgres / sqlite)")]
    UnsupportedScheme(String),

    #[error("driver `{0}` not compiled in (rebuild with the `{0}` feature)")]
    DriverDisabled(&'static str),

    #[error("postgres error: {0}")]
    #[cfg(feature = "postgres")]
    Postgres(#[from] postgres::Error),

    #[error("sqlite error: {0}")]
    #[cfg(feature = "sqlite")]
    Sqlite(#[from] rusqlite::Error),
}

/// Connect to `db_url` and return the live schema. Caller is expected to
/// pass a URL the running process can actually reach; the function returns
/// any driver error directly so callers can show a useful message.
pub fn introspect(db_url: &str) -> Result<TablesAndColumns, IntrospectError> {
    let scheme = url_scheme(db_url);
    match scheme.as_deref() {
        Some("postgres") | Some("postgresql") => introspect_postgres(db_url),
        Some("mysql") => Err(IntrospectError::DriverDisabled("mysql")),
        Some("sqlite") | Some("file") => introspect_sqlite_url(db_url),
        // Bare path → treat as sqlite for convenience.
        None => introspect_sqlite_path(db_url),
        Some(other) => Err(IntrospectError::UnsupportedScheme(other.to_string())),
    }
}

fn url_scheme(s: &str) -> Option<String> {
    // Windows drive letters (`C:\…`) parse as a single-character URL
    // scheme, which would otherwise route a bare path through the
    // unsupported-scheme branch. Treat any single-letter "scheme" as a
    // bare filesystem path.
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return None;
    }
    url::Url::parse(s)
        .ok()
        .map(|u| u.scheme().to_ascii_lowercase())
}

#[cfg(feature = "postgres")]
fn introspect_postgres(db_url: &str) -> Result<TablesAndColumns, IntrospectError> {
    use postgres::{Client, NoTls};

    let mut client = Client::connect(db_url, NoTls)?;
    // information_schema.columns covers user tables plus views, which is
    // what the validator wants. Skip catalog schemas to keep the map lean.
    let rows = client.query(
        "SELECT table_schema, table_name, column_name
         FROM information_schema.columns
         WHERE table_schema NOT IN ('pg_catalog', 'information_schema')",
        &[],
    )?;

    let mut tables: TablesAndColumns = HashMap::new();
    for row in rows {
        let schema: String = row.get(0);
        let table: String = row.get(1);
        let column: String = row.get(2);
        // Postgres returns identifiers already lowercased for unquoted
        // tables. Quoted identifiers come back case-preserved. The
        // validator's PG-mode folding will read both keys correctly.
        let qualified = format!("{schema}.{table}");
        tables.entry(table).or_default().insert(column.clone());
        tables.entry(qualified).or_default().insert(column);
    }
    Ok(tables)
}

#[cfg(not(feature = "postgres"))]
fn introspect_postgres(_db_url: &str) -> Result<TablesAndColumns, IntrospectError> {
    Err(IntrospectError::DriverDisabled("postgres"))
}

#[cfg(feature = "sqlite")]
fn introspect_sqlite_url(db_url: &str) -> Result<TablesAndColumns, IntrospectError> {
    // sqlite:///abs/path or sqlite://relative. Strip scheme+`//` to get
    // the filesystem path; rusqlite opens by path.
    let parsed = url::Url::parse(db_url).map_err(|e| IntrospectError::InvalidUrl(e.to_string()))?;
    let path = parsed.path();
    let path = path.strip_prefix('/').unwrap_or(path);
    introspect_sqlite_path(path)
}

#[cfg(feature = "sqlite")]
fn introspect_sqlite_path(path: &str) -> Result<TablesAndColumns, IntrospectError> {
    let conn = rusqlite::Connection::open(path)?;
    let mut tables: TablesAndColumns = HashMap::new();

    let names: Vec<String> = {
        let mut stmt =
            conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table','view')")?;
        let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in iter {
            out.push(r?);
        }
        out
    };

    for table in names {
        let mut col_stmt = conn.prepare(&format!("PRAGMA table_info(\"{}\")", escape(&table)))?;
        let cols_iter = col_stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut cols = std::collections::HashSet::new();
        for c in cols_iter {
            cols.insert(c?);
        }
        tables.insert(table, cols);
    }
    Ok(tables)
}

#[cfg(not(feature = "sqlite"))]
fn introspect_sqlite_url(_db_url: &str) -> Result<TablesAndColumns, IntrospectError> {
    Err(IntrospectError::DriverDisabled("sqlite"))
}

#[cfg(not(feature = "sqlite"))]
fn introspect_sqlite_path(_path: &str) -> Result<TablesAndColumns, IntrospectError> {
    Err(IntrospectError::DriverDisabled("sqlite"))
}

#[cfg(feature = "sqlite")]
fn escape(s: &str) -> String {
    // PRAGMA table_info() takes an identifier; escape embedded quotes to
    // keep arbitrary table names safe.
    s.replace('"', "\"\"")
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    #[test]
    fn sqlite_in_memory_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER);",
        )
        .unwrap();
        drop(conn);

        let tables = introspect(path.to_str().unwrap()).unwrap();
        assert!(tables.contains_key("users"), "got: {tables:?}");
        assert!(tables.contains_key("orders"));
        assert!(tables["users"].contains("name"));
        assert!(tables["orders"].contains("user_id"));
    }

    #[test]
    fn windows_drive_letter_is_not_treated_as_url_scheme() {
        assert_eq!(url_scheme("C:\\foo\\bar.db"), None);
        assert_eq!(url_scheme("d:/relative.sqlite"), None);
        assert_eq!(url_scheme("postgres://x/y").as_deref(), Some("postgres"));
        assert_eq!(url_scheme("/abs/path.db"), None);
    }
}
