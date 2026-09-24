//! Shared JSONC-safe surgical editor (T79, T117): add, replace, or remove one string-keyed
//! member of a top-level object in a settings file that must keep its comments and trailing
//! commas intact. Zed's `context_servers.<name>` and VS Code's `chat.pluginLocations.<path>`
//! are the same shape — one nested object, one member keyed by name — so both installers go
//! through this instead of each carrying its own text scanner (no duplicated logic).
//!
//! The editor works on byte spans of the original text; a JSONC copy (`jsonc-parser`) is
//! parsed only to validate and to compare values, never written back (T79).

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

/// Parse JSONC (comments, trailing commas) into a `Value`. Validation/lookup only — the
/// surgical editor below works on spans of the original text, never this parsed copy.
pub fn parse(raw: &str) -> Result<Value> {
    let opts = jsonc_parser::ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
    };
    jsonc_parser::parse_to_serde_value::<Value>(raw, &opts).map_err(|e| anyhow::anyhow!("{e}"))
}

fn parse_at(raw: &str, path: &Path) -> Result<Value> {
    parse(raw).with_context(|| path.display().to_string())
}

/// An absent file reads as empty (callers start fresh); any other read error is fatal —
/// never overwrite a config that could not be read.
pub fn read_or_empty(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| path.display().to_string()),
    }
}

/// `raw` with `//…` and `/*…*/` outside strings removed (positions shift; parse only).
pub fn strip_comments(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        match scan_string(bytes, i) {
            Some(end) => {
                out.push_str(&raw[i..end]);
                i = end;
            }
            None => {
                if raw[i..].starts_with("//") {
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                } else if raw[i..].starts_with("/*") {
                    while i < bytes.len() && !raw[i..].starts_with("*/") {
                        i += 1;
                    }
                    i = (i + 2).min(bytes.len());
                } else {
                    let ch = raw[i..].chars().next().unwrap_or('\0');
                    out.push(ch);
                    i += ch.len_utf8();
                }
            }
        }
    }
    out
}

/// End (exclusive) of the `"`-string starting at `i`, or `None` when `i` is not a quote.
fn scan_string(bytes: &[u8], i: usize) -> Option<usize> {
    if bytes.get(i) != Some(&b'"') {
        return None;
    }
    let mut j = i + 1;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' => j += 2,
            b'"' => return Some(j + 1),
            _ => j += 1,
        }
    }
    None
}

/// Skip whitespace and comments from `i`.
fn skip_trivia(text: &str, mut i: usize) -> usize {
    let bytes = text.as_bytes();
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if text[i..].starts_with("//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if text[i..].starts_with("/*") {
            while i < bytes.len() && !text[i..].starts_with("*/") {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            return i;
        }
    }
}

/// End (exclusive) of the JSON value starting at `i` (after trivia), or `None`.
fn skip_value(text: &str, i: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let i = skip_trivia(text, i);
    match bytes.get(i)? {
        b'{' => match_pair(text, i, b'{', b'}'),
        b'[' => match_pair(text, i, b'[', b']'),
        b'"' => scan_string(bytes, i),
        _ => {
            let mut j = i;
            while j < bytes.len() && !matches!(bytes[j], b',' | b'}' | b']') {
                j += 1;
            }
            // A literal must end before a delimiter, not at end of input.
            (j > i && j < bytes.len()).then_some(j)
        }
    }
}

/// End (exclusive) of the bracketed value opening at `i`, or `None` when unbalanced.
fn match_pair(text: &str, i: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    let mut j = i;
    while j < bytes.len() {
        if let Some(end) = scan_string(bytes, j) {
            j = end;
            continue;
        }
        if text[j..].starts_with("//") || text[j..].starts_with("/*") {
            j = skip_trivia(text, j);
            continue;
        }
        match bytes[j] {
            b if b == open => depth += 1,
            b if b == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(j + 1);
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// Decoded key of the string spanning `start..end` (exclusive end, quotes included).
fn key_name(text: &str, start: usize, end: usize) -> Option<String> {
    serde_json::from_str(&text[start..end]).ok()
}

/// `(key_start, value_start, value_end)` of `key` in the object opening at `open`.
fn find_key(text: &str, open: usize, key: &str) -> Option<(usize, usize, usize)> {
    let mut i = skip_trivia(text, open + 1);
    loop {
        if text.as_bytes().get(i) == Some(&b'}') {
            return None;
        }
        let ks = i;
        let ke = scan_string(text.as_bytes(), i)?;
        let name = key_name(text, ks, ke)?;
        i = skip_trivia(text, ke);
        if text.as_bytes().get(i) != Some(&b':') {
            return None;
        }
        let vs = skip_trivia(text, i + 1);
        let ve = skip_value(text, vs)?;
        if name == key {
            return Some((ks, vs, ve));
        }
        i = skip_trivia(text, ve);
        if text.as_bytes().get(i) == Some(&b',') {
            i = skip_trivia(text, i + 1);
        } else {
            return None;
        }
    }
}

/// Byte offset of the root object's `{`, or `None`.
fn root_open(text: &str) -> Option<usize> {
    let i = skip_trivia(text, 0);
    (text.as_bytes().get(i) == Some(&b'{')).then_some(i)
}

/// End of the last member value in the object opening at `open`.
fn last_value_end(text: &str, open: usize) -> Option<usize> {
    let mut i = skip_trivia(text, open + 1);
    let mut last = None;
    loop {
        if text.as_bytes().get(i) == Some(&b'}') {
            return last;
        }
        let ke = scan_string(text.as_bytes(), i)?;
        i = skip_trivia(text, ke);
        if text.as_bytes().get(i) != Some(&b':') {
            return last;
        }
        let ve = skip_value(text, skip_trivia(text, i + 1))?;
        last = Some(ve);
        i = skip_trivia(text, ve);
        if text.as_bytes().get(i) == Some(&b',') {
            i = skip_trivia(text, i + 1);
        } else {
            return last;
        }
    }
}

/// Remove the member spanning `key_start..value_end` plus one adjacent comma.
fn excise_member(text: &str, key_start: usize, value_end: usize) -> String {
    let bytes = text.as_bytes();
    // Prefer the preceding comma, so the survivors keep their separators.
    let mut back = key_start;
    while back > 0 && bytes[back - 1].is_ascii_whitespace() {
        back -= 1;
    }
    if back > 0 && bytes[back - 1] == b',' {
        return format!("{}{}", &text[..back - 1], &text[value_end..]);
    }
    let mut fwd = value_end;
    while fwd < bytes.len() && bytes[fwd].is_ascii_whitespace() {
        fwd += 1;
    }
    if bytes.get(fwd) == Some(&b',') {
        return format!("{}{}", &text[..key_start], &text[fwd + 1..]);
    }
    format!("{}{}", &text[..key_start], &text[value_end..])
}

/// Render `entry` indented by `pad` spaces per level below its key line.
fn render_entry(entry: &Value, pad: usize) -> String {
    serde_json::to_string_pretty(entry)
        .unwrap_or_default()
        .lines()
        .enumerate()
        .map(|(n, l)| {
            if n == 0 {
                l.to_string()
            } else {
                format!("{}{l}", " ".repeat(pad))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A JSON string literal for `s`, properly escaped — a plugin path may hold backslashes
/// (Windows) or other characters that a naive `"{s}"` would write out broken.
fn qkey(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

/// A whole new document carrying only `top_key.entry_key`.
fn fresh_doc(top_key: &str, entry_key: &str, entry: &Value) -> String {
    let mut inner = serde_json::Map::new();
    inner.insert(entry_key.to_string(), entry.clone());
    let mut root = serde_json::Map::new();
    root.insert(top_key.to_string(), Value::Object(inner));
    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default() + "\n"
}

/// What [`upsert_member`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upsert {
    NoChange,
    Added,
    Replaced,
}

/// Add or replace `raw[top_key][entry_key] = entry`, preserving every comment, trailing
/// comma and foreign member. A missing or blank file starts as `{}`; a root that parses but
/// is not an object (or whose `top_key` value is not an object) is replaced outright — there
/// is nowhere to add a member — never silently dropped otherwise.
pub fn upsert_member(
    raw: &str,
    path: &Path,
    top_key: &str,
    entry_key: &str,
    entry: &Value,
) -> Result<(String, Upsert)> {
    let text = if strip_comments(raw).trim().is_empty() {
        String::from("{}")
    } else {
        let root = parse_at(raw, path)?;
        if !root.is_object() {
            return Ok((fresh_doc(top_key, entry_key, entry), Upsert::Added));
        }
        raw.to_string()
    };
    let open = root_open(&text).with_context(|| path.display().to_string())?;
    let close = match_pair(&text, open, b'{', b'}').with_context(|| path.display().to_string())?;
    let brace = close - 1;
    match find_key(&text, open, top_key) {
        None => {
            let inner = strip_comments(&text[open + 1..brace]);
            let trimmed = inner.trim();
            // One separator, never two: a root that already ends in a trailing comma
            // keeps it and gains no second one (T79).
            let sep = if trimmed.is_empty() || trimmed.ends_with(',') {
                "\n  "
            } else {
                ",\n  "
            };
            let mut body = text;
            body.replace_range(
                brace..close,
                &format!(
                    "{sep}{}: {{\n    {}: {}\n  }}\n}}",
                    qkey(top_key),
                    qkey(entry_key),
                    render_entry(entry, 4)
                ),
            );
            Ok((body, Upsert::Added))
        }
        Some((_, vs, ve)) => {
            let span: Value = parse(&text[vs..ve]).unwrap_or(Value::Null);
            if !span.is_object() {
                let mut body = text;
                body.replace_range(
                    vs..ve,
                    &format!(
                        "{{\n    {}: {}\n  }}",
                        qkey(entry_key),
                        render_entry(entry, 4)
                    ),
                );
                return Ok((body, Upsert::Added));
            }
            match find_key(&text, vs, entry_key) {
                None => {
                    let inner_empty = strip_comments(&text[vs + 1..ve.saturating_sub(1)])
                        .trim()
                        .is_empty();
                    let last = last_value_end(&text, vs).unwrap_or(ve - 1);
                    let mut body = text;
                    if inner_empty {
                        body.insert_str(
                            ve - 1,
                            &format!("\n    {}: {}\n  ", qkey(entry_key), render_entry(entry, 4)),
                        );
                    } else {
                        body.insert_str(
                            last,
                            &format!(",\n    {}: {}", qkey(entry_key), render_entry(entry, 4)),
                        );
                    }
                    Ok((body, Upsert::Added))
                }
                Some((_, evs, eve)) => {
                    let have: Value = parse(&text[evs..eve]).unwrap_or(Value::Null);
                    if have == *entry {
                        Ok((text, Upsert::NoChange))
                    } else {
                        let mut body = text;
                        body.replace_range(evs..eve, &render_entry(entry, 4));
                        Ok((body, Upsert::Replaced))
                    }
                }
            }
        }
    }
}

/// Drop `raw[top_key][entry_key]`, keeping every comment and foreign member. An object left
/// with no entries and no comments goes with its key; a comment-only object stays, so user
/// comments are never destroyed. `true` when a member was actually removed.
pub fn remove_member(
    raw: &str,
    path: &Path,
    top_key: &str,
    entry_key: &str,
) -> Result<(String, bool)> {
    if strip_comments(raw).trim().is_empty() {
        return Ok((raw.to_string(), false));
    }
    let root = parse_at(raw, path)?;
    if !root.is_object() {
        return Ok((raw.to_string(), false));
    }
    let open = root_open(raw).unwrap_or(0);
    let Some((_, vs, _)) = find_key(raw, open, top_key) else {
        return Ok((raw.to_string(), false));
    };
    let Some((rks, _, rve)) = find_key(raw, vs, entry_key) else {
        return Ok((raw.to_string(), false));
    };
    let body = excise_member(raw, rks, rve);
    let drop_key = match find_key(&body, root_open(&body).unwrap_or(0), top_key) {
        Some((_, cvs, cve)) => {
            let inner = &body[cvs + 1..cve.saturating_sub(1)];
            let stripped = strip_comments(inner);
            stripped.trim().is_empty() && stripped.len() == inner.len()
        }
        None => false,
    };
    let mut body = body;
    if drop_key {
        let (cks, _, cve) = find_key(&body, root_open(&body).unwrap_or(0), top_key).unwrap();
        body = excise_member(&body, cks, cve);
    }
    Ok((body, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> Value {
        json!({"command": "rtok", "args": ["mcp"]})
    }

    /// White-box: the scanner finds keys through strings, comments and nesting, and an
    /// insert against our own pretty-printed entry (comments and all) is idempotent.
    #[test]
    fn scanner_finds_keys_through_strings_comments_and_nesting() {
        let raw = "{\n  // \"context_servers\": fake\n  \"url\": \"https://x/{\\\"a\\\"}\",\n  /* multi\n  \"rtok\": 1 */\n  \"context_servers\": {\"other\": [1, {\"rtok\": 2}], \"rtok\": {\"command\": \"rtok\", \"args\": [\"mcp\"]}}\n}\n";
        assert_eq!(parse(raw).unwrap()["context_servers"]["rtok"], entry());
        let (body, edit) =
            upsert_member(raw, Path::new("t"), "context_servers", "rtok", &entry()).unwrap();
        assert_eq!(edit, Upsert::NoChange, "{body}");
    }

    #[test]
    fn adds_replaces_and_is_idempotent_on_a_missing_top_key() {
        let (body, edit) = upsert_member(
            "",
            Path::new("t"),
            "chat.pluginLocations",
            "/p",
            &json!(true),
        )
        .unwrap();
        assert_eq!(edit, Upsert::Added);
        let root = parse(&body).unwrap();
        assert_eq!(root["chat.pluginLocations"]["/p"], json!(true));

        let (body2, edit2) = upsert_member(
            &body,
            Path::new("t"),
            "chat.pluginLocations",
            "/p",
            &json!(true),
        )
        .unwrap();
        assert_eq!(edit2, Upsert::NoChange);

        let (body3, edit3) = upsert_member(
            &body2,
            Path::new("t"),
            "chat.pluginLocations",
            "/p",
            &json!(false),
        )
        .unwrap();
        assert_eq!(edit3, Upsert::Replaced);
        assert_eq!(
            parse(&body3).unwrap()["chat.pluginLocations"]["/p"],
            json!(false)
        );
    }

    /// Keys that need JSON escaping (a Windows-shaped path) must round-trip, not corrupt
    /// the document (real risk once the entry key is a filesystem path, not a fixed name).
    #[test]
    fn entry_keys_needing_escapes_round_trip() {
        let key = r#"C:\Users\a "quoted"\plugins\rtok"#;
        let (body, edit) = upsert_member(
            "{}",
            Path::new("t"),
            "chat.pluginLocations",
            key,
            &json!(true),
        )
        .unwrap();
        assert_eq!(edit, Upsert::Added);
        let root = parse(&body).unwrap();
        assert_eq!(root["chat.pluginLocations"][key], json!(true));
        let (body2, removed) =
            remove_member(&body, Path::new("t"), "chat.pluginLocations", key).unwrap();
        assert!(removed);
        assert!(
            parse(&body2).unwrap()["chat.pluginLocations"]
                .get(key)
                .is_none()
        );
    }

    #[test]
    fn remove_keeps_foreign_members_and_comments() {
        let raw = "{\n  // mine\n  \"chat.pluginLocations\": {\n    // foreign\n    \"/other\": true,\n    \"/p\": true\n  }\n}\n";
        let (body, removed) =
            remove_member(raw, Path::new("t"), "chat.pluginLocations", "/p").unwrap();
        assert!(removed);
        assert!(body.contains("// mine"));
        assert!(body.contains("// foreign"));
        assert!(body.contains("\"/other\""));
        assert!(!body.contains("\"/p\""));
        let root = parse(&body).unwrap();
        assert_eq!(root["chat.pluginLocations"]["/other"], json!(true));
    }

    #[test]
    fn remove_on_missing_entry_is_a_no_op() {
        let (body, removed) =
            remove_member("{}", Path::new("t"), "chat.pluginLocations", "/p").unwrap();
        assert!(!removed);
        assert_eq!(body, "{}");
    }
}
