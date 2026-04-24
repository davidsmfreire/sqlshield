//! Errors surfaced by the public API.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SqlShieldError {
    #[error("failed to read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("file {} has no extension", .0.display())]
    MissingExtension(PathBuf),

    #[error("unsupported source file extension `{0}` (expected one of: py, rs)")]
    UnsupportedFileExtension(String),

    #[error("unsupported schema type `{0}` (expected one of: sql)")]
    UnsupportedSchemaType(String),

    #[error("failed to parse SQL: {0}")]
    SqlParse(#[from] sqlparser::parser::ParserError),

    #[error("failed to parse source code")]
    CodeParse,
}

pub type Result<T> = std::result::Result<T, SqlShieldError>;
