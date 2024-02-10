mod sql;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

pub type TablesAndColumns = HashMap<String, HashSet<String>>;

pub fn load_schema_from_file(file_path: &Path) -> Result<TablesAndColumns, String> {
    let schema = fs::read(file_path);
    let file_extension = &file_path.extension().unwrap().to_string_lossy();

    match schema {
        Ok(schema) => match file_extension.as_ref() {
            "sql" => sql::load_schema(&schema),
            _ => Err(format!("File not supported {file_extension}")),
        },
        Err(err) => Err(format!(
            "Could not open {:?} due to error: {err}",
            file_path
        )),
    }
}

trait LoadSchema {
    fn to_schema(&self, schema_type: &str) -> Result<TablesAndColumns, String>;
}

impl LoadSchema for String {
    fn to_schema(&self, schema_type: &str) -> Result<TablesAndColumns, String> {
        load_schema(&self.as_bytes(), schema_type)
    }
}

pub fn load_schema(schema: &[u8], schema_type: &str) -> Result<TablesAndColumns, String> {
    match schema_type.as_ref() {
        "sql" => sql::load_schema(&schema),
        _ => Err(format!("Schema type not supported {schema_type}")),
    }
}
