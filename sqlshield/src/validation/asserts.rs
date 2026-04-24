use std::collections::{HashMap, HashSet};

use crate::schema;

pub fn is_relation_in_schema(
    relation: &sqlparser::ast::TableFactor,
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
) -> Option<String> {
    match relation {
        sqlparser::ast::TableFactor::Table { name, .. } => {
            // TODO support table name with schema prefixed instead of using last ident
            let table_name = name
                .0
                .last()
                .expect("sqlparser guarantees ObjectName has ≥1 ident")
                .value
                .as_str();
            if schema.contains_key(table_name) || extras.contains_key(table_name) {
                return None;
            }
            let name_full = name
                .0
                .iter()
                .map(|e| e.value.as_str())
                .collect::<Vec<&str>>()
                .join(".");
            Some(name_full)
        }
        _ => None,
    }
}
