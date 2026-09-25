//! Zed installer (`rtok agents install zed`, plan T48.6).
//!
//! Zed reads MCP servers from `context_servers` in `~/.config/zed/settings.json`
//! (`[setup.zed] config_path`): `context_servers.rtok = {command, args}`. The settings file
//! is JSONC — `//` and `/* */` comments and trailing commas (T79) — which `serde_json`
//! rejects, so setup edits the text surgically through [`super::jsonc`] (shared with `vscode`,
//! T117): comments, trailing commas and foreign servers survive installs and removes.
//! Zed has no shell hook events; the Zed agent reads these servers directly, and external
//! agents can reach them over ACP.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply, jsonc};
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
    if let Ok(root) = jsonc::parse(raw) {
        return root
            .pointer("/context_servers/rtok")
            .is_some_and(Value::is_object);
    }
    raw.contains("\"rtok\"")
}

/// Add `context_servers.rtok`, or report no changes. Missing or blank files start as `{}`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.zed.config_path;
    let raw = jsonc::read_or_empty(path)?;
    let (body, edit) = jsonc::upsert_member(&raw, path, "context_servers", NAME, &want_entry())?;
    // Never write a document we cannot read back: "a malformed file is never overwritten"
    // applies to our own output too.
    jsonc::parse(&body).with_context(|| path.display().to_string())?;
    let report = match edit {
        jsonc::Upsert::NoChange => rtok_agent_sdk::NO_CHANGES.into(),
        jsonc::Upsert::Added => format!("+ context_servers.{NAME}: {}", summary()),
        jsonc::Upsert::Replaced => format!("~ context_servers.{NAME}: {}", summary()),
    };
    rtok_agent_sdk::write(&apply(cfg), path, &body, &report)?;
    Ok(report)
}

/// Drop `context_servers.rtok`, keeping every comment and foreign server — but only as far as
/// rtok wrote it: [`rtok_agent_sdk::judge_owned`] (T246, T246.5) leaves an entry that does not
/// run the rtok binary, or one the user changed from [`want_entry`] unless `--yes` says remove.
/// A malformed document is left for [`jsonc::remove_member`] to error on, as before. An object
/// left with no entries and no comments goes with it; a comment-only object stays.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.zed.config_path;
    let raw = jsonc::read_or_empty(path)?;
    if let Ok(root) = jsonc::parse(&raw)
        && let Some(have) = root.pointer("/context_servers/rtok")
    {
        let at = format!("context_servers.{NAME} in {}", path.display());
        if let Some(leave) =
            rtok_agent_sdk::judge_owned(&apply(cfg), &at, have, &want_entry(), super::is_rtok_bin)
        {
            return Ok(leave);
        }
    }
    let (body, removed) = jsonc::remove_member(&raw, path, "context_servers", NAME)?;
    jsonc::parse(&body).with_context(|| path.display().to_string())?;
    let report = if removed {
        format!("- context_servers.{NAME}")
    } else {
        rtok_agent_sdk::NO_CHANGES.into()
    };
    rtok_agent_sdk::write(&apply(cfg), path, &body, &report)?;
    Ok(report)
}

/// `tests/agent_remove.rs` strips comments before parsing what `remove` left behind.
pub use super::jsonc::strip_comments;

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
    /// whether `rtok` is on PATH. The scanner itself (strings, comments, nesting) is covered
    /// once, generically, in `super::jsonc`'s own tests (T117) — this only checks that a
    /// document already carrying our exact pretty entry, comments and all, is a no-op.
    #[test]
    fn idempotent_against_a_document_already_carrying_our_entry() {
        let raw = "{\n  // \"context_servers\": fake\n  \"url\": \"https://x/{\\\"a\\\"}\",\n  /* multi\n  \"rtok\": 1 */\n  \"context_servers\": {\"other\": [1, {\"rtok\": 2}], \"rtok\": {\"command\": \"rtok\", \"args\": [\"mcp\"]}}\n}\n";
        assert_eq!(
            jsonc::parse(raw).unwrap()["context_servers"]["rtok"],
            entry()
        );
        let (body, edit) = jsonc::upsert_member(
            raw,
            std::path::Path::new("t"),
            "context_servers",
            NAME,
            &entry(),
        )
        .unwrap();
        assert_eq!(edit, jsonc::Upsert::NoChange, "{body}");
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

    /// T79: Zed writes JSONC — comments and trailing commas. Both survive install and
    /// remove, and a trailing comma inside `context_servers` no longer makes the span
    /// parse fail and replace the whole object (which lost the foreign servers).
    #[test]
    fn trailing_commas_survive_install_and_remove() {
        let (c, path) = cfg("trail", false);
        fs::write(
            &path,
            "{\n  // my theme\n  \"theme\": \"One Dark\",\n  \"context_servers\": {\n    // foreign\n    \"other\": {\"command\": \"npx\", \"args\": [\"x\"],},\n  },\n}\n",
        )
        .unwrap();
        assert!(
            register_mcp(&c)
                .unwrap()
                .starts_with("+ context_servers.rtok: ")
        );
        let raw = fs::read_to_string(&path).unwrap();
        assert!(
            raw.contains("// my theme") && raw.contains("// foreign"),
            "{raw}"
        );
        assert!(raw.contains("\"other\""), "{raw}");
        let root = jsonc::parse(&raw).unwrap();
        assert_eq!(root["context_servers"]["rtok"]["args"], json!(["mcp"]));
        assert_eq!(root["context_servers"]["other"]["command"], "npx");
        assert_eq!(unregister_mcp(&c).unwrap(), "- context_servers.rtok");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("\"rtok\""), "{raw}");
        assert!(
            raw.contains("// my theme") && raw.contains("\"other\""),
            "{raw}"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// T79: a root object already ending in a trailing comma gains one separator, not two —
    /// the real-file shape that used to produce `},\\n,` and break the next install.
    #[test]
    fn a_root_trailing_comma_adds_no_second_comma() {
        let (c, path) = cfg("root-trail", false);
        fs::write(&path, "{\n  // mine\n  \"theme\": \"One Dark\",\n}\n").unwrap();
        assert!(
            register_mcp(&c)
                .unwrap()
                .starts_with("+ context_servers.rtok: ")
        );
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("\n,"), "{raw}");
        assert!(raw.contains("// mine"), "{raw}");
        let root = jsonc::parse(&raw).unwrap();
        assert_eq!(root["theme"], json!("One Dark"));
        assert_eq!(root["context_servers"]["rtok"]["args"], json!(["mcp"]));
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
