use std::sync::LazyLock;

use regex::Regex;

static INTERPOLATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{.*?\}").expect("static regex is valid"));

pub fn extract_query_string_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
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
            content_as_string.push('1');
        }
    }

    // For `.format()`-style strings tree-sitter doesn't yield interpolation
    // nodes, so we sweep `{...}` placeholders here. `.format()` uses `{{`
    // and `}}` as literal braces — pre-escape them with sentinel bytes so
    // the lazy regex doesn't eat the doubled-up form (which would leave a
    // stray `}` and break the SQL parse), then restore single braces after.
    const ESC_OPEN: char = '\u{0001}';
    const ESC_CLOSE: char = '\u{0002}';
    let escaped = content_as_string
        .replace("{{", &ESC_OPEN.to_string())
        .replace("}}", &ESC_CLOSE.to_string());
    let substituted = INTERPOLATION_RE.replace_all(&escaped, "1");
    let restored = substituted.replace(ESC_OPEN, "{").replace(ESC_CLOSE, "}");

    Some(restored)
}
