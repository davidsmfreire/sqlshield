//! UPDATE ... SET ... WHERE ... validation.

use std::collections::{HashMap, HashSet};

use sqlparser::ast::{Assignment, Expr, TableFactor, TableWithJoins};

use crate::schema::TablesAndColumns;
use crate::validation::asserts;

use super::select::{collect_visible_relations, validate_expr_column_refs};
use super::table_ref::display_name;

pub(crate) fn validate_update(
    table: &TableWithJoins,
    assignments: &[Assignment],
    from: Option<&TableWithJoins>,
    selection: Option<&Expr>,
    schema: &TablesAndColumns,
) -> Vec<String> {
    let mut errors = Vec::new();
    let extras: HashMap<&str, HashSet<&str>> = HashMap::new();

    // Target table must exist.
    if let Some(name) = asserts::is_relation_in_schema(&table.relation, schema, &extras) {
        errors.push(format!("Table `{name}` not found in schema nor subqueries"));
    }

    // Assignment targets: each `SET col = ...` column must exist in the target table.
    if let TableFactor::Table { name, .. } = &table.relation {
        if let Some(cols) = super::table_ref::resolve_table_columns(name, schema) {
            for assignment in assignments {
                let Some(last) = assignment.id.last() else {
                    continue;
                };
                if !cols.contains(last.value.as_str()) {
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

    // Validate WHERE expressions' column refs.
    if let Some(where_expr) = selection {
        validate_expr_column_refs(where_expr, &visible, schema, &extras, &mut errors);
    }

    // Validate assignment RHS expressions' column refs.
    for assignment in assignments {
        validate_expr_column_refs(&assignment.value, &visible, schema, &extras, &mut errors);
    }

    errors
}
