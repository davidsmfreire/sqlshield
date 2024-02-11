use regex::Regex;

pub fn extract_query_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    if node.kind() != "string_literal" {
        return None;
    }

    // regex that matches {} and anything between them
    // unfortunately tree_sitter_rust doesn't store "interpolation" node
    // like tree_sitter_python does
    let re = Regex::new(r"\{.*?\}").unwrap();

    let raw_string_content = &code[node.start_byte()..node.end_byte()];

    let string_content_without_commas =
        &String::from_utf8_lossy(raw_string_content).replace("\"", "");

    let clean_string_content = re
        .replace_all(string_content_without_commas, "1")
        .to_string();

    return Some(clean_string_content);
}
