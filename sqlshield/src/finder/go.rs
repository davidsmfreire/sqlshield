//! Extract SQL string literals from Go source.
//!
//! Go has two string literal forms:
//!
//! * Interpreted strings (`"…"`) — backslash escapes apply (`\"`, `\n`, …).
//!   Tree-sitter labels these `interpreted_string_literal`.
//! * Raw strings (`` `…` ``) — no escape processing; backtick is the only
//!   forbidden character. Tree-sitter labels these `raw_string_literal`.
//!
//! Go has no built-in interpolation, but `fmt.Sprintf` and friends use
//! `%s` / `%d` / etc. placeholders embedded inside ordinary strings.
//! Replace those with `1` so the parsed SQL still tokenizes — same trick
//! used by the Python finder for `{}` placeholders.

use std::sync::LazyLock;

use regex::Regex;

/// Match the common `fmt`-style verbs (`%s`, `%d`, `%v`, `%q`, …) that
/// appear in `fmt.Sprintf("SELECT %s FROM …", col)`. `%%` is the literal-`%`
/// escape and is preserved.
static FORMAT_VERB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"%[+\-# 0]*\d*(?:\.\d+)?[vTtbcdoOqxXUeEfFgGspw]").expect("static regex is valid")
});

pub fn extract_query_string_from_node(node: &tree_sitter::Node, code: &[u8]) -> Option<String> {
    let decoded = match node.kind() {
        "interpreted_string_literal" => {
            let inner = inner_text(node, code, '"', '"')?;
            decode_go_escapes(inner)
        }
        "raw_string_literal" => inner_text(node, code, '`', '`')?.to_string(),
        _ => return None,
    };

    // Preserve `%%` as a sentinel so the verb pass doesn't see a stray `%`.
    const ESC_PCT: char = '\u{0001}';
    let escaped = decoded.replace("%%", &ESC_PCT.to_string());
    let substituted = FORMAT_VERB_RE.replace_all(&escaped, "1");
    Some(substituted.replace(ESC_PCT, "%"))
}

fn inner_text<'a>(
    node: &tree_sitter::Node,
    code: &'a [u8],
    open: char,
    close: char,
) -> Option<&'a str> {
    let raw = &code[node.start_byte()..node.end_byte()];
    let text = std::str::from_utf8(raw).ok()?;
    let first = text.find(open)?;
    let last = text.rfind(close)?;
    if first >= last {
        return None;
    }
    Some(&text[first + 1..last])
}

fn decode_go_escapes(s: &str) -> String {
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
            Some('a') | Some('b') | Some('f') | Some('v') | Some('0') => {
                // Drop these control escapes; keeping a literal byte is
                // pointless for SQL, and they never carry semantic meaning
                // inside a query string.
            }
            // `\xNN`, `\uNNNN`, `\UNNNNNNNN`, octal — keep the literal
            // text. Half-decoding adds risk without value for linting.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
