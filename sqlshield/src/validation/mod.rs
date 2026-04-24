//! Validates parsed SQL statements against a schema: flags missing tables,
//! missing columns, and projection mismatches across JOINs and CTEs.

pub mod asserts;
pub mod clauses;

use sqlparser::ast::{Expr, Query, SelectItem, SetExpr, Statement, TableFactor};
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
        match statement {
            Statement::Query(query_box) => {
                errors.extend(validate_query_with_schema(query_box.as_ref(), schema));
            }
            Statement::Insert {
                table_name,
                columns,
                source,
                ..
            } => {
                errors.extend(clauses::insert::validate_insert(
                    table_name, columns, schema,
                ));
                if let Some(source_query) = source {
                    errors.extend(validate_query_with_schema(source_query.as_ref(), schema));
                }
            }
            Statement::Update {
                table,
                assignments,
                from,
                selection,
                ..
            } => {
                errors.extend(clauses::update::validate_update(
                    table,
                    assignments,
                    from.as_ref(),
                    selection.as_ref(),
                    schema,
                ));
            }
            Statement::Delete {
                from,
                using,
                selection,
                ..
            } => {
                errors.extend(clauses::delete::validate_delete(
                    from,
                    using.as_deref(),
                    selection.as_ref(),
                    schema,
                ));
            }
            _ => {}
        }
    }
    errors
}

pub fn validate_query_with_schema<'a>(
    query: &'a Query,
    schema: &schema::TablesAndColumns,
) -> Vec<String> {
    let empty: HashMap<&'a str, HashSet<&'a str>> = HashMap::new();
    validate_query_with_scope(query, schema, &empty)
}

/// Like [`validate_query_with_schema`] but threads a parent `extras` map
/// through so subqueries can see CTEs defined in enclosing scopes. Used for
/// nested subqueries (IN / EXISTS / scalar) and for CTE-to-CTE references.
pub(crate) fn validate_query_with_scope<'a>(
    query: &'a Query,
    schema: &schema::TablesAndColumns,
    parent_extras: &HashMap<&'a str, HashSet<&'a str>>,
) -> Vec<String> {
    let mut extras: HashMap<&'a str, HashSet<&'a str>> = parent_extras.clone();
    let mut errors: Vec<String> = vec![];

    validate_and_extract_subqueries(query, schema, &mut extras, &mut errors);

    validate_set_expr(query.body.as_ref(), schema, &extras, &mut errors);

    // ORDER BY resolves against the outermost Select's scope. For set
    // operations we can't pick one side — skip.
    if let SetExpr::Select(boxed) = query.body.as_ref() {
        let select = boxed.as_ref();
        if !query.order_by.is_empty() {
            let order_exprs: Vec<&Expr> = query.order_by.iter().map(|ob| &ob.expr).collect();
            errors.extend(clauses::select::validate_exprs_in_select_scope(
                &order_exprs,
                select,
                schema,
                &extras,
            ));
        }
    }

    errors
}

/// Walk a `SetExpr` (Select / SetOperation / Query / Values) recursively,
/// validating each inner Select against `schema` plus any CTE-derived
/// `extras` visible at this level. Derived-table scopes are cloned per
/// branch so they don't leak between UNION arms.
fn validate_set_expr<'a>(
    body: &'a SetExpr,
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&'a str, HashSet<&'a str>>,
    errors: &mut Vec<String>,
) {
    match body {
        SetExpr::Select(boxed) => {
            let select = boxed.as_ref();
            let mut local_extras = extras.clone();
            extract_derived_from_factors(select, schema, &mut local_extras, errors);
            errors.extend(select.validate(schema, &local_extras));
        }
        SetExpr::SetOperation { left, right, .. } => {
            validate_set_expr(left.as_ref(), schema, extras, errors);
            validate_set_expr(right.as_ref(), schema, extras, errors);
        }
        SetExpr::Query(inner) => {
            errors.extend(validate_query_with_schema(inner.as_ref(), schema));
        }
        _ => {}
    }
}

/// Extract the set of column names that a `Query` projects (for use as the
/// visible columns of a CTE or derived table).
fn project_columns(query: &Query) -> HashSet<&str> {
    project_columns_of_body(query.body.as_ref())
}

fn project_columns_of_body(body: &SetExpr) -> HashSet<&str> {
    let mut cols = HashSet::new();
    match body {
        SetExpr::Select(select_box) => {
            for item in &select_box.projection {
                match item {
                    SelectItem::UnnamedExpr(expr) => match expr {
                        Expr::Identifier(ident) => {
                            cols.insert(ident.value.as_str());
                        }
                        Expr::CompoundIdentifier(idents) => {
                            if let Some(last) = idents.last() {
                                cols.insert(last.value.as_str());
                            }
                        }
                        _ => {}
                    },
                    SelectItem::ExprWithAlias { alias, .. } => {
                        cols.insert(alias.value.as_str());
                    }
                    _ => {}
                }
            }
        }
        // Per SQL spec, the output column names of `A UNION B` are the left
        // branch's names.
        SetExpr::SetOperation { left, .. } => {
            cols.extend(project_columns_of_body(left.as_ref()));
        }
        SetExpr::Query(inner) => {
            cols.extend(project_columns(inner.as_ref()));
        }
        _ => {}
    }
    cols
}

fn extract_derived_from_factors<'a>(
    select: &'a sqlparser::ast::Select,
    schema: &schema::TablesAndColumns,
    extras: &mut HashMap<&'a str, HashSet<&'a str>>,
    errors: &mut Vec<String>,
) {
    for table in &select.from {
        walk_factor_for_derived(&table.relation, schema, extras, errors);
        for join in &table.joins {
            walk_factor_for_derived(&join.relation, schema, extras, errors);
        }
    }
}

fn walk_factor_for_derived<'a>(
    factor: &'a TableFactor,
    schema: &schema::TablesAndColumns,
    extras: &mut HashMap<&'a str, HashSet<&'a str>>,
    errors: &mut Vec<String>,
) {
    if let TableFactor::Derived {
        subquery, alias, ..
    } = factor
    {
        let Some(alias) = alias.as_ref() else {
            return;
        };

        // Recursive validation: uses its own fresh extras, so inner CTEs don't
        // leak out. Inner errors bubble up to the outer error list.
        errors.extend(validate_query_with_schema(subquery.as_ref(), schema));

        let cols = project_columns(subquery.as_ref());
        extras.insert(alias.name.value.as_str(), cols);
    }
}

fn validate_and_extract_subqueries<'a>(
    query: &'a Query,
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

        errors.extend(validate_query_with_schema(derived.query.as_ref(), schema));

        let derived_name = derived.alias.name.value.as_str();
        let derived_columns = project_columns(derived.query.as_ref());
        extras.insert(derived_name, derived_columns);
    }
}
