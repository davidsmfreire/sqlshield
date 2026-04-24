//! DELETE FROM ... WHERE ... validation.

use std::collections::{HashMap, HashSet};

use sqlparser::ast::{Expr, TableWithJoins};

use crate::schema::TablesAndColumns;
use crate::validation::asserts;

use super::select::{collect_visible_relations, validate_expr_column_refs};

pub(crate) fn validate_delete(
    from: &[TableWithJoins],
    using: Option<&[TableWithJoins]>,
    selection: Option<&Expr>,
    schema: &TablesAndColumns,
) -> Vec<String> {
    let mut errors = Vec::new();
    let extras: HashMap<&str, HashSet<&str>> = HashMap::new();

    for table in from {
        if let Some(name) = asserts::is_relation_in_schema(&table.relation, schema, &extras) {
            errors.push(format!("Table `{name}` not found in schema nor subqueries"));
        }
        for join in &table.joins {
            if let Some(name) = asserts::is_relation_in_schema(&join.relation, schema, &extras) {
                errors.push(format!("Table `{name}` not found in schema nor subqueries"));
            }
        }
    }

    if let Some(using_tables) = using {
        for table in using_tables {
            if let Some(name) = asserts::is_relation_in_schema(&table.relation, schema, &extras) {
                errors.push(format!("Table `{name}` not found in schema nor subqueries"));
            }
        }
    }

    // Build visible set from FROM + USING (both contribute to WHERE scope).
    let mut all_sources: Vec<TableWithJoins> = from.to_vec();
    if let Some(u) = using {
        all_sources.extend(u.iter().cloned());
    }
    let visible = collect_visible_relations(&all_sources);

    if let Some(where_expr) = selection {
        validate_expr_column_refs(where_expr, &visible, schema, &extras, &mut errors);
    }

    errors
}
