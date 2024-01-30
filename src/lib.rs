use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::{fmt, fs};

use colored::Colorize;
use regex::Regex;
use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser as SqlParser;
use tree_sitter::{Node, Parser as CodeParser};
use walkdir::WalkDir;

struct SqlQueryError {
    line: usize,
    description: String,
}

struct QueryInCode {
    line: usize,
    statements: Vec<Statement>,
}

type TablesAndColumns = HashMap<String, HashSet<String>>;

#[derive(Debug, PartialEq)]
pub struct SqlValidationError {
    pub location: String,
    pub description: String,
}

impl SqlValidationError {
    pub fn new(file_path: &std::path::Path, line_number: usize, description: String) -> Self {
        let location = [
            file_path.to_string_lossy().to_string(),
            line_number.to_string(),
        ]
        .join(":");

        SqlValidationError {
            location: location,
            description: description,
        }
    }
}

impl fmt::Display for SqlValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}: {} {}",
            self.location,
            "error:".red(),
            self.description
        )
    }
}

fn schema_to_tables_and_columns(schema: &Vec<Statement>) -> TablesAndColumns {
    let mut tables: HashMap<String, HashSet<String>> = HashMap::new();
    for statement in schema {
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
    return tables;
}

fn is_relation_in_schema(
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

fn could_select_item_be_in_relation(
    item: &sqlparser::ast::SelectItem,
    table: &sqlparser::ast::TableFactor,
    schema: &TablesAndColumns,
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

fn is_select_item_in_relations(
    item: &sqlparser::ast::SelectItem,
    tables: &Vec<sqlparser::ast::TableWithJoins>,
    schema: &TablesAndColumns,
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

fn validate_and_extract_subqueries(
    query: &sqlparser::ast::Query,
    schema: &TablesAndColumns,
    schema_with_derived: &mut TablesAndColumns,
    errors: &mut Vec<String>,
) {
    if let Some(with) = &query.with {
        for derived in &with.cte_tables {
            if let Some(derived_from) = &derived.from {
                if !schema.contains_key(derived_from.value.as_str()) {
                    errors.push(format!(
                        "Table `{}` not found in schema nor subqueries",
                        derived_from.value
                    ));
                    continue;
                }
            }

            let mut new_errors = validate_query_with_schema(derived.query.as_ref(), schema);

            errors.append(&mut new_errors);

            let derived_name = &derived.alias.name.value;
            let mut derived_columns: Vec<String> = vec![];

            match derived.query.as_ref().body.as_ref() {
                SetExpr::Select(select_box) => {
                    let select = select_box.as_ref();
                    for item in &select.projection {
                        match &item {
                            sqlparser::ast::SelectItem::UnnamedExpr(expr) => {
                                match &expr {
                                    sqlparser::ast::Expr::Identifier(ident) => {
                                        derived_columns.push(ident.value.clone());
                                    }
                                    sqlparser::ast::Expr::CompoundIdentifier(idents) => {
                                        // ! is this correct?
                                        derived_columns.push(idents.last().unwrap().value.clone());
                                    }
                                    _ => {}
                                }
                            }
                            sqlparser::ast::SelectItem::ExprWithAlias { alias, .. } => {
                                derived_columns.push(alias.value.clone());
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            schema_with_derived.insert(derived_name.clone(), HashSet::from_iter(derived_columns));
        }
    }
}

fn validate_query_with_schema(
    query: &sqlparser::ast::Query,
    schema: &TablesAndColumns,
) -> Vec<String> {
    let mut schema_with_derived: TablesAndColumns = schema.clone();
    let mut errors: Vec<String> = vec![];

    validate_and_extract_subqueries(&query, &schema, &mut schema_with_derived, &mut errors);

    let tables_in_schema: HashSet<String> = HashSet::from_iter(schema_with_derived.keys().cloned());

    match query.body.as_ref() {
        SetExpr::Select(select_box) => {
            let select = select_box.as_ref();

            for item in &select.from {
                // TODO provide schema with "WITH" queries injected
                let relation_name = is_relation_in_schema(&item.relation, &tables_in_schema);

                if let Some(relation_name) = relation_name {
                    errors.push(format!(
                        "Table `{relation_name}` not found in schema nor subqueries"
                    ))
                }

                for join in &item.joins {
                    let relation_name = is_relation_in_schema(&join.relation, &tables_in_schema);
                    if let Some(relation_name) = relation_name {
                        errors.push(format!(
                            "Table `{relation_name}` not found in schema nor subqueries"
                        ))
                    }
                }
            }

            for item in &select.projection {
                let result = is_select_item_in_relations(item, &select.from, &schema_with_derived);

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
        }
        // TODO: inserts
        // SetExpr::Insert(insert_box) => {}
        _ => {}
    }
    return errors;
}

fn validate_statements_with_schema(
    query: &Vec<Statement>,
    schema: &TablesAndColumns,
) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    for statement in query {
        match statement {
            Statement::Query(query_box) => {
                errors.append(&mut validate_query_with_schema(
                    &query_box.as_ref(),
                    &schema,
                ));
            }
            _ => {}
        }
    }
    return errors;
}

fn find_queries_in_tree(
    node: &Node,
    code: &[u8],
    queries: &mut Vec<QueryInCode>,
    verbose: Option<u8>,
) {
    let mut cursor = node.walk();
    let dialect = GenericDialect {};

    for child in node.children(&mut cursor) {
        let mut child_cursor = child.walk();

        for component in child.children(&mut child_cursor) {
            if component.kind() != "assignment" {
                continue;
            }

            if component.child_count() > 3 {
                continue;
            }

            let identifier = component.child(0).unwrap();
            let equal = component.child(1).unwrap();
            let var = component.child(2).unwrap();

            let is_string_assignment =
                identifier.kind() == "identifier" && equal.kind() == "=" && var.kind() == "string";

            if !is_string_assignment {
                continue;
            }

            let mut content_as_string = String::new();

            let mut var_cursor = var.walk();
            for var_component in var.children(&mut var_cursor) {
                if var_component.kind() == "string_content" {
                    content_as_string.push_str(&String::from_utf8_lossy(
                        &code[var_component.start_byte()..var_component.end_byte()],
                    ));
                }
                if var_component.kind() == "interpolation" {
                    // replace any interpolation in fstring with 1 will always produce valid parsable sql
                    // when they are inplace of static values
                    content_as_string.push_str("1");
                }
            }

            // ! Duct tape
            if content_as_string.find("REPLACE").is_some() {
                continue;
            }
            // !

            let query_at = component.start_position();

            let statements = SqlParser::parse_sql(&dialect, &content_as_string);

            match statements {
                Ok(statements) => {
                    queries.push(QueryInCode {
                        line: query_at.row + 1,
                        statements,
                    });
                }
                Err(err) => {
                    if verbose.unwrap_or(0) > 0 {
                        eprintln!("{err} {content_as_string}");
                    }
                }
            }
        }

        find_queries_in_tree(&child, code, queries, None);
    }
}

fn find_queries(code: &[u8]) -> Vec<QueryInCode> {
    let mut parser = CodeParser::new();

    parser
        .set_language(tree_sitter_python::language())
        .expect("Error loading Python grammar");

    let parsed: Option<tree_sitter::Tree> = parser.parse(code, None);

    let mut queries: Vec<QueryInCode> = Vec::new();

    if let Some(tree) = parsed {
        find_queries_in_tree(&tree.root_node(), &code, &mut queries, None);
    }

    return queries;
}

fn validate_queries_in_code(code: &[u8], schema: &TablesAndColumns) -> Vec<SqlQueryError> {
    let queries: Vec<QueryInCode> = find_queries(&code);
    let mut errors: Vec<SqlQueryError> = Vec::new();
    for query in queries {
        let query_errors: Vec<String> = validate_statements_with_schema(&query.statements, &schema);
        for query_error in query_errors {
            errors.push(SqlQueryError {
                line: query.line,
                description: query_error,
            });
        }
    }
    return errors;
}

pub fn validate(dir: &PathBuf, schema_file_path: &PathBuf) -> Vec<SqlValidationError> {
    let python_file_pattern = Regex::new(r"\.py$").unwrap();
    let schema: String = fs::read_to_string(schema_file_path).expect("Could not read schema file");

    let dialect = GenericDialect {};
    let schema_ast = SqlParser::parse_sql(&dialect, &schema).expect("Could not parse schema file");

    let tables_and_columns: TablesAndColumns = schema_to_tables_and_columns(&schema_ast);

    let mut validation_errors: Vec<SqlValidationError> = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let file_path = entry.path();

        if !(file_path.is_file()
            && python_file_pattern
                .is_match(file_path.to_str().expect("Couldn't convert path to str")))
        {
            continue;
        }

        let code = fs::read(file_path)
            .map_err(|err| eprintln!("Could not open {:?} due to error: {err}", file_path));

        let Ok(code) = code else { continue };

        let query_errors = validate_queries_in_code(&code, &tables_and_columns);

        for query_error in query_errors {
            let validation_error =
                SqlValidationError::new(file_path, query_error.line, query_error.description);

            validation_errors.push(validation_error);
        }
    }

    return validation_errors;
}
