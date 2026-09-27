//! T279: `plugins/<host>/.rtok-plugin-version` and every plugin manifest's `version` field must
//! equal the running rtok version, so a host that caches a plugin by its manifest version (Claude
//! Code) sees a new build as a new version. `tools/plugin-versions.sh --set` writes both from
//! `Cargo.toml` in the same commit as the version bump (`tools/release.sh`); this test is the
//! local half of the guarantee, alongside `tools/plugin-versions.sh --check` in `ci.yml` and
//! `release.yml` (docs/plugin-versions.md has the full scheme).

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every plugin tree that carries `plugins/<host>/.rtok-plugin-version` (T279 step 1). Not
/// every directory under `plugins/` is here: `cline` has no manifest of its own (hooks only)
/// and `antigravity`'s `plugin.json` carries no version field for its host to cache by.
/// Mirrors `tools/plugin-versions.sh`'s `VERSION_HOSTS` list.
const VERSION_FILE_HOSTS: &[&str] = &[
    "claude", "codex", "copilot", "cursor", "devin", "gemini", "grok", "kimi", "opencode", "pi",
    "zcode",
];

/// Every manifest with a top-level `"version"` field, raised in step with the hosts above.
/// Mirrors `tools/plugin-versions.sh`'s `MANIFEST_FILES` list.
const MANIFEST_FILES: &[&str] = &[
    "plugins/claude/.claude-plugin/plugin.json",
    "plugins/codex/.codex-plugin/plugin.json",
    "plugins/cursor/plugin.json",
    "plugins/cursor/.cursor-plugin/plugin.json",
    "plugins/copilot/plugin.json",
    "plugins/gemini/gemini-extension.json",
    "plugins/devin/.devin-plugin/plugin.json",
    "plugins/zcode/.zcode-plugin/plugin.json",
    "plugins/grok/.grok-plugin/plugin.json",
    "plugins/kimi/kimi.plugin.json",
    "plugins/pi/package.json",
];

fn read_json(path: &std::path::Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{}: not valid JSON: {e}", path.display()))
}

#[test]
fn every_rtok_plugin_version_file_parses_with_schema_1_and_the_right_plugin_name() {
    for host in VERSION_FILE_HOSTS {
        let path = root()
            .join("plugins")
            .join(host)
            .join(".rtok-plugin-version");
        let json = read_json(&path);
        assert_eq!(json["schema"], 1, "{}: schema", path.display());
        assert_eq!(json["plugin"], *host, "{}: plugin", path.display());
        assert_eq!(
            json["version"],
            env!("CARGO_PKG_VERSION"),
            "{}: version",
            path.display()
        );
    }
}

#[test]
fn every_plugin_manifest_version_matches_cargo_pkg_version() {
    for rel in MANIFEST_FILES {
        let path = root().join(rel);
        let json = read_json(&path);
        assert_eq!(
            json["version"],
            env!("CARGO_PKG_VERSION"),
            "{}: version",
            path.display()
        );
    }
}
