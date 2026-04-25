//! UPDATE ... SET ... WHERE ... validation.

use std::collections::HashSet;

use sqlparser::ast::{Assignment, Expr, TableFactor, TableWithJoins};

use crate::dialect::Dialect;
use crate::schema::sql::fold_ident;
use crate::schema::TablesAndColumns;
use crate::validation::{asserts, Extras};

use super::select::{collect_visible_relations, validate_expr_column_refs};
use super::table_ref::display_name;

pub(crate) fn validate_update(
    table: &TableWithJoins,
    assignments: &[Assignment],
    from: Option<&TableWithJoins>,
    selection: Option<&Expr>,
    schema: &TablesAndColumns,
    dialect: Dialect,
    parent_extras: &Extras,
) -> Vec<String> {
    let mut errors = Vec::new();
    let extras: Extras = parent_extras.clone();

    // Target table must exist.
    if let Some(name) = asserts::is_relation_in_schema(&table.relation, schema, dialect, &extras) {
        errors.push(format!("Table `{name}` not found in schema nor subqueries"));
    }

    // Assignment targets: each `SET col = ...` column must exist in the target table.
    if let TableFactor::Table { name, .. } = &table.relation {
        if let Some(cols) = super::table_ref::resolve_table_columns(name, schema, dialect) {
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
    }

    // Build visible relations: target table (+ joins) plus any FROM addition.
    let mut relation_sources: Vec<TableWithJoins> = vec![table.clone()];
    if let Some(f) = from {
        relation_sources.push(f.clone());
    }
    let visible = collect_visible_relations(&relation_sources);

    // UPDATE has no projection aliases.
    let no_aliases: HashSet<&str> = HashSet::new();

    // Validate WHERE expressions' column refs.
    if let Some(where_expr) = selection {
        validate_expr_column_refs(
            where_expr,
            &visible,
            schema,
            dialect,
            &extras,
            &no_aliases,
            &mut errors,
        );
    }

    // Validate assignment RHS expressions' column refs.
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

    errors
}
