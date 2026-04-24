pub mod finder;
pub mod schema;
pub mod validation;

use std::path::Path;

use finder::QueryInCode;
use regex::Regex;
use validation::{validate_queries_in_code, validate_statements_with_schema, SqlValidationError};
use walkdir::WalkDir;

pub fn validate_query(query: &str, schema: &str) -> Result<Vec<String>, String> {
    let dialect = sqlparser::dialect::GenericDialect {};

    let statements = match sqlparser::parser::Parser::parse_sql(&dialect, query) {
        Ok(statements) => statements,
        Err(err) => return Err(err.to_string()),
    };

    let loaded_schema = schema::load_schema(schema.as_bytes(), "sql")?;

    Ok(validate_statements_with_schema(&statements, &loaded_schema))
}

pub fn validate_files(dir: &Path, schema_file_path: &Path) -> Vec<SqlValidationError> {
    let supported_code_file_extensions: String = finder::SUPPORTED_CODE_FILE_EXTENSIONS.join("|");

    let code_file_regex = Regex::new(&format!(r"\.({supported_code_file_extensions})$")).unwrap();

    let tables_and_columns: schema::TablesAndColumns =
        schema::load_schema_from_file(schema_file_path).unwrap();

    let mut validation_errors: Vec<SqlValidationError> = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let file_path = entry.path();

        if !(file_path.is_file()
            && code_file_regex.is_match(file_path.to_str().expect("Couldn't convert path to str")))
        {
            continue;
        }

        let queries: Result<Vec<QueryInCode>, String> = finder::find_queries_in_file(file_path);

        if let Ok(queries) = queries {
            let query_errors = validate_queries_in_code(&queries, &tables_and_columns);

            for query_error in query_errors {
                let validation_error =
                    SqlValidationError::new(file_path, query_error.line, query_error.description);

                validation_errors.push(validation_error);
            }
        }
    }

    validation_errors
}
