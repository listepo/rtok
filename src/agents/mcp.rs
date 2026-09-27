//! Shared MCP config-entry core (T275, the start of it — full table-driven core lands in
//! T277). `has_entry` is the read side every host's `installed()` uses to say whether its own
//! config file really carries rtok's MCP server, instead of a raw `contains("\"rtok\"")` on the
//! file text, which a `name` merely mentioned elsewhere (a comment, an unrelated value) would
//! wrongly count.

use std::path::Path;

use serde_json::Value;

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
}
