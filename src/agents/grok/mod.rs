// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

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
use serde_json::{Value, json};
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
        &[rtok_plugin_sdk::Surface::Hook]
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
        if covered(cfg, "mcp")
            || super::mcp::has_toml_entry(&cfg.setup.grok.config_path, "mcp_servers", "rtok")
        {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        if remove {
            return Ok(vec![offer_plugin(cfg, true)?, unregister_mcp(cfg)?]);
        }
        // The hooks note first (covered by the plugin or the Claude import → say so, never a
        // second set), then the MCP table. MCP is independent of the plugin (T275/D33):
        // install/update always try to write `[mcp_servers.rtok]`, plugin installed or not —
        // `register_mcp` skips it only while the Claude import already covers rtok's MCP
        // ([compat.claude], unchanged); only remove takes a written entry out.
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
pub(crate) fn covered(cfg: &Config, module: &str) -> bool {
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

/// The `[mcp_servers.rtok]` table [`register_mcp`] writes, as JSON: the shape
/// [`rtok_agent_sdk::judge_owned`] compares a live table against (T246.5). `cmd` only stands in
/// for the comparison — `judge_owned` treats every rtok binary string as the same one, so the
/// literal command never matters.
fn mcp_entry(cmd: &str) -> Value {
    json!({"command": cmd, "args": ["mcp"]})
}

/// Take back the `rtok` slot only as far as rtok wrote it: [`rtok_agent_sdk::judge_owned`]
/// (T246, T246.5) leaves a slot that does not run the rtok binary, or one the user changed
/// from [`mcp_entry`] unless `--yes` says remove. A missing slot is already the goal.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    let path = &cfg.setup.grok.config_path;
    let mut doc = load(path)?;
    let have = doc
        .get("mcp_servers")
        .and_then(|s| s.get("rtok"))
        .map(super::mcp::toml_item_to_json);
    let Some(have) = have else {
        return Ok(NO_CHANGES.into());
    };
    let at = format!("mcp_servers.rtok in {}", path.display());
    if let Some(leave) = rtok_agent_sdk::judge_owned(
        &apply(cfg),
        &at,
        &have,
        &mcp_entry("rtok"),
        super::is_rtok_bin,
    ) {
        return Ok(leave);
    }
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
        assert!(s.contains("ketch install pyrlyn/rtok"), "{s}");
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

    /// T275/D33: MCP is independent of the plugin — while it is installed, install/update
    /// still keeps `[mcp_servers.rtok]` (and would still write one from scratch); only
    /// remove takes it out. `[mcp_servers.other]` is never touched either way.
    #[test]
    fn plugin_installed_keeps_mcp_table_remove_takes_it_out() {
        let dir = tmp("single");
        let c = cfg(&dir, false, true);
        fs::create_dir_all(plugin_marker(&c)).unwrap();
        fs::write(
            c.setup.grok.config_path.clone(),
            "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]\n\n[mcp_servers.other]\ncommand = \"x\"\n",
        )
        .unwrap();
        Grok.apply(&c, Kind::Cli, Mode::Install).unwrap();
        let text = fs::read_to_string(c.setup.grok.config_path.clone()).unwrap();
        assert!(text.contains("[mcp_servers.rtok]"), "{text}");
        assert!(text.contains("[mcp_servers.other]"), "{text}");
        assert!(Grok.installed(&c, Kind::Cli).contains(&"mcp"));

        let lines = Grok.apply(&c, Kind::Cli, Mode::Remove).unwrap();
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

    /// T275/D33: install/update always try to write `[mcp_servers.rtok]`, plugin installed or
    /// not (skipped only while the Claude import already covers it, tested above); only
    /// remove takes a written entry out; a user-edited entry is left with a `leave` line.
    #[test]
    fn t275_mcp_entry_always_written_except_on_remove() {
        let dir = tmp("t275-mcp");
        // `--yes` only gates the plugin-offer line here (T275/D33); leaving it unset keeps
        // (e)'s user-edited entry declined ("leave"), while (a)-(c) write/remove the MCP
        // entry regardless, since `register_mcp`/`unregister_mcp` never check it themselves.
        let c = cfg(&dir, false, false);
        fs::create_dir_all(plugin_marker(&c)).unwrap();
        assert!(plugin_detected(&c));

        let path = c.setup.grok.config_path.clone();
        crate::agents::mcp::assert_toml_entry_lifecycle(
            &Grok,
            &c,
            Kind::Cli,
            &path,
            "mcp_servers",
            || {
                // Unlike the JSON hosts' `register_server`, `register_mcp` here is a plain
                // no-op once any `rtok` table exists, edited or not — so reseeding after
                // (e) left an edited one first clears it, matching what a user actually
                // removing the file's `[mcp_servers.rtok]` by hand would leave behind.
                let mut doc = load(&path).unwrap();
                if let Some(t) = doc
                    .get_mut("mcp_servers")
                    .and_then(toml_edit::Item::as_table_mut)
                {
                    t.remove("rtok");
                }
                rtok_agent_sdk::write(&apply(&c), &path, &doc.to_string(), "reset").unwrap();
                register_mcp(&c)
            },
        );
        let _ = fs::remove_dir_all(dir);
    }
}
