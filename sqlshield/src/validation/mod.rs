pub mod asserts;
pub mod clauses;

use sqlparser::ast::{SetExpr, Statement};
use std::collections::HashSet;
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
            location: location,
            description: description,
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
    queries: &Vec<finder::QueryInCode>,
    schema: &schema::TablesAndColumns,
) -> Vec<SqlQueryError> {
    let mut errors: Vec<SqlQueryError> = Vec::new();
    for query in queries {
        let query_errors: Vec<String> = validate_statements_with_schema(&query.statements, &schema);
        for query_error in query_errors {
            errors.push(SqlQueryError {
                line: query.line,
                description: query_error,
            });
        }
    }
    return errors;
}

pub fn validate_statements_with_schema(
    query: &Vec<Statement>,
    schema: &schema::TablesAndColumns,
) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    for statement in query {
        match statement {
            Statement::Query(query_box) => {
                errors.append(&mut validate_query_with_schema(
                    &query_box.as_ref(),
                    &schema,
                ));
            }
            _ => {}
        }
    }
    return errors;
}

pub fn validate_query_with_schema(
    query: &sqlparser::ast::Query,
    schema: &schema::TablesAndColumns,
) -> Vec<String> {
    let mut schema_with_derived: schema::TablesAndColumns = schema.clone();

    let mut errors: Vec<String> = vec![];

    validate_and_extract_subqueries(&query, &schema, &mut schema_with_derived, &mut errors);

    match query.body.as_ref() {
        SetExpr::Select(boxed) => errors.append(&mut boxed.as_ref().validate(&schema_with_derived)),
        // TODO: inserts, updated, ...
        // SetExpr::Insert(boxed) => {}
        _ => {}
    }

    return errors;
}

fn validate_and_extract_subqueries(
    query: &sqlparser::ast::Query,
    schema: &schema::TablesAndColumns,
    schema_with_derived: &mut schema::TablesAndColumns,
    errors: &mut Vec<String>,
) {
    if let Some(with) = &query.with {
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

            let derived_name = &derived.alias.name.value;
            let mut derived_columns: Vec<String> = vec![];

            match derived.query.as_ref().body.as_ref() {
                SetExpr::Select(select_box) => {
                    let select = select_box.as_ref();
                    for item in &select.projection {
                        match &item {
                            sqlparser::ast::SelectItem::UnnamedExpr(expr) => {
                                match &expr {
                                    sqlparser::ast::Expr::Identifier(ident) => {
                                        derived_columns.push(ident.value.clone());
                                    }
                                    sqlparser::ast::Expr::CompoundIdentifier(idents) => {
                                        // ! is this correct?
                                        derived_columns.push(idents.last().unwrap().value.clone());
                                    }
                                    _ => {}
                                }
                            }
                            sqlparser::ast::SelectItem::ExprWithAlias { alias, .. } => {
                                derived_columns.push(alias.value.clone());
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            schema_with_derived.insert(derived_name.clone(), HashSet::from_iter(derived_columns));
        }
    }
}
