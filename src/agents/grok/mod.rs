//! Grok Build installer (`rtok agents install grok`, plan T100, D21).
//!
//! Grok owns its plugin store — `grok plugin install <dir> --trust` writes
//! `~/.grok/plugins/rtok/` — so setup never writes there; it prints the exact line (dry-run
//! and apply alike), the Kimi rule (T86). While the plugin is absent, the plain
//! `[mcp_servers.rtok]` table in `~/.grok/config.toml` is the MCP path. And Grok fires
//! Claude's hooks and MCP servers through `[compat.claude]` by default: while rtok's Claude
//! install already serves a capability and that import is on, setup says so instead of
//! adding a second set (two sets fire every event twice and run two `rtok mcp` on one store).

use std::path::{Path, PathBuf};

use anyhow::Result;
use rtok_agent_sdk::{KETCH_INSTALL, NO_CHANGES};
use toml_edit::{DocumentMut, Table, value};

use super::{Agent, Kind, Mode, Support, Variant, apply, rtok_command};
use crate::config::Config;

/// Grok Build: one CLI (`grok` on PATH, or the binary under `~/.grok` / `GROK_HOME`).
pub struct Grok;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "Grok Build",
    bins: &["grok"],
    apps: &["~/.grok/bin/grok", "$GROK_HOME/bin/grok"],
}];

impl Agent for Grok {
    fn id(&self) -> &'static str {
        "grok"
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
            "plugin" => Support::Offer("--yes"),
            "mcp" => Support::Yes,
            "hooks" => Support::No(
                "Grok fires one hook set — the plugin's, or rtok's Claude hooks through [compat.claude] hooks — and setup adds no second (D21)",
            ),
            _ => Support::No(
                "Grok Build providers live in its own settings tables; setup does not edit them",
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
        vec![cfg.setup.grok.config_path.clone()]
    }

    fn markers(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.grok.config_path.clone(), plugin_marker(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        if plugin_detected(cfg) {
            out.push("plugin");
        }
        // The hooks module reads installed whenever the one set that will fire is rtok's:
        // the plugin's (above) or, through the Claude import, rtok's Claude hooks (D21).
        if covered(cfg, "hooks") {
            out.push("hooks");
        }
        if covered(cfg, "mcp") || own_mcp(cfg) {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        if remove {
            return Ok(vec![offer_plugin(cfg, true)?, unregister_mcp(cfg)?]);
        }
        if plugin_detected(cfg) {
            // D21: the plugin is the unit — its hooks and `rtok mcp` serve already, so no
            // `[mcp_servers.rtok]` table is added (and an earlier one is taken back).
            return Ok(vec![offer_plugin(cfg, false)?, unregister_mcp(cfg)?]);
        }
        // Plain path: the hooks note first (covered → say so, never a second set), then the
        // MCP table — skipped for the same reason while the Claude import serves rtok.
        let mut lines = vec![hooks_note(cfg)];
        if cfg.setup.mcp {
            lines.push(register_mcp(cfg)?);
        }
        if lines.iter().any(|l| l != NO_CHANGES) && cfg.setup.yes {
            lines.insert(0, offer_plugin(cfg, false)?);
        } else {
            lines.insert(0, NO_CHANGES.into());
        }
        Ok(lines)
    }
}

/// The plugin marker rtok can honestly read (T100): the copy `grok plugin install <dir>
/// --trust` writes — `<grok home>/plugins/rtok/`, where `<grok home>` is the dir holding
/// `config.toml` (so `[setup.grok] config_path` moves everything together).
pub fn plugin_marker(cfg: &Config) -> PathBuf {
    home(cfg).join("plugins").join("rtok")
}

fn home(cfg: &Config) -> &Path {
    cfg.setup
        .grok
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
}

/// True while Grok's own install of the plugin serves hooks and MCP (D21): setup then keeps
/// its own table away instead of adding it. `grok plugin list --json` reports the same state;
/// the directory marker answers without spawning the CLI.
pub fn plugin_detected(cfg: &Config) -> bool {
    plugin_marker(cfg).is_dir()
}

/// Grok runs Claude's hooks (`~/.claude/settings.json`) and MCP servers (`~/.claude.json`)
/// while `[compat.claude]` is on (the default) — the files the import reads, never Claude's
/// own plugin (T100). `covered` is that import serving rtok already: the D21 say-so state.
fn covered(cfg: &Config, module: &str) -> bool {
    let key = match module {
        "hooks" => "hooks",
        "mcp" => "mcps",
        _ => return false,
    };
    compat_claude(cfg, key) && super::claude::files_serve_rtok(cfg).contains(&module)
}

fn compat_claude(cfg: &Config, key: &str) -> bool {
    load(&cfg.setup.grok.config_path)
        .ok()
        .and_then(|doc| {
            doc.get("compat")
                .and_then(|c| c.get("claude"))
                .and_then(|c| c.get(key))
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(true)
}

fn own_mcp(cfg: &Config) -> bool {
    load(&cfg.setup.grok.config_path).ok().is_some_and(|doc| {
        doc.get("mcp_servers")
            .and_then(|s| s.get("rtok"))
            .is_some_and(|_| true)
    })
}

/// D21: while the Claude import already fires rtok's hooks here, say so instead of adding a
/// second set. Behind `--yes` like the offer line — guidance counts as a change only then.
fn hooks_note(cfg: &Config) -> String {
    if covered(cfg, "hooks") && apply(cfg).yes {
        "hooks: covered by rtok's Claude hooks ([compat.claude] hooks); no second set (D21)".into()
    } else {
        NO_CHANGES.into()
    }
}

/// The `grok plugin install <resolved plugins/grok> --trust` line (T100): printed behind
/// `--yes` only, on dry-run and apply alike; rtok never writes `~/.grok/plugins/` — that
/// store is Grok's. On remove the plugin is left alone with its own remove line.
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    if remove {
        return if plugin_detected(cfg) {
            Ok("keep ~/.grok/plugins/rtok (owned by Grok; remove with `grok plugin uninstall rtok`)".into())
        } else {
            Ok(NO_CHANGES.into())
        };
    }
    if !apply(cfg).yes {
        return Ok(NO_CHANGES.into());
    }
    if let Some(note) = plugin_offer_windows_note(cfg!(windows)) {
        return Ok(note.into());
    }
    Ok(format!(
        "offer plugins/grok → grok plugin install {} --trust {KETCH_INSTALL}",
        super::plugin_src("plugins/grok").display()
    ))
}

/// T250.4: the plugin's hooks are a POSIX shell one-liner; Grok runs hooks through PowerShell
/// on Windows, where that does not run, so install skips the offer there instead of printing
/// a command for a plugin whose hooks would never fire.
fn plugin_offer_windows_note(windows: bool) -> Option<&'static str> {
    windows.then_some(
        "skip plugins/grok (macOS/Linux only: its hooks are POSIX shell; Grok imports rtok's Claude hooks — rtok agents install claude)",
    )
}

/// `[mcp_servers.rtok]` in `config.toml` — Grok's documented MCP shape. Written only while
/// the plugin is absent and the Claude import does not already run rtok's MCP (D21); the
/// `rtok` slot is ours to own, every other name stays.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    if covered(cfg, "mcp") {
        return Ok(if apply(cfg).yes {
            "mcp: covered by rtok's Claude MCP ([compat.claude] mcps); no second server (D21)"
                .into()
        } else {
            NO_CHANGES.into()
        });
    }
    let path = &cfg.setup.grok.config_path;
    let mut doc = load(path)?;
    let servers = doc
        .entry("mcp_servers")
        .or_insert_with(|| Table::new().into())
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("mcp_servers is not a table"))?;
    if servers.contains_key("rtok") {
        return Ok(NO_CHANGES.into());
    }
    let mut entry = Table::new();
    entry.insert("command", value(rtok_command()));
    entry.insert("args", value(toml_edit::Array::from_iter(["mcp"])));
    servers.insert("rtok", entry.into());
    let summary = format!("{} mcp", rtok_command());
    let report = format!("mcp_servers.rtok: {summary}");
    rtok_agent_sdk::write(&apply(cfg), path, &doc.to_string(), &report)?;
    Ok(report)
}

/// Take back the `rtok` slot; a missing slot is already the goal.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.grok.config_path;
    let mut doc = load(path)?;
    let Some(servers) = doc.get_mut("mcp_servers").and_then(|s| s.as_table_mut()) else {
        return Ok(NO_CHANGES.into());
    };
    if servers.remove("rtok").is_none() {
        return Ok(NO_CHANGES.into());
    }
    rtok_agent_sdk::write(&apply(cfg), path, &doc.to_string(), "- mcp_servers.rtok")?;
    Ok("- mcp_servers.rtok".into())
}

fn load(path: &Path) -> Result<DocumentMut> {
    let text = super::read(path);
    Ok(text.parse().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-grok-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cfg(dir: &Path, dry: bool, yes: bool) -> Config {
        let mut c = Config::default();
        c.setup.grok.config_path = dir.join("config.toml");
        c.setup.claude.settings_path = dir.join("claude-settings.json");
        c.doctor.claude_json = dir.join("claude.json");
        c.setup.dry_run = dry;
        c.setup.yes = yes;
        c.setup.backup = false;
        c
    }

    /// T250.4: the decision is pure so both OSes are covered without a real Windows box.
    #[test]
    fn plugin_offer_windows_note_skips_only_on_windows() {
        assert_eq!(plugin_offer_windows_note(false), None);
        let note = plugin_offer_windows_note(true).unwrap();
        assert!(note.contains("macOS/Linux only"), "{note}");
        assert!(note.contains("rtok agents install claude"), "{note}");
    }

    // Windows skips the offer (T250.4); `plugin_offer_windows_note_skips_only_on_windows` covers it.
    #[cfg(not(windows))]
    #[test]
    fn dry_run_offer_names_the_grok_plugin_command() {
        let dir = tmp("offer");
        let c = cfg(&dir, true, true);
        let s = offer_plugin(&c, false).unwrap();
        assert!(s.contains("plugins/grok"), "{s}");
        assert!(s.contains("grok plugin install"), "{s}");
        assert!(s.contains("--trust"), "{s}");
        assert!(s.contains("ketch install listepo/rtok"), "{s}");
        assert!(!plugin_marker(&c).exists());
        assert_eq!(
            offer_plugin(&cfg(&dir, true, false), false).unwrap(),
            NO_CHANGES
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// Detection: the plugin marker and the MCP table read back as `installed()`, and the
    /// `[compat.claude]` import reports the Claude set as covered (D21).
    #[test]
    fn detection_reads_marker_table_and_the_claude_import() {
        let dir = tmp("detect");
        let c = cfg(&dir, false, true);
        assert!(Grok.installed(&c, Kind::Cli).is_empty());
        fs::create_dir_all(plugin_marker(&c)).unwrap();
        fs::write(
            c.setup.grok.config_path.clone(),
            "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n",
        )
        .unwrap();
        assert_eq!(Grok.installed(&c, Kind::Cli), vec!["plugin", "mcp"]);
        fs::write(
            c.setup.claude.settings_path.clone(),
            r#"{"hooks":{"PostToolUse":[{"hooks":[{"command":"rtok hook PostToolUse"}]}]}}"#,
        )
        .unwrap();
        fs::write(
            c.setup.grok.config_path.clone(),
            "[compat.claude]\nhooks = true\nmcps = false\n\n[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n",
        )
        .unwrap();
        assert_eq!(
            Grok.installed(&c, Kind::Cli),
            vec!["plugin", "hooks", "mcp"]
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// D21: while the plugin serves, no `[mcp_servers.rtok]` table is added and an earlier
    /// one is taken back.
    #[test]
    fn the_plugin_is_the_singleton_no_second_mcp_table() {
        let dir = tmp("single");
        let c = cfg(&dir, false, true);
        fs::create_dir_all(plugin_marker(&c)).unwrap();
        fs::write(
            c.setup.grok.config_path.clone(),
            "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n\n[mcp_servers.other]\ncommand = \"x\"\n",
        )
        .unwrap();
        let lines = Grok.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(
            lines.contains(&"- mcp_servers.rtok".to_string()),
            "{lines:?}"
        );
        let text = fs::read_to_string(c.setup.grok.config_path.clone()).unwrap();
        assert!(!text.contains("[mcp_servers.rtok]"), "{text}");
        assert!(text.contains("[mcp_servers.other]"), "{text}");
        let _ = fs::remove_dir_all(dir);
    }

    /// D21: the Claude import fires rtok's hooks and MCP already — the run says so instead
    /// of adding a second set or a second `rtok mcp`, and a repeat stays a repeat.
    #[test]
    fn the_claude_import_is_said_not_duplicated() {
        let dir = tmp("covered");
        let c = cfg(&dir, false, true);
        fs::write(
            c.setup.claude.settings_path.clone(),
            r#"{"hooks":{"PostToolUse":[{"hooks":[{"command":"rtok hook PostToolUse"}]}]}}"#,
        )
        .unwrap();
        fs::write(
            c.doctor.claude_json.clone(),
            r#"{"mcpServers":{"rtok":{"command":"rtok","args":["mcp"]}}}"#,
        )
        .unwrap();
        let lines = Grok.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("hooks: covered by rtok's Claude hooks")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("mcp: covered by rtok's Claude MCP")),
            "{lines:?}"
        );
        let text = fs::read_to_string(&c.setup.grok.config_path).unwrap_or_default();
        assert!(!text.contains("[mcp_servers"), "{text}");
        let _ = fs::remove_dir_all(dir);
    }
}
