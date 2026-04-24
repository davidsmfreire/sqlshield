use crate::{schema, validation::asserts};

use super::ClauseValidation;

use std::collections::{HashMap, HashSet};

impl ClauseValidation for sqlparser::ast::Select {
    fn validate(
        &self,
        schema: &schema::TablesAndColumns,
        extras: &HashMap<&str, HashSet<&str>>,
    ) -> Vec<String> {
        let select = self;
        let mut errors = vec![];

        for item in &select.from {
            if let Some(relation_name) =
                asserts::is_relation_in_schema(&item.relation, schema, extras)
            {
                errors.push(format!(
                    "Table `{relation_name}` not found in schema nor subqueries"
                ))
            }

            for join in &item.joins {
                if let Some(relation_name) =
                    asserts::is_relation_in_schema(&join.relation, schema, extras)
                {
                    errors.push(format!(
                        "Table `{relation_name}` not found in schema nor subqueries"
                    ))
                }
            }
        }

        for item in &select.projection {
            let result = is_select_item_in_relations(item, &select.from, schema, extras);

            if let Some((item_name, relations_not_found_in)) = result {
                if let [table] = relations_not_found_in.as_slice() {
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

fn is_select_item_in_relations<'a>(
    item: &'a sqlparser::ast::SelectItem,
    tables: &'a [sqlparser::ast::TableWithJoins],
    schema: &'a schema::TablesAndColumns,
    extras: &HashMap<&'a str, HashSet<&'a str>>,
) -> Option<(&'a str, Vec<&'a str>)> {
    let mut tables_searched_where_not_found: Vec<&str> = vec![];
    let mut item_name: Option<&str> = None;

    for relation in tables {
        if let Some((col_name, table_name)) =
            could_select_item_be_in_relation(item, &relation.relation, schema, extras)
        {
            tables_searched_where_not_found.push(table_name);
            if item_name.is_none() {
                item_name = Some(col_name);
            }
        }
        for join in &relation.joins {
            if let Some((col_name, table_name)) =
                could_select_item_be_in_relation(item, &join.relation, schema, extras)
            {
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

fn could_select_item_be_in_relation<'a>(
    item: &'a sqlparser::ast::SelectItem,
    table: &'a sqlparser::ast::TableFactor,
    schema: &'a schema::TablesAndColumns,
    extras: &HashMap<&'a str, HashSet<&'a str>>,
) -> Option<(&'a str, &'a str)> {
    // returns item_name, table_name if item could be in table but is not

    let (col_name, col_table_alias): (Option<&str>, Option<&str>) = match item {
        sqlparser::ast::SelectItem::UnnamedExpr(expression) => match expression {
            sqlparser::ast::Expr::Identifier(identifier) => (Some(identifier.value.as_str()), None),
            sqlparser::ast::Expr::CompoundIdentifier(identifier) if identifier.len() == 2 => (
                Some(identifier[1].value.as_str()),
                Some(identifier[0].value.as_str()),
            ),
            _ => (None, None),
        },
        // TODO: aliased columns
        // sqlparser::ast::SelectItem::ExprWithAlias { expr, alias } => {},
        _ => (None, None),
    };

    let (table_name, alias) = match table {
        sqlparser::ast::TableFactor::Table { name, alias, .. } => (
            name.0
                .last()
                .expect("sqlparser guarantees ObjectName has ≥1 ident")
                .value
                .as_str(),
            alias.as_ref(),
        ),
        // TODO Implement for others
        _ => return None,
    };

    let should_check = match (alias, col_table_alias) {
        (None, None) => true,
        (Some(table_alias), Some(col_alias)) => table_alias.name.value == col_alias,
        _ => false,
    };

    if !should_check {
        return None;
    }

    let col_name = col_name?;

    let column_present = if let Some(cols) = schema.get(table_name) {
        cols.contains(col_name)
    } else if let Some(cols) = extras.get(table_name) {
        cols.contains(col_name)
    } else {
        return None;
    };

    if column_present {
        None
    } else {
        Some((col_name, table_name))
    }
}
