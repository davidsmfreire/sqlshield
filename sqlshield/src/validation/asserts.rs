use std::collections::HashSet;

pub fn is_relation_in_schema(
    relation: &sqlparser::ast::TableFactor,
    tables: &HashSet<String>,
) -> Option<String> {
    // returns table_name if not in schema
    match &relation {
        sqlparser::ast::TableFactor::Table { name, .. } => {
            // TODO support table name with schema prefixed instead of using last ident
            let table_name = name.0.last().unwrap();
            let table_name_str = table_name.value.as_str();
            if tables.contains(table_name_str) {
                return None;
            }
            let name_full: String = name
                .0
                .iter()
                .map(|e| e.value.as_str())
                .collect::<Vec<&str>>()
                .join(".");
            return Some(name_full);
        }
        _ => {}
    }
    None
}
