//! GitHub Copilot installer (`rtok agents install copilot`, plan T46.4).
//!
//! Copilot CLI (`copilot`) and the GitHub Copilot app share `~/.copilot` (`[setup.copilot] dir`):
//! MCP in `mcp-config.json` (`mcpServers.<name> = {type: "local", command, args, tools}`), hooks
//! as any `hooks/*.json` file. rtok owns `hooks/rtok.json` outright — nothing to merge, `remove`
//! deletes it — and its commands run `rtok hook <Event> --host copilot`, which maps Copilot's
//! camelCase payloads (T46.3). The app does not document hooks, so that variant reports `no`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rtok_agent_sdk::{Apply, NO_CHANGES, edit_json};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

/// Copilot's event names paired with the Claude event `rtok hook` runs for them.
const EVENTS: &[(&str, &str)] = &[
    ("preToolUse", "PreToolUse"),
    ("postToolUse", "PostToolUse"),
    ("userPromptSubmitted", "UserPromptSubmit"),
    ("sessionStart", "SessionStart"),
    ("sessionEnd", "SessionEnd"),
];

/// The CLI and the desktop app read the same `~/.copilot` files.
pub struct Copilot;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Copilot CLI",
        bins: &["copilot"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "GitHub Copilot",
        bins: &[],
        apps: &[
            "/Applications/GitHub Copilot.app",
            "$LOCALAPPDATA/Programs/GitHub Copilot/GitHub Copilot.exe",
        ],
    },
];

/// `<dir>/mcp-config.json`.
pub fn mcp_path(cfg: &Config) -> PathBuf {
    cfg.setup.copilot.dir.join("mcp-config.json")
}

/// `<dir>/hooks/rtok.json` — rtok's own file; Copilot loads every `hooks/*.json`.
pub fn hooks_path(cfg: &Config) -> PathBuf {
    cfg.setup.copilot.dir.join("hooks").join("rtok.json")
}

impl Agent for Copilot {
    fn id(&self) -> &'static str {
        "copilot"
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

    fn support(&self, kind: Kind, module: &str) -> Support {
        match (module, kind) {
            ("hooks", Kind::Cli) | ("mcp", _) => Support::Yes,
            ("hooks", _) => Support::No(
                "the GitHub Copilot app does not document hooks; the shared hooks/rtok.json is written for the CLI",
            ),
            ("proxy", _) => Support::No(
                "Copilot BYOK is env-only (COPILOT_PROVIDER_BASE_URL); there is no config file to point at the proxy",
            ),
            _ => Support::No(
                "Copilot plugins live in installed-plugins/, owned by `copilot plugin`; there is no local plugin directory to link",
            ),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![hooks_path(cfg), mcp_path(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        if super::read(&hooks_path(cfg)).contains(" hook PreToolUse") {
            out.push("hooks");
        }
        if super::read(&mcp_path(cfg)).contains("\"rtok\"") {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines = vec![run(cfg, remove)?];
        if remove {
            lines.push(unregister_mcp(cfg)?);
        } else if cfg.setup.mcp {
            lines.push(register_mcp(cfg)?);
        }
        Ok(lines)
    }
}

/// Write, dry-run, or delete `hooks/rtok.json`. The file is rtok's, so apply means "make it
/// this document" and an identical file is no change.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = hooks_path(cfg);
    if remove {
        return remove_file(&apply(cfg), &path);
    }
    let want = hooks_doc(&super::rtok_hook_bin(), cfg.setup.hook_timeout_s);
    edit_json(&apply(cfg), &path, |root| {
        if *root == want {
            return NO_CHANGES.into();
        }
        *root = want;
        format!("+ {} ({} events)", path.display(), EVENTS.len())
    })
}

/// `{version: 1, hooks: {<event>: [{type: "command", bash, powershell, timeoutSec}]}}`.
fn hooks_doc(bin: &str, timeout: u64) -> Value {
    let mut hooks = serde_json::Map::new();
    for &(copilot, claude) in EVENTS {
        let cmd = format!("{bin} hook {claude} --host copilot");
        hooks.insert(
            copilot.into(),
            json!([{"type": "command", "bash": cmd, "powershell": cmd, "timeoutSec": timeout}]),
        );
    }
    json!({"version": 1, "hooks": hooks})
}

/// Delete a file rtok owns: backed up like any edit, reported as `- <path>`; absent is no change.
fn remove_file(apply: &Apply, path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(NO_CHANGES.into());
    }
    let report = format!("- {}", path.display());
    if apply.writes(&report) {
        if apply.backup {
            rtok_agent_sdk::backup(path)?;
        }
        fs::remove_file(path).with_context(|| path.display().to_string())?;
    }
    Ok(report)
}

/// `mcpServers.rtok = {type: "local", command, args, tools: ["*"]}` in `mcp-config.json`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    let entry = json!({"type": "local", "command": cmd, "args": ["mcp"], "tools": ["*"]});
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &mcp_path(cfg),
        "mcpServers",
        NAME,
        entry,
        &format!("{cmd} mcp"),
    )
}

/// Drop `mcpServers.rtok` from `mcp-config.json`.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_server(&apply(cfg), &mcp_path(cfg), "mcpServers", NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-copilot-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.setup.copilot.dir = dir.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, dir)
    }

    #[test]
    fn dry_run_names_the_hook_file_and_creates_nothing() {
        let (c, dir) = cfg("dry", true);
        let out = run(&c, false).unwrap();
        assert!(
            out.starts_with("+ ") && out.ends_with("(5 events)"),
            "{out}"
        );
        assert!(!hooks_path(&c).exists());
        assert!(Copilot.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_writes_five_events_is_idempotent_and_remove_deletes() {
        let (c, dir) = cfg("apply", false);
        assert!(run(&c, false).unwrap().starts_with("+ "));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let doc: Value =
            serde_json::from_str(&fs::read_to_string(hooks_path(&c)).unwrap()).unwrap();
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["hooks"].as_object().unwrap().len(), 5);
        let pre = &doc["hooks"]["preToolUse"][0];
        assert_eq!(pre["type"], "command");
        assert_eq!(pre["timeoutSec"], 5);
        let bash = pre["bash"].as_str().unwrap();
        assert!(
            bash.ends_with("rtok hook PreToolUse --host copilot"),
            "{bash}"
        );
        assert_eq!(pre["powershell"], pre["bash"]);
        assert!(
            doc["hooks"]["userPromptSubmitted"][0]["bash"]
                .as_str()
                .unwrap()
                .contains(" hook UserPromptSubmit ")
        );
        assert_eq!(Copilot.installed(&c, Kind::Cli), ["hooks"]);

        let gone = run(&c, true).unwrap();
        assert_eq!(gone, format!("- {}", hooks_path(&c).display()));
        assert!(!hooks_path(&c).exists());
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn mcp_entry_is_local_with_all_tools_and_the_app_reports_no_hooks() {
        let (c, dir) = cfg("mcp", false);
        let first = register_mcp(&c).unwrap();
        assert!(first.starts_with("mcpServers.rtok: "), "{first}");
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let doc: Value = serde_json::from_str(&fs::read_to_string(mcp_path(&c)).unwrap()).unwrap();
        let rtok = &doc["mcpServers"]["rtok"];
        assert_eq!(rtok["type"], "local");
        assert_eq!(rtok["args"], json!(["mcp"]));
        assert_eq!(rtok["tools"], json!(["*"]));
        assert_eq!(Copilot.installed(&c, Kind::Desktop), ["mcp"]);
        assert_eq!(unregister_mcp(&c).unwrap(), "- mcpServers.rtok");
        assert!(!fs::read_to_string(mcp_path(&c)).unwrap().contains("rtok"));

        assert!(matches!(Copilot.support(Kind::Cli, "hooks"), Support::Yes));
        assert!(matches!(
            Copilot.support(Kind::Desktop, "hooks"),
            Support::No(_)
        ));
        assert!(Copilot.shared());
        let _ = fs::remove_dir_all(dir);
    }
}
