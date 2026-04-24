//! Locates SQL strings inside source files by walking a tree-sitter AST.

mod python;
mod rust;

use std::{fs, path::Path};

use crate::error::{Result, SqlShieldError};

#[derive(Debug)]
pub struct QueryInCode {
    pub line: usize,
    pub statements: Vec<sqlparser::ast::Statement>,
}

pub const SUPPORTED_CODE_FILE_EXTENSIONS: [&str; 2] = ["py", "rs"];

pub fn find_queries_in_file(file_path: &Path) -> Result<Vec<QueryInCode>> {
    let dialect = sqlparser::dialect::GenericDialect {};
    find_queries_in_file_with_dialect(file_path, &dialect)
}

pub fn find_queries_in_file_with_dialect(
    file_path: &Path,
    dialect: &dyn sqlparser::dialect::Dialect,
) -> Result<Vec<QueryInCode>> {
    let file_extension = file_path
        .extension()
        .ok_or_else(|| SqlShieldError::MissingExtension(file_path.to_path_buf()))?
        .to_string_lossy();

    let code = fs::read(file_path).map_err(|source| SqlShieldError::Io {
        path: file_path.to_path_buf(),
        source,
    })?;

    find_queries_in_code_with_dialect(&code, &file_extension, dialect)
}

type NodeQueryExtractor = fn(&tree_sitter::Node, &[u8]) -> Option<String>;

pub fn find_queries_in_code(code: &[u8], file_extension: &str) -> Result<Vec<QueryInCode>> {
    let dialect = sqlparser::dialect::GenericDialect {};
    find_queries_in_code_with_dialect(code, file_extension, &dialect)
}

pub fn find_queries_in_code_with_dialect(
    code: &[u8],
    file_extension: &str,
    dialect: &dyn sqlparser::dialect::Dialect,
) -> Result<Vec<QueryInCode>> {
    let (language, query_extractor): (tree_sitter::Language, NodeQueryExtractor) =
        match file_extension {
            "py" => (
                tree_sitter_python::language(),
                python::extract_query_string_from_node,
            ),
            "rs" => (
                tree_sitter_rust::language(),
                rust::extract_query_string_from_node,
            ),
            other => return Err(SqlShieldError::UnsupportedFileExtension(other.to_string())),
        };

    let mut parser = tree_sitter::Parser::new();

    parser
        .set_language(language)
        .expect("tree-sitter grammar incompatible with tree-sitter runtime");

    let parsed: Option<tree_sitter::Tree> = parser.parse(code, None);
    let mut queries: Vec<QueryInCode> = Vec::new();

    let tree = parsed.ok_or(SqlShieldError::CodeParse)?;
    find_queries_in_ast(
        &tree.root_node(),
        code,
        &query_extractor,
        dialect,
        &mut queries,
        None,
    );
    Ok(queries)
}

fn find_queries_in_ast(
    node: &tree_sitter::Node,
    code: &[u8],
    query_extractor: &NodeQueryExtractor,
    dialect: &dyn sqlparser::dialect::Dialect,
    queries: &mut Vec<QueryInCode>,
    verbose: Option<u8>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match query_extractor(&child, code) {
            Some(string_content) => {
                let query_at = child.start_position();

                let statements = sqlparser::parser::Parser::parse_sql(dialect, &string_content);

                match statements {
                    Ok(statements) => {
                        queries.push(QueryInCode {
                            line: query_at.row + 1,
                            statements,
                        });
                    }
                    Err(err) => {
                        if verbose.unwrap_or(0) > 0 {
                            eprintln!("{err} {string_content}");
                        }
                    }
                }
            }
            None => find_queries_in_ast(&child, code, query_extractor, dialect, queries, None),
        }
    }
}
