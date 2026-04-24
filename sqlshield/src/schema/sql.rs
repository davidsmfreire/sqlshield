use sqlparser::{
    ast::{
        AlterTableOperation, ColumnDef, Expr, ObjectName, Query, SelectItem, SetExpr, Statement,
        ViewColumnDef,
    },
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
            Statement::CreateTable {
                columns,
                name,
                query,
                ..
            } => {
                ingest_create_table(&name, &columns, query.as_deref(), &mut tables);
            }
            Statement::AlterTable {
                name, operations, ..
            } => {
                apply_alters(&name, &operations, &mut tables);
            }
            Statement::CreateView {
                name,
                columns,
                query,
                ..
            } => {
                ingest_create_view(&name, &columns, &query, &mut tables);
            }
            _ => {}
        }
    }
    Ok(tables)
}

fn ingest_create_table(
    name: &ObjectName,
    columns: &[ColumnDef],
    query: Option<&Query>,
    tables: &mut HashMap<String, HashSet<String>>,
) {
    let Some(last_ident) = name.0.last() else {
        return;
    };

    // CREATE TABLE … AS SELECT carries no explicit column list; infer the
    // names from the source query's projection. Plain CREATE TABLE uses
    // the explicit list. If both are present, the explicit list wins.
    let columns_set: HashSet<String> = if !columns.is_empty() {
        columns.iter().map(|e| lc(&e.name.value)).collect()
    } else if let Some(q) = query {
        project_column_names(q)
            .into_iter()
            .map(|s| lc(&s))
            .collect()
    } else {
        HashSet::new()
    };

    // Store the bare table name so unqualified queries resolve; if the
    // schema was declared as `schema.table`, ALSO store the fully
    // qualified form so qualified queries can be resolved strictly.
    // Both keys are case-folded: identifier matching is ASCII case-insensitive
    // throughout sqlshield.
    tables.insert(lc(&last_ident.value), columns_set.clone());
    if name.0.len() > 1 {
        tables.insert(lc(&display_name(name)), columns_set);
    }
}

fn ingest_create_view(
    name: &ObjectName,
    columns: &[ViewColumnDef],
    query: &Query,
    tables: &mut HashMap<String, HashSet<String>>,
) {
    let Some(last_ident) = name.0.last() else {
        return;
    };
    // Explicit column list `CREATE VIEW v(a, b) AS …` overrides whatever
    // names the body projects.
    let columns_set: HashSet<String> = if !columns.is_empty() {
        columns.iter().map(|c| lc(&c.name.value)).collect()
    } else {
        project_column_names(query)
            .into_iter()
            .map(|s| lc(&s))
            .collect()
    };
    tables.insert(lc(&last_ident.value), columns_set.clone());
    if name.0.len() > 1 {
        tables.insert(lc(&display_name(name)), columns_set);
    }
}

/// Case-fold to ASCII lowercase. Used at every identifier insertion and
/// lookup site so the schema map and query-side identifiers compare
/// case-insensitively.
pub(crate) fn lc(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// Owned-string version of `validation::project_columns`. Used at schema-
/// ingestion time to capture the column names that a view or CTAS body
/// projects.
fn project_column_names(query: &Query) -> Vec<String> {
    project_names_of_body(query.body.as_ref())
}

fn project_names_of_body(body: &SetExpr) -> Vec<String> {
    let mut names = Vec::new();
    match body {
        SetExpr::Select(select_box) => {
            for item in &select_box.projection {
                match item {
                    SelectItem::UnnamedExpr(expr) => match expr {
                        Expr::Identifier(ident) => names.push(ident.value.clone()),
                        Expr::CompoundIdentifier(idents) => {
                            if let Some(last) = idents.last() {
                                names.push(last.value.clone());
                            }
                        }
                        _ => {}
                    },
                    SelectItem::ExprWithAlias { alias, .. } => {
                        names.push(alias.value.clone());
                    }
                    _ => {}
                }
            }
        }
        // For UNION/etc., the output names are taken from the left branch.
        SetExpr::SetOperation { left, .. } => {
            names.extend(project_names_of_body(left.as_ref()));
        }
        SetExpr::Query(inner) => {
            names.extend(project_column_names(inner.as_ref()));
        }
        _ => {}
    }
    names
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
    let bare = lc(&last.value);
    if name.0.len() > 1 {
        // Qualified ALTER: target only the exact qualified key.
        let q = lc(&display_name(name));
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
                    .is_some_and(|seg| seg == bare && k.as_str() != bare)
        })
        .cloned()
        .collect()
}

fn apply_one(cols: &mut HashSet<String>, op: &AlterTableOperation) {
    match op {
        AlterTableOperation::AddColumn { column_def, .. } => {
            cols.insert(lc(&column_def.name.value));
        }
        AlterTableOperation::DropColumn { column_name, .. } => {
            cols.remove(lc(&column_name.value).as_str());
        }
        AlterTableOperation::RenameColumn {
            old_column_name,
            new_column_name,
        } => {
            if cols.remove(lc(&old_column_name.value).as_str()) {
                cols.insert(lc(&new_column_name.value));
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
