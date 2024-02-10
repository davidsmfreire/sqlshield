use crate::{schema, validation::asserts};

use super::ClauseValidation;

use std::collections::HashSet;

impl ClauseValidation for sqlparser::ast::Select {
    fn validate(&self, schema: &schema::TablesAndColumns) -> Vec<String> {
        let tables_in_schema: HashSet<String> = HashSet::from_iter(schema.keys().cloned());

        let select = self;
        let mut errors = vec![];

        for item in &select.from {
            let relation_name = asserts::is_relation_in_schema(&item.relation, &tables_in_schema);

            if let Some(relation_name) = relation_name {
                errors.push(format!(
                    "Table `{relation_name}` not found in schema nor subqueries"
                ))
            }

            for join in &item.joins {
                let relation_name =
                    asserts::is_relation_in_schema(&join.relation, &tables_in_schema);
                if let Some(relation_name) = relation_name {
                    errors.push(format!(
                        "Table `{relation_name}` not found in schema nor subqueries"
                    ))
                }
            }
        }

        for item in &select.projection {
            let result = is_select_item_in_relations(item, &select.from, &schema);

            if let Some((item_name, relations_not_found_in)) = result {
                if relations_not_found_in.len() == 1 {
                    let table = relations_not_found_in.first().unwrap();
                    errors.push(format!("Column `{item_name}` not found in table `{table}`"))
                } else {
                    let not_found_on = relations_not_found_in.join(",");
                    errors.push(format!(
                        "Column `{item_name}` not found in none of the tables: {not_found_on}"
                    ))
                }
            }
        }
        errors
    }
}

fn is_select_item_in_relations(
    item: &sqlparser::ast::SelectItem,
    tables: &Vec<sqlparser::ast::TableWithJoins>,
    schema: &schema::TablesAndColumns,
) -> Option<(String, Vec<String>)> {
    let mut tables_searched_where_not_found: Vec<String> = vec![];
    let mut item_name: Option<String> = None;

    for relation in tables {
        let result = could_select_item_be_in_relation(&item, &relation.relation, &schema);
        if let Some((col_name, table_name)) = result {
            tables_searched_where_not_found.push(table_name);
            if item_name.is_none() {
                item_name = Some(col_name);
            }
        }
        for join in &relation.joins {
            let result = could_select_item_be_in_relation(&item, &join.relation, &schema);
            if let Some((col_name, table_name)) = result {
                tables_searched_where_not_found.push(table_name);
                if item_name.is_none() {
                    item_name = Some(col_name);
                }
            }
        }
    }
    if tables_searched_where_not_found.is_empty() {
        return None;
    }

    Some((item_name?, tables_searched_where_not_found))
}

fn could_select_item_be_in_relation(
    item: &sqlparser::ast::SelectItem,
    table: &sqlparser::ast::TableFactor,
    schema: &schema::TablesAndColumns,
) -> Option<(String, String)> {
    // returns item_name, table_name if item could be in table but is not

    let mut columns: Option<&HashSet<String>> = None;
    let mut col_name: Option<String> = None;
    let mut col_table_alias: Option<String> = None;
    // let mut col_alias: Option<String> = None;

    let mut table_name: Option<String> = None;

    match &item {
        sqlparser::ast::SelectItem::UnnamedExpr(expression) => {
            match expression {
                sqlparser::ast::Expr::Identifier(identifier) => {
                    col_name = Some(identifier.value.clone());
                }
                sqlparser::ast::Expr::CompoundIdentifier(identifier) => {
                    // for now only supports table alias
                    if identifier.len() == 2 {
                        col_table_alias = Some(identifier[0].value.clone());
                        col_name = Some(identifier[1].value.clone());
                    }
                }
                _ => {}
            }
        }
        // TODO: aliased columns
        // sqlparser::ast::SelectItem::ExprWithAlias { expr, alias } => {},
        _ => {}
    }

    match &table {
        sqlparser::ast::TableFactor::Table { name, alias, .. } => {
            let name = &name.0.last().unwrap().value;

            match (alias, col_table_alias) {
                (None, None) => {
                    columns = schema.get(name);
                }
                (None, Some(_)) => {}
                (Some(_), None) => {}
                (Some(alias), Some(col_table_alias)) => {
                    if alias.name.value == col_table_alias {
                        columns = schema.get(name);
                    }
                }
            }
            table_name = Some(name.clone());
        }
        // TODO Implement for others
        _ => (),
    }

    if let (Some(columns), Some(col_name)) = (columns, col_name) {
        if !columns.contains(col_name.as_str()) {
            return Some((col_name, table_name?));
        }
    }

    None
}
