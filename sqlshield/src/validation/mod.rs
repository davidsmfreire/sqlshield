pub mod asserts;
pub mod clauses;

use sqlparser::ast::{SetExpr, Statement};
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::{finder, schema};
use colored::Colorize;

use self::clauses::ClauseValidation;

#[derive(Debug, PartialEq)]
pub struct SqlValidationError {
    pub location: String,
    pub description: String,
}

impl SqlValidationError {
    pub fn new(file_path: &std::path::Path, line_number: usize, description: String) -> Self {
        let location = [
            file_path.to_string_lossy().to_string(),
            line_number.to_string(),
        ]
        .join(":");

        SqlValidationError {
            location,
            description,
        }
    }
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

pub struct SqlQueryError {
    pub line: usize,
    pub description: String,
}

pub fn validate_queries_in_code(
    queries: &[finder::QueryInCode],
    schema: &schema::TablesAndColumns,
) -> Vec<SqlQueryError> {
    let mut errors: Vec<SqlQueryError> = Vec::new();
    for query in queries {
        let query_errors = validate_statements_with_schema(&query.statements, schema);
        for query_error in query_errors {
            errors.push(SqlQueryError {
                line: query.line,
                description: query_error,
            });
        }
    }
    errors
}

pub fn validate_statements_with_schema(
    query: &[Statement],
    schema: &schema::TablesAndColumns,
) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    for statement in query {
        if let Statement::Query(query_box) = statement {
            errors.append(&mut validate_query_with_schema(query_box.as_ref(), schema));
        }
    }
    errors
}

pub fn validate_query_with_schema<'a>(
    query: &'a sqlparser::ast::Query,
    schema: &schema::TablesAndColumns,
) -> Vec<String> {
    let mut extras: HashMap<&'a str, HashSet<&'a str>> = HashMap::new();
    let mut errors: Vec<String> = vec![];

    validate_and_extract_subqueries(query, schema, &mut extras, &mut errors);

    if let SetExpr::Select(boxed) = query.body.as_ref() {
        errors.append(&mut boxed.as_ref().validate(schema, &extras));
    }
    // TODO: inserts, updates, ...

    errors
}

fn validate_and_extract_subqueries<'a>(
    query: &'a sqlparser::ast::Query,
    schema: &schema::TablesAndColumns,
    extras: &mut HashMap<&'a str, HashSet<&'a str>>,
    errors: &mut Vec<String>,
) {
    let Some(with) = &query.with else {
        return;
    };

    for derived in &with.cte_tables {
        if let Some(derived_from) = &derived.from {
            if !schema.contains_key(derived_from.value.as_str()) {
                errors.push(format!(
                    "Table `{}` not found in schema nor subqueries",
                    derived_from.value
                ));
                continue;
            }
        }

        let mut new_errors = validate_query_with_schema(derived.query.as_ref(), schema);
        errors.append(&mut new_errors);

        let derived_name = derived.alias.name.value.as_str();
        let mut derived_columns: HashSet<&str> = HashSet::new();

        if let SetExpr::Select(select_box) = derived.query.as_ref().body.as_ref() {
            let select = select_box.as_ref();
            for item in &select.projection {
                match item {
                    sqlparser::ast::SelectItem::UnnamedExpr(expr) => match expr {
                        sqlparser::ast::Expr::Identifier(ident) => {
                            derived_columns.insert(ident.value.as_str());
                        }
                        sqlparser::ast::Expr::CompoundIdentifier(idents) => {
                            // ! is this correct?
                            derived_columns.insert(idents.last().unwrap().value.as_str());
                        }
                        _ => {}
                    },
                    sqlparser::ast::SelectItem::ExprWithAlias { alias, .. } => {
                        derived_columns.insert(alias.value.as_str());
                    }
                    _ => {}
                }
            }
        }

        extras.insert(derived_name, derived_columns);
    }
}
