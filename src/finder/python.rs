pub fn find_queries_in_code(code: &[u8]) -> Result<Vec<super::QueryInCode>, String> {
    let mut parser = tree_sitter::Parser::new();

    parser
        .set_language(tree_sitter_python::language())
        .expect("Error loading Python grammar");

    let parsed: Option<tree_sitter::Tree> = parser.parse(code, None);

    let mut queries: Vec<super::QueryInCode> = Vec::new();

    if let Some(tree) = parsed {
        find_queries_in_tree(&tree.root_node(), &code, &mut queries, None);
        return Ok(queries);
    } else {
        return Err("Could not parse code".to_string());
    }
}

fn find_queries_in_tree(
    node: &tree_sitter::Node,
    code: &[u8],
    queries: &mut Vec<super::QueryInCode>,
    verbose: Option<u8>,
) {
    let mut cursor = node.walk();
    let dialect = sqlparser::dialect::GenericDialect {};

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

            let statements = sqlparser::parser::Parser::parse_sql(&dialect, &content_as_string);

            match statements {
                Ok(statements) => {
                    queries.push(super::QueryInCode {
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
