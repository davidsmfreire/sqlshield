pub mod python;

use std::{fs, path::Path};

use sqlparser;

pub struct QueryInCode {
    pub line: usize,
    pub statements: Vec<sqlparser::ast::Statement>,
}

pub const SUPPORTED_CODE_FILE_EXTENSIONS: [&str; 1] = ["py"];

pub fn find_queries_in_file(file_path: &Path) -> Result<Vec<super::QueryInCode>, String> {
    let code = fs::read(file_path);
    let file_extension = &file_path.extension().unwrap().to_string_lossy();
    match code {
        Ok(code) => find_queries_in_code(&code, &file_extension),
        Err(err) => Err(format!(
            "Could not open {:?} due to error: {err}",
            file_path
        )),
    }
}

pub fn find_queries_in_code(
    code: &[u8],
    file_extension: &str,
) -> Result<Vec<super::QueryInCode>, String> {
    match file_extension.as_ref() {
        "py" => python::find_queries_in_code(&code),
        _ => Err(format!("File not supported {file_extension}")),
    }
}
