use sqlparser::{
    ast::{AlterTableOperation, ColumnDef, ObjectName, Statement},
    dialect::GenericDialect,
    parser::Parser,
};
use std::collections::{HashMap, HashSet};

use crate::error::Result;

pub fn load_schema(schema: &[u8]) -> Result<super::TablesAndColumns> {
    let schema_str = String::from_utf8_lossy(schema);

    let dialect = GenericDialect {};
    let statements = Parser::parse_sql(&dialect, schema_str.as_ref())?;

    let mut tables: HashMap<String, HashSet<String>> = HashMap::new();
    for statement in statements {
        match statement {
            Statement::CreateTable { columns, name, .. } => {
                ingest_create_table(&name, &columns, &mut tables);
            }
            Statement::AlterTable {
                name, operations, ..
            } => {
                apply_alters(&name, &operations, &mut tables);
            }
            _ => {}
        }
    }
    Ok(tables)
}

fn ingest_create_table(
    name: &ObjectName,
    columns: &[ColumnDef],
    tables: &mut HashMap<String, HashSet<String>>,
) {
    let Some(last_ident) = name.0.last() else {
        return;
    };
    let columns_set: HashSet<String> =
        HashSet::from_iter(columns.iter().map(|e| e.name.value.clone()));

    // Store the bare table name so unqualified queries resolve; if the
    // schema was declared as `schema.table`, ALSO store the fully
    // qualified form so qualified queries can be resolved strictly.
    tables.insert(last_ident.value.clone(), columns_set.clone());
    if name.0.len() > 1 {
        tables.insert(display_name(name), columns_set);
    }
}

fn apply_alters(
    name: &ObjectName,
    operations: &[AlterTableOperation],
    tables: &mut HashMap<String, HashSet<String>>,
) {
    // ALTER TABLE updates both the bare and (if applicable) the qualified
    // twin so the two keep in sync after migrations. Unknown tables are
    // silently skipped — schema files often list ops in dependency order
    // and over-strict validation here trips real-world dumps.
    for key in target_keys(name, tables) {
        let Some(cols) = tables.get_mut(&key) else {
            continue;
        };
        for op in operations {
            apply_one(cols, op);
        }
    }
}

fn target_keys(name: &ObjectName, tables: &HashMap<String, HashSet<String>>) -> Vec<String> {
    let Some(last) = name.0.last() else {
        return Vec::new();
    };
    let bare = last.value.as_str();
    if name.0.len() > 1 {
        // Qualified ALTER: target only the exact qualified key.
        let q = display_name(name);
        if tables.contains_key(&q) {
            return vec![q];
        }
        return Vec::new();
    }
    // Bare ALTER: target the bare key plus any qualified twin whose last
    // segment matches (so `CREATE TABLE public.users; ALTER TABLE users …`
    // updates both entries).
    tables
        .keys()
        .filter(|k| {
            k.as_str() == bare
                || k.rsplit('.')
                    .next()
                    .is_some_and(|seg| seg == bare && k != &bare)
        })
        .cloned()
        .collect()
}

fn apply_one(cols: &mut HashSet<String>, op: &AlterTableOperation) {
    match op {
        AlterTableOperation::AddColumn { column_def, .. } => {
            cols.insert(column_def.name.value.clone());
        }
        AlterTableOperation::DropColumn { column_name, .. } => {
            cols.remove(column_name.value.as_str());
        }
        AlterTableOperation::RenameColumn {
            old_column_name,
            new_column_name,
        } => {
            if cols.remove(old_column_name.value.as_str()) {
                cols.insert(new_column_name.value.clone());
            }
        }
        // Other ops (constraints, RLS, RENAME TABLE, …) don't change the
        // column set we track.
        _ => {}
    }
}

fn display_name(name: &ObjectName) -> String {
    name.0
        .iter()
        .map(|p| p.value.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::load_schema;

    #[test]
    fn test_load_schema() {
        let schema = "
            CREATE TABLE users (
                id INT PRIMARY KEY AUTO_INCREMENT,
                name VARCHAR(255) NOT NULL
            );
            CREATE TABLE receipt (
                id INT PRIMARY KEY AUTO_INCREMENT,
                content VARCHAR(128),
                user_id INT,
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
        ";

        let expected_result: HashMap<String, HashSet<String>> = HashMap::from([
            ("users".into(), HashSet::from(["id".into(), "name".into()])),
            (
                "receipt".into(),
                HashSet::from(["id".into(), "content".into(), "user_id".into()]),
            ),
        ]);

        let result = load_schema(schema.as_bytes()).unwrap();

        assert_eq!(result, expected_result);
    }
}
