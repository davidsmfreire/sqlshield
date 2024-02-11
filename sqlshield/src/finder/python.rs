use regex::Regex;

pub fn extract_query_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }

    let mut content_as_string = String::new();

    let mut var_cursor = node.walk();
    for var_component in node.children(&mut var_cursor) {
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

    // If it isn't an fstring, but has interpolations to be formatted with .format later
    // tree_sitter will not store the interpolation node and the code above won't clean it
    let re = Regex::new(r"\{.*?\}").unwrap();
    content_as_string = re.replace_all(&content_as_string, "1").to_string();

    return Some(content_as_string);
}
