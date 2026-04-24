use std::collections::{HashMap, HashSet};

use crate::schema;

/// Returns `Some(name)` if the relation cannot be resolved against the schema
/// or CTE extras. Qualified references (`schema.table`) require an exact
/// qualified match; unqualified references match on the bare table name.
pub fn is_relation_in_schema(
    relation: &sqlparser::ast::TableFactor,
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
) -> Option<String> {
    match relation {
        sqlparser::ast::TableFactor::Table { name, .. } => {
            let name_full = name
                .0
                .iter()
                .map(|e| e.value.as_str())
                .collect::<Vec<&str>>()
                .join(".");

            if name.0.len() > 1 {
                if schema.contains_key(&name_full) || extras.contains_key(name_full.as_str()) {
                    return None;
                }
                return Some(name_full);
            }

            let last = name
                .0
                .last()
                .expect("sqlparser guarantees ObjectName has ≥1 ident")
                .value
                .as_str();
            if schema.contains_key(last) || extras.contains_key(last) {
                return None;
            }
            Some(name_full)
        }
        _ => None,
    }
}
