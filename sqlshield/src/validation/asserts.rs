use std::collections::HashSet;

use crate::dialect::Dialect;
use crate::schema;
use crate::schema::sql::{fold_ident, qualified_key};

use super::Extras;

/// Returns `Some(name)` if the relation cannot be resolved against the schema
/// or CTE extras. Qualified references (`schema.table`) require an exact
/// qualified match; unqualified references match on the bare table name.
/// Identifier comparisons honor the dialect's folding rules.
pub fn is_relation_in_schema(
    relation: &sqlparser::ast::TableFactor,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Option<String> {
    match relation {
        sqlparser::ast::TableFactor::Table { name, .. } => {
            let display = name
                .0
                .iter()
                .map(|e| e.value.as_str())
                .collect::<Vec<&str>>()
                .join(".");

            if name.0.len() > 1 {
                let key = qualified_key(name, dialect);
                if schema.contains_key(&key) || extras.contains_key(&key) {
                    return None;
                }
                return Some(display);
            }

            let last = name
                .0
                .last()
                .expect("sqlparser guarantees ObjectName has ≥1 ident");
            let key = fold_ident(last, dialect);
            if schema.contains_key(&key) || extras.contains_key(&key) {
                return None;
            }
            Some(display)
        }
        _ => None,
    }
}

/// Lookup against the dialect-folded extras map. Caller passes the already-
/// folded key; this is just a thin wrapper to keep the call sites readable.
pub(crate) fn extras_get<'a>(extras: &'a Extras, key: &str) -> Option<&'a HashSet<String>> {
    extras.get(key)
}
