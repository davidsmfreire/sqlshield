//! Extract SQL string literals from JavaScript and TypeScript source.
//!
//! Both grammars expose the same string-literal shapes:
//!
//! * `string` — single- or double-quoted, with backslash escapes.
//! * `template_string` — backtick-quoted; may contain `${...}`
//!   `template_substitution` children.
//!
//! For template strings we follow the Python finder's pattern: each
//! `template_substitution` becomes a literal `1`, which keeps the SQL
//! parsable when substitutions stand in for static values.

pub fn extract_query_string_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    match node.kind() {
        // string_inner_text already decodes any escape_sequence children;
        // its output is the final SQL text.
        "string" => string_inner_text(node, code),
        "template_string" => Some(extract_template(node, code)),
        _ => None,
    }
}

/// Get the text between the quotes of a `string` node. Tree-sitter's JS
/// grammar emits `string_fragment` children for the raw content, so prefer
/// concatenating those when present; fall back to slicing between the outer
/// quote characters otherwise.
fn string_inner_text(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let mut content = String::new();
    let mut found_fragment = false;
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_fragment" => {
                found_fragment = true;
                content.push_str(&String::from_utf8_lossy(
                    &code[child.start_byte()..child.end_byte()],
                ));
            }
            "escape_sequence" => {
                found_fragment = true;
                let raw = &code[child.start_byte()..child.end_byte()];
                let text = std::str::from_utf8(raw).ok()?;
                // Decode the single escape inline so the caller's pass is a
                // no-op for whatever we produce here.
                content.push_str(&decode_js_escapes(text));
            }
            _ => {}
        }
    }
    if found_fragment {
        return Some(content);
    }
    // Empty string or older grammar shape: slice between the outer quotes.
    let raw = &code[node.start_byte()..node.end_byte()];
    let text = std::str::from_utf8(raw).ok()?;
    let bytes = text.as_bytes();
    let quote = bytes.first().copied()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let first = text.find(quote as char)?;
    let last = text.rfind(quote as char)?;
    if first >= last {
        return None;
    }
    Some(text[first + 1..last].to_string())
}

fn extract_template(node: &tree_sitter::Node, code: &[u8]) -> String {
    let mut cursor = node.walk();
    let mut out = String::new();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_fragment" => {
                out.push_str(&String::from_utf8_lossy(
                    &code[child.start_byte()..child.end_byte()],
                ));
            }
            "escape_sequence" => {
                let raw = &code[child.start_byte()..child.end_byte()];
                if let Ok(text) = std::str::from_utf8(raw) {
                    out.push_str(&decode_js_escapes(text));
                }
            }
            "template_substitution" => {
                // Replace ${...} with `1` — same trick as the Python finder.
                out.push('1');
            }
            _ => {}
        }
    }
    out
}

fn decode_js_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('`') => out.push('`'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            // `\b`, `\f`, `\v`, `\xNN`, `\uNNNN`, `\u{...}`, octal: keep
            // literal text. Half-decoding adds risk without payoff.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
