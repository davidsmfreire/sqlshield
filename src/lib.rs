use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::{fmt, fs};

use colored::Colorize;
use regex::Regex;
use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser as SqlParser;
use tree_sitter::{Node, Parser as CodeParser};
use walkdir::WalkDir;

struct SqlQueryError {
    line: usize,
    description: String,
}

struct QueryInCode {
    line: usize,
    statements: Vec<Statement>,
}

type TablesAndColumns<'a> = HashMap<&'a str, HashSet<&'a str>>;

#[derive(Debug, PartialEq)]
pub struct SqlValidationError {
    pub location: String,
    pub description: String,
}

impl fmt::Display for SqlValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}: {} {}",
            self.location,
            "error:".red(),
            self.description
        )
    }
}

fn schema_to_tables_and_columns(schema: &Vec<Statement>) -> TablesAndColumns {
    let mut tables: HashMap<&str, HashSet<&str>> = HashMap::new();
    for statement in schema {
        match statement {
            Statement::CreateTable { columns, name, .. } => {
                let ident = &name.0[0];
                let name = &ident.value;
                let columns_set: HashSet<&str> =
                    HashSet::from_iter(columns.iter().map(|e| e.name.value.as_str()));
                tables.insert(&name, columns_set);
            }
            _ => {}
        }
    }
    return tables;
}

fn validate_query_with_schema(query: &Vec<Statement>, schema: &TablesAndColumns) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    for statement in query {
        match statement {
            Statement::Query(query_box) => {
                let query_ref = query_box.as_ref();
                let mut columns: Option<&HashSet<&str>> = None;

                match query_ref.body.as_ref() {
                    SetExpr::Select(select_box) => {
                        let select = select_box.as_ref();
                        let mut current_table: Option<&str> = None;

                        for item in &select.from {
                            match &item.relation {
                                sqlparser::ast::TableFactor::Table { name, .. } => {
                                    let table_name = name.0.last().unwrap();
                                    let table_name_str = table_name.value.as_str();

                                    current_table = Some(table_name_str);

                                    columns = schema.get(table_name_str);

                                    if columns.is_none() {
                                        errors.push(format!(
                                            "Table `{table_name}` not found in schema"
                                        ))
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Some(cols) = columns {
                            for item in &select.projection {
                                match item {
                                    sqlparser::ast::SelectItem::UnnamedExpr(expression) => {
                                        match expression {
                                            sqlparser::ast::Expr::Identifier(identifier) => {
                                                let col = identifier.value.as_str();
                                                let current_table = current_table.unwrap();
                                                if !cols.contains(col) {
                                                    errors.push(format!("Column `{col}` not found in table `{current_table}`"))
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    // sqlparser::ast::SelectItem::ExprWithAlias { expr, alias } => todo!(),
                                    // sqlparser::ast::SelectItem::QualifiedWildcard(_, _) => todo!(),
                                    // sqlparser::ast::SelectItem::Wildcard(_) => todo!(),
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    return errors;
}

fn find_queries_in_tree(node: &Node, code: &[u8], queries: &mut Vec<QueryInCode>) {
    let mut cursor = node.walk();
    let dialect = GenericDialect {};

    for child in node.children(&mut cursor) {
        let mut child_cursor = child.walk();
        for component in child.children(&mut child_cursor) {
            if component.kind() != "assignment" {
                continue;
            }

            if component.child_count() > 3 {
                continue;
            }

            let identifier = component.child(0).unwrap();
            let equal = component.child(1).unwrap();
            let var = component.child(2).unwrap();

            let is_string_assignment =
                identifier.kind() == "identifier" && equal.kind() == "=" && var.kind() == "string";

            if !is_string_assignment {
                continue;
            }

            let content = var.child(1).unwrap();
            let content_as_string =
                String::from_utf8_lossy(&code[content.start_byte()..content.end_byte()]);

            let point = component.start_position();

            let statements = SqlParser::parse_sql(&dialect, &content_as_string);

            if let Ok(statements) = statements {
                queries.push(QueryInCode {
                    line: point.row + 1,
                    statements,
                })
            }
        }
        find_queries_in_tree(&child, code, queries);
    }
}

fn find_queries(code: &[u8]) -> Vec<QueryInCode> {
    let mut parser = CodeParser::new();
    parser
        .set_language(tree_sitter_python::language())
        .expect("Error loading Python grammar");
    let parsed = parser.parse(code, None);
    let mut queries: Vec<QueryInCode> = Vec::new();

    if let Some(tree) = parsed {
        find_queries_in_tree(&tree.root_node(), &code, &mut queries);
    }
    return queries;
}

fn validate_queries_in_code(code: &[u8], schema: &TablesAndColumns) -> Vec<SqlQueryError> {
    let queries = find_queries(&code);
    let mut errors: Vec<SqlQueryError> = Vec::new();
    for query in queries {
        let query_errors = validate_query_with_schema(&query.statements, &schema);
        for query_error in query_errors {
            errors.push(SqlQueryError {
                line: query.line,
                description: query_error,
            });
        }
    }
    return errors;
}

pub fn validate(dir: &PathBuf, schema_file_path: &PathBuf) -> Vec<SqlValidationError> {
    let python_file_pattern = Regex::new(r"\.py$").unwrap();
    let schema: String = fs::read_to_string(schema_file_path).unwrap();

    let dialect = GenericDialect {};
    let schema_ast = SqlParser::parse_sql(&dialect, &schema).unwrap();

    let tables_and_columns: HashMap<&str, HashSet<&str>> =
        schema_to_tables_and_columns(&schema_ast);

    let mut validation_errors: Vec<SqlValidationError> = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let file_path = entry.path();

        if file_path.is_file() && python_file_pattern.is_match(file_path.to_str().unwrap()) {
            // println!("{:?}", file_path);
            let code = fs::read(file_path)
                .map_err(|err| eprintln!("Could not open {:?} due to error: {err}", file_path));
            if let Ok(code) = code {
                let query_errors = validate_queries_in_code(&code, &tables_and_columns);
                for query_error in query_errors {
                    let location = [
                        file_path.to_string_lossy().to_string(),
                        query_error.line.to_string(),
                    ]
                    .join(":");

                    validation_errors.push(SqlValidationError {
                        location: location,
                        description: query_error.description,
                    });
                }
            }
        }
    }

    return validation_errors;
}
