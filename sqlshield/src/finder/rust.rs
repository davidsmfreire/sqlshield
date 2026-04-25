use std::sync::LazyLock;

use regex::Regex;

// tree_sitter_rust doesn't emit an "interpolation" node (unlike tree_sitter_python),
// so we strip any `{...}` placeholders after the fact.
static INTERPOLATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{.*?\}").expect("static regex is valid"));

pub fn extract_query_string_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    let decoded = match node.kind() {
        // Regular `"…"`, byte `b"…"`, and C `c"…"` string literals: strip the
        // outer quotes and decode common backslash escapes so `\"` survives
        // as a literal quote (quoted SQL identifier).
        "string_literal" | "byte_string" | "c_string" => {
            let inner = inner_text(node, code)?;
            decode_rust_escapes(inner)
        }
        // `r"…"` and `r#"…"#` — no escape decoding; `"` can appear literally
        // when protected by one or more `#` pairs.
        "raw_string_literal" => inner_text(node, code)?.to_string(),
        _ => return None,
    };

    Some(INTERPOLATION_RE.replace_all(&decoded, "1").to_string())
}

/// Return the source text between the opening and closing `"` of a Rust
/// string literal. Works for regular, byte, C, and raw strings — the first
/// `"` skips past `b`/`c`/`r#…`, and the last `"` strips any trailing `#…`.
fn inner_text<'a>(node: &tree_sitter::Node, code: &'a [u8]) -> Option<&'a str> {
    let raw = &code[node.start_byte()..node.end_byte()];
    let text = std::str::from_utf8(raw).ok()?;
    let first = text.find('"')?;
    let last = text.rfind('"')?;
    if first >= last {
        return None;
    }
    Some(&text[first + 1..last])
}

fn decode_rust_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            // `\xNN`, `\u{…}` and friends — keep the literal text rather
            // than half-decoding. Harmless for SQL linting.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
