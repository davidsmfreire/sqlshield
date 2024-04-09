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
        Ok(schema) => load_schema(&schema, file_extension),
        Err(err) => Err(format!(
            "Could not open {:?} due to error: {err}",
            file_path
        )),
    }
}

pub fn load_schema(schema: &[u8], schema_type: &str) -> Result<TablesAndColumns, String> {
    match schema_type.as_ref() {
        "sql" => sql::load_schema(schema),
        _ => panic!("{}", format!("Schema type not supported {schema_type}")),
    }
}
