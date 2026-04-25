use sqlparser::{
    ast::{
        AlterTableOperation, ColumnDef, Expr, Ident, ObjectName, Query, SelectItem, SetExpr,
        Statement, ViewColumnDef,
    },
    dialect::GenericDialect,
    parser::Parser,
};
use std::collections::{HashMap, HashSet};

use crate::dialect::Dialect;
use crate::error::Result;

pub fn load_schema(schema: &[u8], dialect: Dialect) -> Result<super::TablesAndColumns> {
    let schema_str = String::from_utf8_lossy(schema);

    // Always parse with GenericDialect: schema files often mix DDL syntax
    // and `Generic` is the most permissive. Identifier folding is the part
    // that varies by dialect, not parsing.
    let parser_dialect = GenericDialect {};
    let statements = Parser::parse_sql(&parser_dialect, schema_str.as_ref())?;

    let mut tables: HashMap<String, HashSet<String>> = HashMap::new();
    for statement in statements {
        match statement {
            Statement::CreateTable {
                columns,
                name,
                query,
                ..
            } => {
                ingest_create_table(&name, &columns, query.as_deref(), dialect, &mut tables);
            }
            Statement::AlterTable {
                name, operations, ..
            } => {
                apply_alters(&name, &operations, dialect, &mut tables);
            }
            Statement::CreateView {
                name,
                columns,
                query,
                ..
            } => {
                ingest_create_view(&name, &columns, &query, dialect, &mut tables);
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
    dialect: Dialect,
    tables: &mut HashMap<String, HashSet<String>>,
) {
    let Some(last_ident) = name.0.last() else {
        return;
    };

    // CREATE TABLE … AS SELECT carries no explicit column list; infer the
    // names from the source query's projection. Plain CREATE TABLE uses
    // the explicit list. If both are present, the explicit list wins.
    let columns_set: HashSet<String> = if !columns.is_empty() {
        columns
            .iter()
            .map(|e| fold_ident(&e.name, dialect))
            .collect()
    } else if let Some(q) = query {
        project_column_names(q)
            .into_iter()
            .map(|i| fold_ident(&i, dialect))
            .collect()
    } else {
        HashSet::new()
    };

    // Store the bare table name so unqualified queries resolve; if the
    // schema was declared as `schema.table`, ALSO store the fully
    // qualified form so qualified queries can be resolved strictly.
    // Both keys are folded by the active dialect.
    tables.insert(fold_ident(last_ident, dialect), columns_set.clone());
    if name.0.len() > 1 {
        tables.insert(qualified_key(name, dialect), columns_set);
    }
}

fn ingest_create_view(
    name: &ObjectName,
    columns: &[ViewColumnDef],
    query: &Query,
    dialect: Dialect,
    tables: &mut HashMap<String, HashSet<String>>,
) {
    let Some(last_ident) = name.0.last() else {
        return;
    };
    // Explicit column list `CREATE VIEW v(a, b) AS …` overrides whatever
    // names the body projects.
    let columns_set: HashSet<String> = if !columns.is_empty() {
        columns
            .iter()
            .map(|c| fold_ident(&c.name, dialect))
            .collect()
    } else {
        project_column_names(query)
            .into_iter()
            .map(|i| fold_ident(&i, dialect))
            .collect()
    };
    tables.insert(fold_ident(last_ident, dialect), columns_set.clone());
    if name.0.len() > 1 {
        tables.insert(qualified_key(name, dialect), columns_set);
    }
}

/// Dialect-aware identifier folding. Postgres: quoted preserves case,
/// unquoted lowercases. All other dialects: ASCII lowercase regardless.
pub(crate) fn fold(s: &str, quoted: bool, dialect: Dialect) -> String {
    if dialect == Dialect::Postgres && quoted {
        s.to_string()
    } else {
        s.to_ascii_lowercase()
    }
}

pub(crate) fn fold_ident(ident: &Ident, dialect: Dialect) -> String {
    fold(&ident.value, ident.quote_style.is_some(), dialect)
}

/// Treat `s` as an unquoted identifier when folding. Use this only for
/// literal strings that never came from a quoted Ident (e.g., an alias
/// already stored unquoted).
pub(crate) fn fold_str(s: &str, dialect: Dialect) -> String {
    fold(s, false, dialect)
}

/// Per-part folded join of an `ObjectName` (`"Public".users` → `Public.users`
/// in Postgres mode; `public.users` everywhere else).
pub(crate) fn qualified_key(name: &ObjectName, dialect: Dialect) -> String {
    name.0
        .iter()
        .map(|p| fold_ident(p, dialect))
        .collect::<Vec<_>>()
        .join(".")
}

/// Owned version that captures projected column names as Idents (preserving
/// quote_style) so callers can fold them in dialect-aware fashion.
fn project_column_names(query: &Query) -> Vec<Ident> {
    project_names_of_body(query.body.as_ref())
}

fn project_names_of_body(body: &SetExpr) -> Vec<Ident> {
    let mut names = Vec::new();
    match body {
        SetExpr::Select(select_box) => {
            for item in &select_box.projection {
                match item {
                    SelectItem::UnnamedExpr(expr) => match expr {
                        Expr::Identifier(ident) => names.push(ident.clone()),
                        Expr::CompoundIdentifier(idents) => {
                            if let Some(last) = idents.last() {
                                names.push(last.clone());
                            }
                        }
                        _ => {}
                    },
                    SelectItem::ExprWithAlias { alias, .. } => {
                        names.push(alias.clone());
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
    dialect: Dialect,
    tables: &mut HashMap<String, HashSet<String>>,
) {
    // ALTER TABLE updates both the bare and (if applicable) the qualified
    // twin so the two keep in sync after migrations. Unknown tables are
    // silently skipped — schema files often list ops in dependency order
    // and over-strict validation here trips real-world dumps.
    for key in target_keys(name, dialect, tables) {
        let Some(cols) = tables.get_mut(&key) else {
            continue;
        };
        for op in operations {
            apply_one(cols, op, dialect);
        }
    }
}

fn target_keys(
    name: &ObjectName,
    dialect: Dialect,
    tables: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    let Some(last) = name.0.last() else {
        return Vec::new();
    };
    let bare = fold_ident(last, dialect);
    if name.0.len() > 1 {
        // Qualified ALTER: target only the exact qualified key.
        let q = qualified_key(name, dialect);
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

fn apply_one(cols: &mut HashSet<String>, op: &AlterTableOperation, dialect: Dialect) {
    match op {
        AlterTableOperation::AddColumn { column_def, .. } => {
            cols.insert(fold_ident(&column_def.name, dialect));
        }
        AlterTableOperation::DropColumn { column_name, .. } => {
            cols.remove(fold_ident(column_name, dialect).as_str());
        }
        AlterTableOperation::RenameColumn {
            old_column_name,
            new_column_name,
        } => {
            if cols.remove(fold_ident(old_column_name, dialect).as_str()) {
                cols.insert(fold_ident(new_column_name, dialect));
            }
        }
        // Other ops (constraints, RLS, RENAME TABLE, …) don't change the
        // column set we track.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::load_schema;
    use crate::dialect::Dialect;

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

        let result = load_schema(schema.as_bytes(), Dialect::default()).unwrap();

        assert_eq!(result, expected_result);
    }

    #[test]
    fn postgres_quoted_identifiers_preserve_case() {
        let schema = r#"CREATE TABLE "Users" ("Id" INT, "Name" VARCHAR(64));"#;
        let result = load_schema(schema.as_bytes(), Dialect::Postgres).unwrap();
        // Quoted identifiers in Postgres preserve case → key is `Users`,
        // not `users`.
        assert!(result.contains_key("Users"));
        assert!(!result.contains_key("users"));
        let cols = &result["Users"];
        assert!(cols.contains("Id"));
        assert!(cols.contains("Name"));
        assert!(!cols.contains("id"));
    }

    #[test]
    fn postgres_unquoted_identifiers_lowercased() {
        let schema = "CREATE TABLE Users (Id INT, Name VARCHAR(64));";
        let result = load_schema(schema.as_bytes(), Dialect::Postgres).unwrap();
        assert!(result.contains_key("users"));
        assert!(result["users"].contains("id"));
        assert!(result["users"].contains("name"));
    }

    #[test]
    fn non_postgres_dialect_lowercases_quoted_identifiers() {
        let schema = r#"CREATE TABLE "Users" ("Id" INT);"#;
        // MySQL and friends keep the legacy ASCII case-insensitive behavior.
        let result = load_schema(schema.as_bytes(), Dialect::MySql).unwrap();
        assert!(result.contains_key("users"));
        assert!(!result.contains_key("Users"));
    }
}
