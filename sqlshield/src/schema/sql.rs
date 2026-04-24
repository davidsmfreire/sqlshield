use sqlparser::{ast::Statement, dialect::GenericDialect, parser::Parser};
use std::collections::{HashMap, HashSet};

pub fn load_schema(schema: &[u8]) -> Result<super::TablesAndColumns, String> {
    let schema_str = String::from_utf8_lossy(schema);

    let dialect = GenericDialect {};
    let statements = match Parser::parse_sql(&dialect, schema_str.as_ref()) {
        Ok(statements) => statements,
        Err(err) => return Err(format!("Could not parse schema file: {err}")),
    };

    let mut tables: HashMap<String, HashSet<String>> = HashMap::new();
    for statement in statements {
        if let Statement::CreateTable { columns, name, .. } = statement {
            // ! Ignoring schema, by getting last ident only gets table name
            let last_ident = name.0.last().unwrap();
            let columns_set: HashSet<String> =
                HashSet::from_iter(columns.iter().map(|e| e.name.value.clone()));
            tables.insert(last_ident.value.clone(), columns_set);
        }
    }
    Ok(tables)
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
