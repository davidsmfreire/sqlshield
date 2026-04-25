//! INSERT INTO ... validation: target table and explicit column list.

use sqlparser::ast::{Ident, ObjectName};

use crate::dialect::Dialect;
use crate::schema::sql::fold_ident;
use crate::schema::TablesAndColumns;

use super::table_ref::{display_name, resolve_table_columns};

pub(crate) fn validate_insert(
    table_name: &ObjectName,
    columns: &[Ident],
    schema: &TablesAndColumns,
    dialect: Dialect,
) -> Vec<String> {
    let mut errors = Vec::new();

    let Some(cols) = resolve_table_columns(table_name, schema, dialect) else {
        errors.push(format!(
            "Table `{}` not found in schema nor subqueries",
            display_name(table_name)
        ));
        return errors;
    };

    for col in columns {
        if !cols.contains(&fold_ident(col, dialect)) {
            errors.push(format!(
                "Column `{}` not found in table `{}`",
                col.value,
                display_name(table_name)
            ));
        }
    }

    errors
}
