use sqlparser::{ast::Statement, dialect::GenericDialect, parser::Parser};
use std::collections::{HashMap, HashSet};

pub fn load_schema(schema: &[u8]) -> Result<super::TablesAndColumns, String> {
    // TODO return err instead of panics

    let schema_str = String::from_utf8_lossy(schema);

    let dialect = GenericDialect {};
    let statements =
        Parser::parse_sql(&dialect, schema_str.as_ref()).expect("Could not parse schema file");

    let mut tables: HashMap<String, HashSet<String>> = HashMap::new();
    for statement in statements {
        match statement {
            Statement::CreateTable { columns, name, .. } => {
                // ! Ignoring schema, by getting last ident only gets table name
                let last_ident = name.0.last().unwrap();
                let columns_set: HashSet<String> =
                    HashSet::from_iter(columns.iter().map(|e| e.name.value.clone()));
                tables.insert(last_ident.value.clone(), columns_set);
            }
            _ => {}
        }
    }
    return Ok(tables);
}
