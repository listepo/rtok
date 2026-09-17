//! Zed installer (`rtok agents install zed`, plan T48.6).
//!
//! Zed reads MCP servers from `context_servers` in `~/.config/zed/settings.json`
//! (`[setup.zed] config_path`): `context_servers.rtok = {command, args}`. The settings file
//! is JSON with `//` and `/* */` comments, which `serde_json` rejects, so setup edits the text
//! surgically and only parses a comment-stripped copy to validate: comments and foreign
//! servers survive installs and removes. Zed has no shell hook events; the Zed agent reads
//! these servers directly, and external agents can reach them over ACP.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

/// Zed: MCP in the shared `settings.json`; the CLI and the desktop app read the same file.
pub struct Zed;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Zed CLI",
        bins: &["zed"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Zed",
        bins: &[],
        apps: &[
            "/Applications/Zed.app",
            "$LOCALAPPDATA/Programs/Zed/Zed.exe",
        ],
    },
];

impl Agent for Zed {
    fn id(&self) -> &'static str {
        "zed"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn shared(&self) -> bool {
        true
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "mcp" => Support::Yes,
            "hooks" => Support::No(
                "Zed has no shell hook events; its agent runs tools itself and takes external agents over ACP",
            ),
            "proxy" => Support::No(
                "Zed serves hosted models or provider API keys; there is no documented base-URL setting to point at the proxy",
            ),
            _ => Support::No(
                "Zed extensions install from the marketplace; there is no local directory to link",
            ),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.zed.config_path.clone()]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        if has_rtok(&super::read(&cfg.setup.zed.config_path)) {
            vec!["mcp"]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        if mode == Mode::Remove {
            Ok(vec![unregister_mcp(cfg)?])
        } else if cfg.setup.mcp {
            Ok(vec![register_mcp(cfg)?])
        } else {
            Ok(vec![rtok_agent_sdk::NO_CHANGES.into()])
        }
    }
}

/// The entry setup writes: the local-server shape from the Zed MCP docs.
fn want_entry() -> Value {
    json!({"command": super::rtok_command(), "args": ["mcp"]})
}

/// True when `context_servers.rtok` is an object in the document (comments allowed); a
/// document that does not parse at all falls back to the house `contains` check.
fn has_rtok(raw: &str) -> bool {
    if let Ok(root) = parse(raw) {
        return root
            .pointer("/context_servers/rtok")
            .is_some_and(Value::is_object);
    }
    raw.contains("\"rtok\"")
}

/// Add `context_servers.rtok`, or report no changes. Missing or blank files start as `{}`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.zed.config_path;
    let raw = read_opt(path)?;
    let (body, report) = insert_rtok(&raw, path, &want_entry())?;
    rtok_agent_sdk::write(&apply(cfg), path, &body, &report)?;
    Ok(report)
}

/// Drop `context_servers.rtok`, keeping every comment and foreign server. An object left with
/// no entries and no comments goes with it; a comment-only object stays.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.zed.config_path;
    let raw = read_opt(path)?;
    let (body, report) = remove_rtok(&raw, path)?;
    rtok_agent_sdk::write(&apply(cfg), path, &body, &report)?;
    Ok(report)
}

/// An absent file is `{}`; an unreadable one is an error (never overwrite a config that was
/// not read).
fn read_opt(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| path.display().to_string()),
    }
}

/// The document with comments stripped, parsed — or an error naming the file (a malformed
/// file is never overwritten).
fn parse(raw: &str) -> Result<Value> {
    serde_json::from_str(&strip_comments(raw)).map_err(|e| anyhow::anyhow!("{e}"))
}

fn parse_at(raw: &str, path: &Path) -> Result<Value> {
    parse(raw).with_context(|| path.display().to_string())
}

/// `raw` with `//…` and `/*…*/` outside strings removed (positions shift; parse only).
/// Shared with the remove e2e so it asserts on the same parse the installer uses.
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

fn insert_rtok(raw: &str, path: &Path, entry: &Value) -> Result<(String, String)> {
    use rtok_agent_sdk::NO_CHANGES;
    let report = format!("+ context_servers.{NAME}: {}", summary());
    let text = if strip_comments(raw).trim().is_empty() {
        String::from("{}")
    } else {
        let root = parse_at(raw, path)?;
        if !root.is_object() {
            return Ok((fresh_doc(entry), report));
        }
        raw.to_string()
    };
    let open = root_open(&text).with_context(|| path.display().to_string())?;
    let close = match_pair(&text, open, b'{', b'}').with_context(|| path.display().to_string())?;
    let brace = close - 1;
    match find_key(&text, open, "context_servers") {
        None => {
            let inner_empty = strip_comments(&text[open + 1..brace]).trim().is_empty();
            let sep = if inner_empty { "\n  " } else { ",\n  " };
            let mut body = text;
            body.replace_range(
                brace..close,
                &format!(
                    "{sep}\"context_servers\": {{\n    \"{NAME}\": {}\n  }}\n}}",
                    render_entry(entry, 4)
                ),
            );
            Ok((body, report))
        }
        Some((_, vs, ve)) => {
            let span: Value =
                serde_json::from_str(&strip_comments(&text[vs..ve])).unwrap_or(Value::Null);
            if !span.is_object() {
                let mut body = text;
                body.replace_range(
                    vs..ve,
                    &format!("{{\n    \"{NAME}\": {}\n  }}", render_entry(entry, 4)),
                );
                return Ok((body, report));
            }
            match find_key(&text, vs, NAME) {
                None => {
                    let inner_empty = strip_comments(&text[vs + 1..ve.saturating_sub(1)])
                        .trim()
                        .is_empty();
                    let last = last_value_end(&text, vs).unwrap_or(ve - 1);
                    let mut body = text;
                    if inner_empty {
                        body.insert_str(
                            ve - 1,
                            &format!("\n    \"{NAME}\": {}\n  ", render_entry(entry, 4)),
                        );
                    } else {
                        body.insert_str(
                            last,
                            &format!(",\n    \"{NAME}\": {}", render_entry(entry, 4)),
                        );
                    }
                    Ok((body, report))
                }
                Some((_, evs, eve)) => {
                    let have: Value = serde_json::from_str(&strip_comments(&text[evs..eve]))
                        .unwrap_or(Value::Null);
                    if have == *entry {
                        Ok((text, NO_CHANGES.into()))
                    } else {
                        let mut body = text;
                        body.replace_range(evs..eve, &render_entry(entry, 4));
                        Ok((body, format!("~ context_servers.{NAME}: {}", summary())))
                    }
                }
            }
        }
    }
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

fn remove_rtok(raw: &str, path: &Path) -> Result<(String, String)> {
    use rtok_agent_sdk::NO_CHANGES;
    if strip_comments(raw).trim().is_empty() {
        return Ok((raw.to_string(), NO_CHANGES.into()));
    }
    let root = parse_at(raw, path)?;
    if !root.is_object() {
        return Ok((raw.to_string(), NO_CHANGES.into()));
    }
    let open = root_open(raw).unwrap_or(0);
    let Some((_, vs, _)) = find_key(raw, open, "context_servers") else {
        return Ok((raw.to_string(), NO_CHANGES.into()));
    };
    let Some((rks, _, rve)) = find_key(raw, vs, NAME) else {
        return Ok((raw.to_string(), NO_CHANGES.into()));
    };
    let body = excise_member(raw, rks, rve);
    // An object left with no entries and no comments goes with its key; a comment-only
    // object stays, so user comments are never destroyed.
    let drop_key = match find_key(&body, root_open(&body).unwrap_or(0), "context_servers") {
        Some((_, cvs, cve)) => {
            let inner = &body[cvs + 1..cve.saturating_sub(1)];
            let stripped = strip_comments(inner);
            stripped.trim().is_empty() && stripped.len() == inner.len()
        }
        None => false,
    };
    let mut body = body;
    if drop_key {
        let (cks, _, cve) =
            find_key(&body, root_open(&body).unwrap_or(0), "context_servers").unwrap();
        body = excise_member(&body, cks, cve);
    }
    Ok((body, format!("- context_servers.{NAME}")))
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

/// A whole new document carrying only our entry.
fn fresh_doc(entry: &Value) -> String {
    serde_json::to_string_pretty(&json!({"context_servers": {NAME: entry}})).unwrap_or_default()
        + "\n"
}

fn summary() -> String {
    let cmd = super::rtok_command();
    format!("{cmd} mcp")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_agent_sdk::NO_CHANGES;
    use std::fs;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-zed-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut c = Config::default();
        c.setup.zed.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    fn entry() -> Value {
        json!({"command": "rtok", "args": ["mcp"]})
    }

    #[test]
    fn dry_run_names_the_change_and_creates_nothing() {
        let (c, path) = cfg("dry", true);
        let out = register_mcp(&c).unwrap();
        assert!(out.starts_with("+ context_servers.rtok: "), "{out}");
        assert!(out.ends_with(" mcp"), "{out}");
        assert!(!path.exists());
        assert!(Zed.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_into_a_missing_file_is_idempotent() {
        let (c, path) = cfg("missing", false);
        let first = register_mcp(&c).unwrap();
        assert!(first.starts_with("+ context_servers.rtok: "), "{first}");
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["context_servers"]["rtok"]["args"], json!(["mcp"]));
        assert_eq!(Zed.installed(&c, Kind::Desktop), ["mcp"]);
        assert_eq!(unregister_mcp(&c).unwrap(), "- context_servers.rtok");
        assert!(!fs::read_to_string(&path).unwrap().contains("rtok"));
        assert_eq!(unregister_mcp(&c).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// The `entry()` helper pins the command the tests run with: the suite never depends on
    /// whether `rtok` is on PATH.
    #[test]
    fn scanner_finds_keys_through_strings_comments_and_nesting() {
        let raw = "{\n  // \"context_servers\": fake\n  \"url\": \"https://x/{\\\"a\\\"}\",\n  /* multi\n  \"rtok\": 1 */\n  \"context_servers\": {\"other\": [1, {\"rtok\": 2}], \"rtok\": {\"command\": \"rtok\", \"args\": [\"mcp\"]}}\n}\n";
        assert_eq!(parse(raw).unwrap()["context_servers"]["rtok"], entry());
        let open = root_open(raw).unwrap();
        let (cks, cvs, _) = find_key(raw, open, "context_servers").unwrap();
        assert_eq!(&raw[cks..cks + 17], "\"context_servers\"");
        let (rks, rvs, rve) = find_key(raw, cvs, "rtok").unwrap();
        assert_eq!(&raw[rks..rks + 6], "\"rtok\"");
        assert_eq!(
            serde_json::from_str::<Value>(&raw[rvs..rve]).unwrap(),
            entry()
        );
        // Idempotent against our own pretty entry, comments and all.
        let (body, report) = insert_rtok(raw, Path::new("t"), &entry()).unwrap();
        assert_eq!(report, NO_CHANGES, "{body}");
    }

    #[test]
    fn apply_keeps_comments_and_foreign_servers() {
        let (c, path) = cfg("comments", false);
        fs::write(
            &path,
            "{\n  // my theme\n  \"theme\": \"One Dark\",\n  /* servers */\n  \"context_servers\": {\n    // foreign\n    \"other\": {\"command\": \"npx\", \"args\": [\"x\"]}\n  }\n}\n",
        )
        .unwrap();
        assert!(
            register_mcp(&c)
                .unwrap()
                .starts_with("+ context_servers.rtok: ")
        );
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("// my theme"), "{raw}");
        assert!(raw.contains("/* servers */"), "{raw}");
        assert!(raw.contains("// foreign"), "{raw}");
        assert!(raw.contains("\"other\""), "{raw}");
        let root: Value = serde_json::from_str(&strip_comments(&raw)).unwrap();
        assert_eq!(root["context_servers"]["rtok"]["args"], json!(["mcp"]));
        assert_eq!(root["context_servers"]["other"]["command"], "npx");
        assert_eq!(Zed.installed(&c, Kind::Cli), ["mcp"]);

        assert_eq!(unregister_mcp(&c).unwrap(), "- context_servers.rtok");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("\"rtok\""), "{raw}");
        assert!(
            raw.contains("// my theme") && raw.contains("// foreign"),
            "{raw}"
        );
        assert!(raw.contains("\"other\""), "{raw}");
        assert!(Zed.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn remove_leaves_a_comment_only_object_in_place() {
        let (c, path) = cfg("conly", false);
        fs::write(
            &path,
            "{\"context_servers\": {\n    // keep me\n    \"rtok\": {\"command\": \"rtok\", \"args\": [\"mcp\"]}\n  }}\n",
        )
        .unwrap();
        assert_eq!(unregister_mcp(&c).unwrap(), "- context_servers.rtok");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("// keep me"), "{raw}");
        assert!(raw.contains("\"context_servers\""), "{raw}");
        assert_eq!(unregister_mcp(&c).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn malformed_input_is_an_error_and_writes_nothing() {
        let (c, path) = cfg("bad", false);
        fs::write(&path, "{\"context_servers\": ").unwrap();
        assert!(register_mcp(&c).is_err());
        assert!(unregister_mcp(&c).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"context_servers\": ");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
