//! Parses schema definitions into the `TablesAndColumns` map consumed by validation.

pub(crate) mod sql;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use crate::error::{Result, SqlShieldError};

pub type TablesAndColumns = HashMap<String, HashSet<String>>;

pub fn load_schema_from_file(file_path: &Path) -> Result<TablesAndColumns> {
    let file_extension = file_path
        .extension()
        .ok_or_else(|| SqlShieldError::MissingExtension(file_path.to_path_buf()))?
        .to_string_lossy();

    let schema = fs::read(file_path).map_err(|source| SqlShieldError::Io {
        path: file_path.to_path_buf(),
        source,
    })?;

    load_schema(&schema, &file_extension)
}

pub fn load_schema(schema: &[u8], schema_type: &str) -> Result<TablesAndColumns> {
    match schema_type {
        "sql" => sql::load_schema(schema),
        other => Err(SqlShieldError::UnsupportedSchemaType(other.to_string())),
    }
}
