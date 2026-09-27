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
