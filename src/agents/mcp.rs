//! Shared MCP config-entry core (T275, the start of it — full table-driven core lands in
//! T277). `has_entry` is the read side every host's `installed()` uses to say whether its own
//! config file really carries rtok's MCP server, instead of a raw `contains("\"rtok\"")` on the
//! file text, which a `name` merely mentioned elsewhere (a comment, an unrelated value) would
//! wrongly count.

use std::path::Path;

use serde_json::Value;
use toml_edit::{DocumentMut, Item};

/// Whether `<key>.<name>` exists as a JSON object in an already-parsed `value` — never a
/// substring match. A `key` that is not an object, or a `name` that exists but is not itself an
/// object (or is mentioned only elsewhere) reads as `false` — fail open, never a panic. Shared
/// by `has_entry` (reads a config file off disk) and doctor.rs's own missing-entry check, so the
/// lookup lives in one place.
pub(crate) fn entry_in(value: &Value, key: &str, name: &str) -> bool {
    value
        .get(key)
        .and_then(|servers| servers.get(name))
        .is_some_and(Value::is_object)
}

/// Whether `<key>.<name>` exists as a JSON object in the JSONC file at `path` — never a
/// substring match on the raw text. An absent, unreadable, or unparsable file, a `key` that is
/// not an object, or a `name` that exists but is not itself an object (or is mentioned only
/// elsewhere in the file) all read as `false` — fail open, never a panic.
pub(crate) fn has_entry(path: &Path, key: &str, name: &str) -> bool {
    let Ok(value) = super::jsonc::parse(&super::read(path)) else {
        return false;
    };
    entry_in(&value, key, name)
}

/// [`has_entry`]'s TOML equivalent, for Codex's and Grok's `~/.codex/config.toml`-shaped
/// config: whether `[<table>.<name>]` exists as a table in the TOML document at `path` —
/// never a substring match on the raw text. An absent, unreadable, or unparsable file, a
/// `table` that is not itself a table, or a `name` that exists but is not a table (or is
/// mentioned only elsewhere) all read as `false` — fail open, never a panic.
pub(crate) fn has_toml_entry(path: &Path, table: &str, name: &str) -> bool {
    let Ok(doc) = super::read(path).parse::<DocumentMut>() else {
        return false;
    };
    doc.get(table)
        .and_then(Item::as_table)
        .and_then(|t| t.get(name))
        .is_some_and(Item::is_table)
}

/// `item` as a [`Value`], so a TOML entry (Codex's, Grok's `[mcp_servers.rtok]`) can run
/// through [`rtok_agent_sdk::judge_owned`] the same way the JSON hosts' entries do (T246.5).
/// Covers the shapes an `[mcp_servers.rtok]` table can hold.
pub(crate) fn toml_item_to_json(item: &Item) -> Value {
    match item {
        Item::None => Value::Null,
        Item::Value(v) => toml_value_to_json(v),
        Item::Table(t) => t
            .iter()
            .map(|(k, v)| (k.to_string(), toml_item_to_json(v)))
            .collect(),
        Item::ArrayOfTables(_) => Value::Null,
    }
}

fn toml_value_to_json(v: &toml_edit::Value) -> Value {
    match v {
        toml_edit::Value::String(s) => Value::String(s.value().clone()),
        toml_edit::Value::Integer(i) => Value::Number((*i.value()).into()),
        toml_edit::Value::Float(f) => {
            serde_json::Number::from_f64(*f.value()).map_or(Value::Null, Value::Number)
        }
        toml_edit::Value::Boolean(b) => Value::Bool(*b.value()),
        toml_edit::Value::Datetime(d) => Value::String(d.value().to_string()),
        toml_edit::Value::Array(a) => a.iter().map(toml_value_to_json).collect(),
        toml_edit::Value::InlineTable(t) => t
            .iter()
            .map(|(k, v)| (k.to_string(), toml_value_to_json(v)))
            .collect(),
    }
}

/// T275/D33 checks (a)-(e) for one host, shared so every host runs the same sequence: with
/// the host's plugin already in place, (d) no entry reads as no `mcp`; (a) install writes it;
/// (b) update keeps it; (e) remove leaves an entry the user edited (`edit` adds `--extra` to
/// it in `file`) with a `leave <label>` line; (c) once `reseed` writes rtok's own entry again,
/// remove takes it out.
#[cfg(test)]
pub(crate) fn assert_entry_lifecycle(
    agent: &dyn super::Agent,
    cfg: &crate::config::Config,
    kind: super::Kind,
    label: &str,
    file: &Path,
    edit: impl Fn(),
    reseed: impl Fn(),
) {
    use super::Mode;
    let has = || agent.installed(cfg, kind).contains(&"mcp");
    assert!(!has(), "(d) no {label} yet");
    agent.apply(cfg, kind, Mode::Install).unwrap();
    assert!(has(), "(a) install writes {label}");
    let lines = agent.apply(cfg, kind, Mode::Update).unwrap();
    let removed = format!("- {label}");
    assert!(!lines.iter().any(|l| l.contains(&removed)), "(b) {lines:?}");
    assert!(has(), "(b) update keeps {label}");
    edit();
    let lines = agent.apply(cfg, kind, Mode::Remove).unwrap();
    let leave = format!("leave {label}");
    assert!(lines.iter().any(|l| l.starts_with(&leave)), "(e) {lines:?}");
    assert!(
        super::read(file).contains("--extra"),
        "(e) edited entry survives"
    );
    reseed();
    assert!(has(), "(c) reseeded {label}");
    agent.apply(cfg, kind, Mode::Remove).unwrap();
    assert!(!has(), "(c) remove takes {label} out");
}

/// [`assert_entry_lifecycle`] for a host whose entry is `rtok` under the dotted JSON `key` in
/// `file`; `reseed` writes rtok's own entry again.
#[cfg(test)]
pub(crate) fn assert_json_entry_lifecycle(
    agent: &dyn super::Agent,
    cfg: &crate::config::Config,
    kind: super::Kind,
    file: &Path,
    key: &str,
    reseed: impl Fn() -> anyhow::Result<String>,
) {
    let label = format!("{key}.rtok");
    let edit = || edit_json_args(file, key);
    assert_entry_lifecycle(agent, cfg, kind, &label, file, edit, || {
        reseed().unwrap();
    });
}

/// The user edit [`assert_entry_lifecycle`] expects, for a JSON config: `--extra` appended to
/// the `rtok` entry's `args` under the dotted `key`.
#[cfg(test)]
pub(crate) fn edit_json_args(file: &Path, key: &str) {
    let mut doc: Value = serde_json::from_str(&super::read(file)).unwrap();
    let entry = key.split('.').fold(&mut doc, |v, k| &mut v[k]);
    entry["rtok"]["args"] = serde_json::json!(["mcp", "--extra"]);
    std::fs::write(file, doc.to_string()).unwrap();
}

/// [`assert_entry_lifecycle`] for a host whose entry is `rtok` under the TOML `table` in
/// `file` (Codex's, Grok's `[mcp_servers.rtok]`); `reseed` writes rtok's own entry again.
#[cfg(test)]
pub(crate) fn assert_toml_entry_lifecycle(
    agent: &dyn super::Agent,
    cfg: &crate::config::Config,
    kind: super::Kind,
    file: &Path,
    table: &str,
    reseed: impl Fn() -> anyhow::Result<String>,
) {
    let label = format!("{table}.rtok");
    let edit = || edit_toml_args(file, table);
    assert_entry_lifecycle(agent, cfg, kind, &label, file, edit, || {
        reseed().unwrap();
    });
}

/// The user edit [`assert_entry_lifecycle`] expects, for a TOML config: `--extra` appended to
/// the `rtok` entry's `args` under the dotted `table`.
#[cfg(test)]
pub(crate) fn edit_toml_args(file: &Path, table: &str) {
    let mut doc: DocumentMut = super::read(file).parse().unwrap();
    let mut parts = table.split('.');
    let mut entry = &mut doc[parts.next().unwrap()];
    for part in parts {
        entry = &mut entry[part];
    }
    entry["rtok"]["args"] = toml_edit::value(toml_edit::Array::from_iter(["mcp", "--extra"]));
    std::fs::write(file, doc.to_string()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_file(name: &str, contents: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rtok-mcp-has-entry-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn present_entry_is_true() {
        let path = tmp_file(
            "present",
            r#"{"mcpServers": {"rtok": {"command": "rtok", "args": ["mcp"]}}}"#,
        );
        assert!(has_entry(&path, "mcpServers", "rtok"));
    }

    #[test]
    fn missing_key_name_or_file_is_false() {
        let path = tmp_file("missing", r#"{"mcpServers": {"other": {"command": "x"}}}"#);
        assert!(!has_entry(&path, "mcpServers", "rtok"));
        assert!(!has_entry(&path, "otherKey", "rtok"));
        assert!(!has_entry(
            Path::new("/does/not/exist/rtok-t275.json"),
            "mcpServers",
            "rtok"
        ));
    }

    /// `"rtok"` shows up in a comment and as an unrelated value, never as `mcpServers.rtok`
    /// itself — a substring match on the raw text would wrongly say yes (the bug this
    /// function replaces).
    #[test]
    fn name_mentioned_elsewhere_in_the_file_is_not_an_entry() {
        let path = tmp_file(
            "elsewhere",
            r#"{
                // rtok
                "notes": "rtok lives here too",
                "mcpServers": {"other": {"command": "rtok"}}
            }"#,
        );
        assert!(!has_entry(&path, "mcpServers", "rtok"));
    }

    #[test]
    fn invalid_file_is_false() {
        let path = tmp_file("invalid", "{ this is not json");
        assert!(!has_entry(&path, "mcpServers", "rtok"));
    }

    fn tmp_toml(name: &str, contents: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rtok-mcp-has-toml-entry-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn toml_present_entry_is_true() {
        let path = tmp_toml(
            "present",
            "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n",
        );
        assert!(has_toml_entry(&path, "mcp_servers", "rtok"));
    }

    #[test]
    fn toml_missing_table_name_or_file_is_false() {
        let path = tmp_toml("missing", "[mcp_servers.other]\ncommand = \"x\"\n");
        assert!(!has_toml_entry(&path, "mcp_servers", "rtok"));
        assert!(!has_toml_entry(&path, "other_table", "rtok"));
        assert!(!has_toml_entry(
            Path::new("/does/not/exist/rtok-t275.toml"),
            "mcp_servers",
            "rtok"
        ));
    }

    /// `"rtok"` shows up as an unrelated string value, never as `[mcp_servers.rtok]` itself.
    #[test]
    fn toml_name_mentioned_elsewhere_is_not_an_entry() {
        let path = tmp_toml(
            "elsewhere",
            "notes = \"rtok lives here too\"\n\n[mcp_servers.other]\ncommand = \"rtok\"\n",
        );
        assert!(!has_toml_entry(&path, "mcp_servers", "rtok"));
    }

    #[test]
    fn toml_invalid_file_is_false() {
        let path = tmp_toml("invalid", "this is not = [valid toml");
        assert!(!has_toml_entry(&path, "mcp_servers", "rtok"));
    }

    #[test]
    fn toml_item_to_json_converts_a_server_table() {
        let doc: DocumentMut = "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n"
            .parse()
            .unwrap();
        let item = doc["mcp_servers"]["rtok"].clone();
        assert_eq!(
            toml_item_to_json(&item),
            serde_json::json!({"command": "rtok", "args": ["mcp"]})
        );
    }
}
