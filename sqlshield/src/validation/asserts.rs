use std::collections::{HashMap, HashSet};

use crate::schema;
use crate::schema::sql::lc;

/// Returns `Some(name)` if the relation cannot be resolved against the schema
/// or CTE extras. Qualified references (`schema.table`) require an exact
/// qualified match; unqualified references match on the bare table name.
/// Comparisons are ASCII case-insensitive.
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
            let name_full_lc = lc(&name_full);

            if name.0.len() > 1 {
                if schema.contains_key(&name_full_lc) || extras_contains(extras, &name_full_lc) {
                    return None;
                }
                return Some(name_full);
            }

            let last_lc = lc(&name
                .0
                .last()
                .expect("sqlparser guarantees ObjectName has ≥1 ident")
                .value);
            if schema.contains_key(&last_lc) || extras_contains(extras, &last_lc) {
                return None;
            }
            Some(name_full)
        }
        _ => None,
    }
}

/// Case-insensitive lookup against the borrowed-`&str` extras map. Extras
/// keys come from query-side identifiers (CTE / derived-table aliases) and
/// aren't lowercased at insertion since they're tied to AST lifetimes.
pub(crate) fn extras_contains(extras: &HashMap<&str, HashSet<&str>>, key: &str) -> bool {
    extras.keys().any(|k| k.eq_ignore_ascii_case(key))
}

/// Case-insensitive lookup that returns the matching value.
pub(crate) fn extras_get<'a>(
    extras: &'a HashMap<&'a str, HashSet<&'a str>>,
    key: &str,
) -> Option<&'a HashSet<&'a str>> {
    extras
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

/// Case-insensitive `set.contains` for a borrowed `&str` set.
pub(crate) fn set_contains_ci(set: &HashSet<&str>, needle: &str) -> bool {
    set.iter().any(|s| s.eq_ignore_ascii_case(needle))
}
