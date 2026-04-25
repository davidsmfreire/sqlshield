//! Helpers for looking up a table by `ObjectName` — shared across DML validators.

use std::collections::HashSet;

use sqlparser::ast::ObjectName;

use crate::dialect::Dialect;
use crate::schema::sql::{fold_ident, qualified_key};
use crate::schema::TablesAndColumns;

/// Human-readable form of `ObjectName`: `public.users` or `users`.
/// Preserves the user's casing for error messages.
pub(crate) fn display_name(name: &ObjectName) -> String {
    name.0
        .iter()
        .map(|p| p.value.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

/// Resolve a table reference to its column set, respecting qualified vs.
/// unqualified lookup semantics (see `asserts::is_relation_in_schema`).
/// Identifier matching honors the dialect's folding rules.
pub(crate) fn resolve_table_columns<'a>(
    name: &ObjectName,
    schema: &'a TablesAndColumns,
    dialect: Dialect,
) -> Option<&'a HashSet<String>> {
    if name.0.len() > 1 {
        return schema.get(&qualified_key(name, dialect));
    }
    let last = name.0.last()?;
    schema.get(&fold_ident(last, dialect))
}
