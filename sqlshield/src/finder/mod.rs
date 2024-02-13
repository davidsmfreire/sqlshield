mod python;
mod rust;
mod javascript;

use std::{fs, path::Path};

use sqlparser;

pub struct QueryInCode {
    pub line: usize,
    pub statements: Vec<sqlparser::ast::Statement>,
}

pub const SUPPORTED_CODE_FILE_EXTENSIONS: [&str; 3] = ["py", "rs", "js"];

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

type NodeQueryExtractor = fn(&tree_sitter::Node, &[u8]) -> Option<String>;

pub fn find_queries_in_code(
    code: &[u8],
    file_extension: &str,
) -> Result<Vec<super::QueryInCode>, String> {
    let (language, query_extractor): (tree_sitter::Language, NodeQueryExtractor) =
        match file_extension.as_ref() {
            "py" => (
                tree_sitter_python::language(),
                python::extract_query_from_node,
            ),
            "rs" => (tree_sitter_rust::language(), rust::extract_query_from_node),
            "js" => (tree_sitter_javascript::language(), javascript::extract_query_from_node),
            _ => panic!("{}", format!("File not supported {file_extension}")),
        };

    let mut parser = tree_sitter::Parser::new();

    parser
        .set_language(language)
        .expect("Error loading grammar");

    let parsed: Option<tree_sitter::Tree> = parser.parse(code, None);

    let mut queries: Vec<super::QueryInCode> = Vec::new();

    let dialect = sqlparser::dialect::GenericDialect {};

    if let Some(tree) = parsed {
        find_queries_in_ast(
            &tree.root_node(),
            &code,
            &query_extractor,
            &dialect,
            &mut queries,
            None,
        );
        return Ok(queries);
    } else {
        return Err("Could not parse code".to_string());
    }
}

fn find_queries_in_ast(
    node: &tree_sitter::Node,
    code: &[u8],
    query_extractor: &NodeQueryExtractor,
    dialect: &impl sqlparser::dialect::Dialect,
    queries: &mut Vec<super::QueryInCode>,
    verbose: Option<u8>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let mut child_cursor = child.walk();

        for component in child.children(&mut child_cursor) {
            match query_extractor(&component, code) {
                Some(string_content) => {
                    // ! Duct tape
                    if string_content.find("REPLACE").is_some() {
                        continue;
                    }
                    // !

                    let query_at = component.start_position();

                    let statements = sqlparser::parser::Parser::parse_sql(dialect, &string_content);

                    match statements {
                        Ok(statements) => {
                            queries.push(super::QueryInCode {
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
                None => continue,
            }
        }

        find_queries_in_ast(&child, code, query_extractor, dialect, queries, None);
    }
}
