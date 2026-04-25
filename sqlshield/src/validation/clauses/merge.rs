//! MERGE INTO ... USING ... ON ... WHEN [NOT] MATCHED THEN ... validation.
//!
//! Covers the subset shared by Postgres 15+, Snowflake, BigQuery, Oracle,
//! and SQL Server: a target table, a source (table or subquery), an ON
//! join predicate, and any number of WHEN MATCHED / WHEN NOT MATCHED
//! branches. We validate:
//!
//! * Target and source tables exist (when the source is a real table).
//! * The ON predicate's column refs resolve in the combined scope.
//! * Each MATCHED UPDATE assignment column is valid in the target.
//! * Each MATCHED predicate / UPDATE-RHS column ref resolves.
//! * Each NOT MATCHED INSERT column list is a subset of the target's
//!   columns (mirrors `INSERT … (cols)` validation).

use std::collections::HashSet;

use sqlparser::ast::{Expr, MergeClause, TableFactor, TableWithJoins};

use crate::dialect::Dialect;
use crate::schema::sql::fold_ident;
use crate::schema::TablesAndColumns;
use crate::validation::{asserts, Extras};

use super::select::{collect_visible_relations, validate_expr_column_refs};
use super::table_ref::{display_name, resolve_table_columns};

pub(crate) fn validate_merge(
    target: &TableFactor,
    source: &TableFactor,
    on: &Expr,
    clauses: &[MergeClause],
    schema: &TablesAndColumns,
    dialect: Dialect,
    parent_extras: &Extras,
) -> Vec<String> {
    let mut errors = Vec::new();
    let extras = parent_extras.clone();

    // Both target and source are TableFactors. Run them through the same
    // schema check used elsewhere — Derived (subquery) sources slip past
    // here without error, which is what we want; their columns surface via
    // the visible-relations machinery below.
    if let Some(name) = asserts::is_relation_in_schema(target, schema, dialect, &extras) {
        errors.push(format!("Table `{name}` not found in schema nor subqueries"));
    }
    if let Some(name) = asserts::is_relation_in_schema(source, schema, dialect, &extras) {
        errors.push(format!("Table `{name}` not found in schema nor subqueries"));
    }

    // Build a TableWithJoins for each side so we can reuse the
    // visible-relations + expr-walker stack from SELECT.
    let target_tw = TableWithJoins {
        relation: target.clone(),
        joins: vec![],
    };
    let source_tw = TableWithJoins {
        relation: source.clone(),
        joins: vec![],
    };
    let scope = [target_tw, source_tw];
    let visible = collect_visible_relations(&scope);
    let no_aliases: HashSet<&str> = HashSet::new();

    // ON predicate: resolves against target ∪ source.
    validate_expr_column_refs(
        on,
        &visible,
        schema,
        dialect,
        &extras,
        &no_aliases,
        &mut errors,
    );

    // Resolve target columns once for assignment / INSERT-list checks.
    let target_cols = match target {
        TableFactor::Table { name, .. } => {
            Some((name, resolve_table_columns(name, schema, dialect)))
        }
        _ => None,
    };

    for clause in clauses {
        match clause {
            MergeClause::MatchedUpdate {
                predicate,
                assignments,
            } => {
                if let Some(pred) = predicate {
                    validate_expr_column_refs(
                        pred,
                        &visible,
                        schema,
                        dialect,
                        &extras,
                        &no_aliases,
                        &mut errors,
                    );
                }
                if let Some((name, Some(cols))) = &target_cols {
                    for assignment in assignments {
                        let Some(last) = assignment.id.last() else {
                            continue;
                        };
                        if !cols.contains(&fold_ident(last, dialect)) {
                            errors.push(format!(
                                "Column `{}` not found in table `{}`",
                                last.value,
                                display_name(name)
                            ));
                        }
                    }
                }
                for assignment in assignments {
                    validate_expr_column_refs(
                        &assignment.value,
                        &visible,
                        schema,
                        dialect,
                        &extras,
                        &no_aliases,
                        &mut errors,
                    );
                }
            }
            MergeClause::MatchedDelete(predicate) => {
                if let Some(pred) = predicate {
                    validate_expr_column_refs(
                        pred,
                        &visible,
                        schema,
                        dialect,
                        &extras,
                        &no_aliases,
                        &mut errors,
                    );
                }
            }
            MergeClause::NotMatched {
                predicate,
                columns,
                values,
            } => {
                if let Some(pred) = predicate {
                    validate_expr_column_refs(
                        pred,
                        &visible,
                        schema,
                        dialect,
                        &extras,
                        &no_aliases,
                        &mut errors,
                    );
                }
                if let Some((name, Some(cols))) = &target_cols {
                    for col in columns {
                        if !cols.contains(&fold_ident(col, dialect)) {
                            errors.push(format!(
                                "Column `{}` not found in table `{}`",
                                col.value,
                                display_name(name)
                            ));
                        }
                    }
                }
                // VALUES (...) row expressions can reference source columns
                // (`USING t AS s … VALUES (s.x)`); validate against the
                // combined scope.
                for row in &values.rows {
                    for v in row {
                        validate_expr_column_refs(
                            v,
                            &visible,
                            schema,
                            dialect,
                            &extras,
                            &no_aliases,
                            &mut errors,
                        );
                    }
                }
            }
        }
    }

    errors
}
