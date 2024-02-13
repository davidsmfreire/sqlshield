
// string ""
// template_string ``
// string_fragment (characters)
// TODO template_substitution ${id}

pub fn extract_query_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    if node.kind() != "string" && node.kind() != "template_string" {
        return None;
    }

    // println!("{}", node.kind());

    let mut content_as_string = String::new();

    let mut var_cursor = node.walk();

    for var_component in node.children(&mut var_cursor) {
        if var_component.kind() == "string_fragment" {
            content_as_string.push_str(&String::from_utf8_lossy(
                &code[var_component.start_byte()..var_component.end_byte()],
            ));
        }
    }

    println!("{}", content_as_string);

    return Some(content_as_string);
}
