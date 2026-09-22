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
use rtok_agent_sdk::{Apply, KETCH_INSTALL, NO_CHANGES, edit_json};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

/// Copilot's event names paired with the Claude event `rtok hook` runs for them.
pub const EVENTS: &[(&str, &str)] = &[
    ("preToolUse", "PreToolUse"),
    ("postToolUse", "PostToolUse"),
    ("userPromptSubmitted", "UserPromptSubmit"),
    ("sessionStart", "SessionStart"),
    ("sessionEnd", "SessionEnd"),
    ("preCompact", "PreCompact"),
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
            ("plugin", Kind::Cli) => Support::Flag("--yes"),
            ("plugin", _) => Support::No(
                "the GitHub Copilot app does not document plugin installs; `copilot plugin` serves the CLI",
            ),
            _ => Support::No(
                "Copilot plugins live in installed-plugins/, owned by `copilot plugin`; there is no local plugin directory to link",
            ),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[
            rtok_plugin_sdk::Surface::Hook,
            rtok_plugin_sdk::Surface::Mcp,
        ]
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
        if plugin_installed(cfg) {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let head = plugin(cfg, remove)?;
        if remove || plugin_installed(cfg) {
            // D21: the plugin is the unit — its hooks and `rtok mcp` serve already, so
            // rtok's own hooks/rtok.json and mcp-config.json entry go instead of coming
            // (kimi's `plugin_detected` rule), on the same run that installs it too.
            return Ok(vec![
                head,
                run(cfg, true)?,
                unregister_mcp(cfg)?,
                super::skill::sync("copilot", cfg, remove)?,
            ]);
        }
        let mut lines = vec![head, run(cfg, false)?];
        if cfg.setup.mcp {
            lines.push(register_mcp(cfg)?);
        }
        lines.push(super::skill::sync("copilot", cfg, false)?);
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
/// The plugin tree's `hooks/hooks.json` is this document with `bin = "rtok"`, pinned by
/// `tests/copilot_plugin.rs` — one shape, two surfaces (D21).
pub fn hooks_doc(bin: &str, timeout: u64) -> Value {
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

const PLUGIN_SRC: &str = "plugins/copilot";

/// True while `copilot plugin` has rtok installed (T116): the cached copy under
/// `installed-plugins/` — `MARKETPLACE/PLUGIN-NAME` from a marketplace, `_direct/<id>` from
/// a local path — is the host's own record. `copilot plugin list --json` reports the same
/// state; the manifest answers without spawning the CLI (T75).
pub fn plugin_installed(cfg: &Config) -> bool {
    let root = cfg.setup.copilot.dir.join("installed-plugins");
    let Ok(outer) = fs::read_dir(&root) else {
        return false;
    };
    outer.flatten().any(|o| {
        fs::read_dir(o.path())
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                fs::read_to_string(e.path().join("plugin.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .is_some_and(|v| v.get("name").and_then(Value::as_str) == Some(NAME))
            })
    })
}

/// One `copilot plugin …` call; `COPILOT_HOME` only when `dir` is not the default. Windows
/// shim resolution and stderr handling live in `super::run_cli` (T140).
fn copilot_cli(cfg: &Config, args: &[&str]) -> std::result::Result<(), String> {
    let dir = cfg.setup.copilot.dir.as_path();
    let default = super::home_dir().join(".copilot");
    let env = (dir != default).then_some(("COPILOT_HOME", dir));
    super::run_cli("copilot", args, env)
}

/// Offer, install, or uninstall `plugins/copilot` through `copilot plugin` (T116): the
/// documented local-path install (`copilot plugin install <dir>`), uninstall by the
/// manifest's `name`. Behind `--yes` (`Support::Flag`) — rtok never writes
/// `installed-plugins/`, that store is Copilot's, so the flag never turns the printed line
/// into state (`installed()` reads the marker alone). A failing or missing `copilot` keeps
/// the offer open instead of failing the install: the settings-file hooks still go in.
fn plugin(cfg: &Config, remove: bool) -> Result<String> {
    let a = apply(cfg);
    let installed = plugin_installed(cfg);
    if remove {
        if !installed {
            return Ok(NO_CHANGES.into());
        }
    } else if installed || !a.yes {
        return Ok(NO_CHANGES.into());
    }
    let src = super::plugin_src(PLUGIN_SRC);
    let shown = if remove {
        format!("copilot plugin uninstall {NAME}")
    } else {
        format!("copilot plugin install {}", src.display())
    };
    if a.dry_run {
        return Ok(if remove {
            format!("- plugin {NAME} ({shown})")
        } else {
            format!("offer {PLUGIN_SRC} → {shown} {KETCH_INSTALL}")
        });
    }
    let args: Vec<&str> = if remove {
        vec!["plugin", "uninstall", NAME]
    } else {
        vec!["plugin", "install", src.to_str().unwrap_or_default()]
    };
    match copilot_cli(cfg, &args) {
        Ok(()) => Ok(if remove {
            format!("- plugin {NAME}")
        } else {
            format!("+ plugin {PLUGIN_SRC} → {NAME}")
        }),
        Err(e) => Ok(format!(
            "offer {PLUGIN_SRC} → {shown} (copilot failed: {e})"
        )),
    }
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
            out.starts_with("+ ") && out.ends_with("(6 events)"),
            "{out}"
        );
        assert!(!hooks_path(&c).exists());
        assert!(Copilot.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_writes_six_events_is_idempotent_and_remove_deletes() {
        let (c, dir) = cfg("apply", false);
        assert!(run(&c, false).unwrap().starts_with("+ "));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let doc: Value =
            serde_json::from_str(&fs::read_to_string(hooks_path(&c)).unwrap()).unwrap();
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["hooks"].as_object().unwrap().len(), 6);
        assert!(doc["hooks"].get("preCompact").is_some());
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

    /// T116: the offer line names the documented local install and runs nothing without
    /// `--yes`; the marker alone answers `installed()` (T75) and stills the offer.
    #[test]
    fn plugin_offers_the_local_install_behind_yes() {
        let (mut c, dir) = cfg("offer", true);
        assert_eq!(plugin(&c, false).unwrap(), NO_CHANGES, "no --yes, no line");
        c.setup.yes = true;
        let s = plugin(&c, false).unwrap();
        assert!(s.contains("plugins/copilot"), "{s}");
        assert!(s.contains("copilot plugin install"), "{s}");
        assert!(s.contains("ketch install listepo/rtok"), "{s}");
        assert!(!plugin_installed(&c));
        let marker = c
            .setup
            .copilot
            .dir
            .join("installed-plugins/_direct/x/plugin.json");
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, r#"{"name":"rtok"}"#).unwrap();
        assert!(plugin_installed(&c));
        assert_eq!(plugin(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(
            plugin(&c, true).unwrap(),
            "- plugin rtok (copilot plugin uninstall rtok)"
        );
        fs::write(&marker, r#"{"name":"other"}"#).unwrap();
        assert!(!plugin_installed(&c), "a foreign manifest is not ours");
        let _ = fs::remove_dir_all(dir);
    }

    /// D21: while the plugin serves, its hooks and MCP are the unit — an earlier plain
    /// install's `hooks/rtok.json` and `mcpServers.rtok` are taken back, never added again.
    #[test]
    fn the_plugin_is_the_singleton_over_hooks_and_mcp() {
        let (mut c, dir) = cfg("single", false);
        c.setup.yes = true;
        assert!(run(&c, false).unwrap().starts_with("+ "));
        assert!(register_mcp(&c).unwrap().starts_with("mcpServers.rtok: "));
        let marker = c
            .setup
            .copilot
            .dir
            .join("installed-plugins/_direct/x/plugin.json");
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, r#"{"name":"rtok"}"#).unwrap();
        let lines = Copilot.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("- ") && l.contains("hooks")),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"- mcpServers.rtok".to_string()),
            "{lines:?}"
        );
        assert!(!hooks_path(&c).exists());
        assert!(
            !fs::read_to_string(mcp_path(&c))
                .unwrap_or_default()
                .contains("rtok"),
            "no second rtok mcp"
        );
        assert_eq!(Copilot.installed(&c, Kind::Cli), ["plugin"]);
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

    #[test]
    fn pre_compact_writes_a_checkpoint_note() {
        let (c, dir) = cfg("precompact", false);
        let mut c = c;
        c.hook.host = "copilot".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        let pre = serde_json::json!({
            "sessionId": "cop-compact",
            "trigger": "auto"
        });
        let mut out = Vec::new();
        crate::hooks::run("PreCompact", pre.to_string().as_bytes(), &mut out, &c);
        assert_eq!(out, b"{}");
        let note = crate::store::Store::open(&c.core.db_path)
            .unwrap()
            .latest_note("checkpoint:cop-compact")
            .unwrap();
        assert!(note.is_some(), "preCompact must save a checkpoint");
        let _ = fs::remove_dir_all(dir);
    }
}
