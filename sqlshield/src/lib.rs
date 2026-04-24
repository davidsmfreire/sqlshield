//! SQL schema-aware linter. Extracts raw SQL strings from source files and validates
//! them against a schema, reporting missing tables, missing columns, and join errors.
//!
//! The public entry points are [`validate_query`] for a single SQL string and
//! [`validate_files`] for walking a directory tree of supported source files.
//! Both have `_with_dialect` variants if you need to target a specific SQL
//! flavor; the defaults use [`Dialect::Generic`].

pub mod dialect;
pub mod error;
pub mod finder;
pub mod schema;
pub mod validation;

use std::path::Path;
use std::sync::LazyLock;

use finder::QueryInCode;
use regex::Regex;
use validation::{validate_queries_in_code, validate_statements_with_schema, SqlValidationError};
use walkdir::WalkDir;

pub use dialect::Dialect;
pub use error::{Result, SqlShieldError};

static CODE_FILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    let extensions = finder::SUPPORTED_CODE_FILE_EXTENSIONS.join("|");
    Regex::new(&format!(r"\.({extensions})$")).expect("static regex built from known extensions")
});

/// Validate a single SQL query against a schema using the [`Dialect::Generic`]
/// parser. For dialect-specific parsing, see [`validate_query_with_dialect`].
pub fn validate_query(query: &str, schema: &str) -> Result<Vec<String>> {
    validate_query_with_dialect(query, schema, Dialect::default())
}

pub fn validate_query_with_dialect(
    query: &str,
    schema: &str,
    dialect: Dialect,
) -> Result<Vec<String>> {
    let parser_dialect = dialect.as_sqlparser();
    let statements = sqlparser::parser::Parser::parse_sql(parser_dialect.as_ref(), query)?;
    let loaded_schema = schema::load_schema(schema.as_bytes(), "sql")?;
    Ok(validate_statements_with_schema(&statements, &loaded_schema))
}

/// Walk `dir`, extract SQL from each supported source file, and validate
/// it against the schema declared in `schema_file_path`. Uses
/// [`Dialect::Generic`]; see [`validate_files_with_dialect`] for a specific
/// dialect.
pub fn validate_files(dir: &Path, schema_file_path: &Path) -> Result<Vec<SqlValidationError>> {
    validate_files_with_dialect(dir, schema_file_path, Dialect::default())
}

pub fn validate_files_with_dialect(
    dir: &Path,
    schema_file_path: &Path,
    dialect: Dialect,
) -> Result<Vec<SqlValidationError>> {
    let tables_and_columns: schema::TablesAndColumns =
        schema::load_schema_from_file(schema_file_path)?;
    let parser_dialect = dialect.as_sqlparser();

    let mut validation_errors: Vec<SqlValidationError> = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let file_path = entry.path();

        let Some(path_str) = file_path.to_str() else {
            continue;
        };

        if !(file_path.is_file() && CODE_FILE_RE.is_match(path_str)) {
            continue;
        }

        let queries: Result<Vec<QueryInCode>> =
            finder::find_queries_in_file_with_dialect(file_path, parser_dialect.as_ref());

        if let Ok(queries) = queries {
            let query_errors = validate_queries_in_code(&queries, &tables_and_columns);

            for query_error in query_errors {
                let validation_error =
                    SqlValidationError::new(file_path, query_error.line, query_error.description);

                validation_errors.push(validation_error);
            }
        }
    }

    Ok(validation_errors)
}
