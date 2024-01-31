mod sql;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

pub type TablesAndColumns = HashMap<String, HashSet<String>>;

pub fn load_schema(file_path: &Path) -> Result<TablesAndColumns, String> {
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
