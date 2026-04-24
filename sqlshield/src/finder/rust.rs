use std::sync::LazyLock;

use regex::Regex;

// tree_sitter_rust doesn't emit an "interpolation" node (unlike tree_sitter_python),
// so we strip any `{...}` placeholders after the fact.
static INTERPOLATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{.*?\}").expect("static regex is valid"));

pub fn extract_query_string_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    if node.kind() != "string_literal" {
        return None;
    }

    let raw_string_content = &code[node.start_byte()..node.end_byte()];

    let string_content_without_commas =
        &String::from_utf8_lossy(raw_string_content).replace('"', "");

    let clean_string_content = INTERPOLATION_RE
        .replace_all(string_content_without_commas, "1")
        .to_string();

    Some(clean_string_content)
}
