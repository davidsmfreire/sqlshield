//! Validates parsed SQL statements against a schema: flags missing tables,
//! missing columns, and projection mismatches across JOINs and CTEs.

pub mod asserts;
pub mod clauses;

use sqlparser::ast::{Expr, Ident, Query, Select, SelectItem, SetExpr, Statement, TableFactor};
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::dialect::Dialect;
use crate::schema::sql::{fold_ident, qualified_key};
use crate::{finder, schema};
use colored::Colorize;

use self::clauses::ClauseValidation;

/// Per-relation extras: keys (CTE / derived-table names) and column names
/// stored in dialect-folded form so lookups are direct equality rather than
/// case-insensitive scans.
pub(crate) type Extras = HashMap<String, HashSet<String>>;

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
    dialect: Dialect,
) -> Vec<SqlQueryError> {
    let mut errors: Vec<SqlQueryError> = Vec::new();
    for query in queries {
        let query_errors = validate_statements_with_schema(&query.statements, schema, dialect);
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
    dialect: Dialect,
) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    for statement in query {
        match statement {
            Statement::Query(query_box) => {
                errors.extend(validate_query_with_schema(
                    query_box.as_ref(),
                    schema,
                    dialect,
                ));
            }
            Statement::Insert {
                table_name,
                columns,
                source,
                ..
            } => {
                errors.extend(clauses::insert::validate_insert(
                    table_name, columns, schema, dialect,
                ));
                if let Some(source_query) = source {
                    errors.extend(validate_query_with_schema(
                        source_query.as_ref(),
                        schema,
                        dialect,
                    ));
                }
            }
            Statement::Update {
                table,
                assignments,
                from,
                selection,
                ..
            } => {
                let empty: Extras = HashMap::new();
                errors.extend(clauses::update::validate_update(
                    table,
                    assignments,
                    from.as_ref(),
                    selection.as_ref(),
                    schema,
                    dialect,
                    &empty,
                ));
            }
            Statement::Delete {
                from,
                using,
                selection,
                ..
            } => {
                let empty: Extras = HashMap::new();
                errors.extend(clauses::delete::validate_delete(
                    from,
                    using.as_deref(),
                    selection.as_ref(),
                    schema,
                    dialect,
                    &empty,
                ));
            }
            Statement::Merge {
                table,
                source,
                on,
                clauses: merge_clauses,
                ..
            } => {
                let empty: Extras = HashMap::new();
                errors.extend(clauses::merge::validate_merge(
                    table,
                    source,
                    on,
                    merge_clauses,
                    schema,
                    dialect,
                    &empty,
                ));
            }
            _ => {}
        }
    }
    errors
}

pub fn validate_query_with_schema(
    query: &Query,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
) -> Vec<String> {
    let empty: Extras = HashMap::new();
    validate_query_with_scope(query, schema, dialect, &empty)
}

/// Like [`validate_query_with_schema`] but threads a parent `extras` map
/// through so subqueries can see CTEs defined in enclosing scopes. Used for
/// nested subqueries (IN / EXISTS / scalar) and for CTE-to-CTE references.
pub(crate) fn validate_query_with_scope(
    query: &Query,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    parent_extras: &Extras,
) -> Vec<String> {
    let mut extras: Extras = parent_extras.clone();
    let mut errors: Vec<String> = vec![];

    validate_and_extract_subqueries(query, schema, dialect, &mut extras, &mut errors);

    validate_set_expr(query.body.as_ref(), schema, dialect, &extras, &mut errors);

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
                dialect,
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
fn validate_set_expr(
    body: &SetExpr,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
    errors: &mut Vec<String>,
) {
    match body {
        SetExpr::Select(boxed) => {
            let select = boxed.as_ref();
            let mut local_extras = extras.clone();
            extract_derived_from_factors(select, schema, dialect, &mut local_extras, errors);
            errors.extend(select.validate(schema, dialect, &local_extras));
        }
        SetExpr::SetOperation {
            op, left, right, ..
        } => {
            if let (Some(l), Some(r)) = (
                count_projection(left.as_ref()),
                count_projection(right.as_ref()),
            ) {
                if l != r {
                    errors.push(format!(
                        "{op}: column count mismatch (left has {l}, right has {r})"
                    ));
                }
            }
            validate_set_expr(left.as_ref(), schema, dialect, extras, errors);
            validate_set_expr(right.as_ref(), schema, dialect, extras, errors);
        }
        SetExpr::Query(inner) => {
            errors.extend(validate_query_with_scope(
                inner.as_ref(),
                schema,
                dialect,
                extras,
            ));
        }
        // `WITH … INSERT/UPDATE …`: sqlparser wraps the DML inside a Query
        // body. Dispatch back to the DML validators so the inner statement
        // is checked and the surrounding CTEs (carried in `extras`) stay in
        // scope.
        SetExpr::Insert(Statement::Insert {
            table_name,
            columns,
            source,
            ..
        }) => {
            errors.extend(clauses::insert::validate_insert(
                table_name, columns, schema, dialect,
            ));
            if let Some(source_query) = source {
                errors.extend(validate_query_with_scope(
                    source_query.as_ref(),
                    schema,
                    dialect,
                    extras,
                ));
            }
        }
        SetExpr::Update(Statement::Update {
            table,
            assignments,
            from,
            selection,
            ..
        }) => {
            errors.extend(clauses::update::validate_update(
                table,
                assignments,
                from.as_ref(),
                selection.as_ref(),
                schema,
                dialect,
                extras,
            ));
        }
        _ => {}
    }
}

/// Best-effort projection arity for a SetExpr. Returns `None` when the
/// branch projects through a wildcard (`SELECT *`) or another shape we
/// can't measure statically — caller should skip arity checks in that case.
fn count_projection(body: &SetExpr) -> Option<usize> {
    match body {
        SetExpr::Select(select_box) => {
            // `SELECT *` / `SELECT t.*` carry no static arity; bail out.
            if select_box.projection.iter().any(|item| {
                matches!(
                    item,
                    SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _)
                )
            }) {
                return None;
            }
            Some(select_box.projection.len())
        }
        SetExpr::Query(inner) => count_projection(inner.body.as_ref()),
        SetExpr::SetOperation { left, .. } => count_projection(left.as_ref()),
        SetExpr::Values(values) => values.rows.first().map(|row| row.len()),
        _ => None,
    }
}

/// Extract the set of column names that a `Query` projects (for use as the
/// visible columns of a CTE or derived table). Returns Idents so callers can
/// fold them dialect-aware.
///
/// Wildcards (`*`, `t.*`) are expanded by walking the inner FROM clause and
/// looking each relation up in `schema` or `parent_extras`. If a wildcard
/// can't be resolved (unknown table, derived table whose own projection we
/// can't determine), it contributes no columns — better a quiet false
/// negative than a noisy false positive.
fn project_columns(
    query: &Query,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    parent_extras: &Extras,
) -> Vec<Ident> {
    project_columns_of_body(query.body.as_ref(), schema, dialect, parent_extras)
}

fn project_columns_of_body(
    body: &SetExpr,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Vec<Ident> {
    let mut cols = Vec::new();
    match body {
        SetExpr::Select(select_box) => {
            cols.extend(project_select_columns(
                select_box.as_ref(),
                schema,
                dialect,
                extras,
            ));
        }
        // Per SQL spec, the output column names of `A UNION B` are the left
        // branch's names.
        SetExpr::SetOperation { left, .. } => {
            cols.extend(project_columns_of_body(
                left.as_ref(),
                schema,
                dialect,
                extras,
            ));
        }
        SetExpr::Query(inner) => {
            cols.extend(project_columns(inner.as_ref(), schema, dialect, extras));
        }
        _ => {}
    }
    cols
}

fn project_select_columns(
    select: &Select,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    parent_extras: &Extras,
) -> Vec<Ident> {
    // Pre-publish derived tables in the FROM clause into a local extras so
    // that wildcards over them (`SELECT d.* FROM (SELECT ...) d`) and bare
    // `SELECT *` cross-joining a derived table both resolve.
    let mut local = parent_extras.clone();
    for tw in &select.from {
        publish_derived(&tw.relation, schema, dialect, &mut local);
        for j in &tw.joins {
            publish_derived(&j.relation, schema, dialect, &mut local);
        }
    }

    let mut out = Vec::new();
    for item in &select.projection {
        match item {
            SelectItem::UnnamedExpr(Expr::Identifier(ident)) => out.push(ident.clone()),
            SelectItem::UnnamedExpr(Expr::CompoundIdentifier(ids)) => {
                if let Some(last) = ids.last() {
                    out.push(last.clone());
                }
            }
            SelectItem::ExprWithAlias { alias, .. } => out.push(alias.clone()),
            SelectItem::Wildcard(_) => {
                for tw in &select.from {
                    out.extend(relation_columns(&tw.relation, schema, dialect, &local));
                    for j in &tw.joins {
                        out.extend(relation_columns(&j.relation, schema, dialect, &local));
                    }
                }
            }
            SelectItem::QualifiedWildcard(name, _) => {
                let Some(last) = name.0.last() else { continue };
                let qual = fold_ident(last, dialect);
                for tw in &select.from {
                    out.extend(relation_columns_if_match(
                        &tw.relation,
                        schema,
                        dialect,
                        &local,
                        &qual,
                    ));
                    for j in &tw.joins {
                        out.extend(relation_columns_if_match(
                            &j.relation,
                            schema,
                            dialect,
                            &local,
                            &qual,
                        ));
                    }
                }
            }
            // Bare expressions without aliases project no name we can track.
            SelectItem::UnnamedExpr(_) => {}
        }
    }
    out
}

/// Compute the projected columns of a derived `(SELECT ...) alias` table
/// and stash them under the alias in `extras`. Used during wildcard
/// expansion to make derived-table columns visible to the outer projection,
/// and reused at ORDER BY time so outer references resolve.
pub(crate) fn publish_derived(
    factor: &TableFactor,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &mut Extras,
) {
    match factor {
        TableFactor::Derived {
            subquery, alias, ..
        } => {
            let Some(alias) = alias else { return };
            let cols: HashSet<String> = project_columns(subquery, schema, dialect, extras)
                .iter()
                .map(|i| fold_ident(i, dialect))
                .collect();
            extras.insert(fold_ident(&alias.name, dialect), cols);
        }
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => {
            publish_derived(&table_with_joins.relation, schema, dialect, extras);
            for j in &table_with_joins.joins {
                publish_derived(&j.relation, schema, dialect, extras);
            }
        }
        _ => {}
    }
}

/// All columns of a single FROM-clause relation. Schema lookups handle
/// qualified and bare names; CTE/derived references fall back to `extras`.
fn relation_columns(
    factor: &TableFactor,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Vec<Ident> {
    match factor {
        TableFactor::Table { name, .. } => {
            let cols_from_schema = if name.0.len() > 1 {
                schema.get(&qualified_key(name, dialect))
            } else {
                name.0
                    .last()
                    .and_then(|n| schema.get(&fold_ident(n, dialect)))
            };
            if let Some(cols) = cols_from_schema {
                return cols.iter().map(|c| Ident::new(c.as_str())).collect();
            }
            // Fall through to extras so CTE references in `SELECT * FROM cte`
            // expand correctly.
            if let Some(last) = name.0.last() {
                let key = fold_ident(last, dialect);
                if let Some(cols) = extras.get(&key) {
                    return cols.iter().map(|c| Ident::new(c.as_str())).collect();
                }
            }
            Vec::new()
        }
        TableFactor::Derived { alias, .. } => alias
            .as_ref()
            .and_then(|a| extras.get(&fold_ident(&a.name, dialect)))
            .map(|cols| cols.iter().map(|c| Ident::new(c.as_str())).collect())
            .unwrap_or_default(),
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => {
            let mut out = relation_columns(&table_with_joins.relation, schema, dialect, extras);
            for j in &table_with_joins.joins {
                out.extend(relation_columns(&j.relation, schema, dialect, extras));
            }
            out
        }
        _ => Vec::new(),
    }
}

fn relation_columns_if_match(
    factor: &TableFactor,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
    qualifier: &str,
) -> Vec<Ident> {
    if relation_qualifier_matches(factor, dialect, qualifier) {
        return relation_columns(factor, schema, dialect, extras);
    }
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = factor
    {
        let mut out = relation_columns_if_match(
            &table_with_joins.relation,
            schema,
            dialect,
            extras,
            qualifier,
        );
        for j in &table_with_joins.joins {
            out.extend(relation_columns_if_match(
                &j.relation,
                schema,
                dialect,
                extras,
                qualifier,
            ));
        }
        return out;
    }
    Vec::new()
}

fn relation_qualifier_matches(factor: &TableFactor, dialect: Dialect, qualifier: &str) -> bool {
    let folded = match factor {
        TableFactor::Table { name, alias, .. } => {
            if let Some(a) = alias {
                fold_ident(&a.name, dialect)
            } else {
                let Some(last) = name.0.last() else {
                    return false;
                };
                fold_ident(last, dialect)
            }
        }
        TableFactor::Derived { alias, .. } => {
            let Some(a) = alias else { return false };
            fold_ident(&a.name, dialect)
        }
        _ => return false,
    };
    folded == qualifier
}

fn extract_derived_from_factors(
    select: &sqlparser::ast::Select,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &mut Extras,
    errors: &mut Vec<String>,
) {
    for table in &select.from {
        walk_factor_for_derived(&table.relation, schema, dialect, extras, errors);
        for join in &table.joins {
            walk_factor_for_derived(&join.relation, schema, dialect, extras, errors);
        }
    }
}

fn walk_factor_for_derived(
    factor: &TableFactor,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &mut Extras,
    errors: &mut Vec<String>,
) {
    match factor {
        TableFactor::Derived {
            subquery, alias, ..
        } => {
            let Some(alias) = alias.as_ref() else {
                return;
            };

            // Recursive validation: uses its own fresh extras, so inner CTEs
            // don't leak out. Inner errors bubble up to the outer error list.
            errors.extend(validate_query_with_schema(
                subquery.as_ref(),
                schema,
                dialect,
            ));

            let cols: HashSet<String> = project_columns(subquery.as_ref(), schema, dialect, extras)
                .iter()
                .map(|i| fold_ident(i, dialect))
                .collect();
            extras.insert(fold_ident(&alias.name, dialect), cols);
        }
        // A parenthesized join group can contain derived tables that should
        // still be exposed in the surrounding scope.
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => {
            walk_factor_for_derived(&table_with_joins.relation, schema, dialect, extras, errors);
            for join in &table_with_joins.joins {
                walk_factor_for_derived(&join.relation, schema, dialect, extras, errors);
            }
        }
        _ => {}
    }
}

fn validate_and_extract_subqueries(
    query: &Query,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &mut Extras,
    errors: &mut Vec<String>,
) {
    let Some(with) = &query.with else {
        return;
    };

    let recursive = with.recursive;

    for derived in &with.cte_tables {
        if let Some(derived_from) = &derived.from {
            // CTE `… FROM other_cte` references must resolve against the
            // schema (folded by current dialect).
            let key = crate::schema::sql::fold_ident(derived_from, dialect);
            if !schema.contains_key(&key) {
                errors.push(format!(
                    "Table `{}` not found in schema nor subqueries",
                    derived_from.value
                ));
                continue;
            }
        }

        let derived_name = fold_ident(&derived.alias.name, dialect);

        // For WITH RECURSIVE, the CTE's own name must be visible inside its
        // own body (so `WITH RECURSIVE t AS (SELECT 1 UNION SELECT x FROM t)`
        // doesn't flag `t` as unknown). Pre-populate with the declared
        // column list if one was given; otherwise fall back to the body's
        // projected columns after validation.
        if recursive {
            let declared_cols: HashSet<String> = derived
                .alias
                .columns
                .iter()
                .map(|c| fold_ident(c, dialect))
                .collect();
            extras.insert(derived_name.clone(), declared_cols);
        }

        // Validate the inner CTE body with the current `extras` (earlier
        // CTEs + — for recursive CTEs — self) visible.
        errors.extend(validate_query_with_scope(
            derived.query.as_ref(),
            schema,
            dialect,
            extras,
        ));

        // Publish this CTE's output columns so later CTEs and the outer
        // body can resolve against them. Explicit column lists
        // `WITH t(a, b) AS (...)` override the body's projection names.
        let derived_columns: HashSet<String> = if derived.alias.columns.is_empty() {
            project_columns(derived.query.as_ref(), schema, dialect, extras)
                .iter()
                .map(|i| fold_ident(i, dialect))
                .collect()
        } else {
            derived
                .alias
                .columns
                .iter()
                .map(|c| fold_ident(c, dialect))
                .collect()
        };
        extras.insert(derived_name, derived_columns);
    }
}
