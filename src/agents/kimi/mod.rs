//! Kimi Code CLI + Desktop (`rtok agents install kimi`, plan T46.2, T86).
//!
//! Moonshot's Kimi Code CLI reads hooks as `[[hooks]]` tables in `~/.kimi-code/config.toml`
//! (`event`, `matcher`, `command`, `timeout` in seconds; Claude-compatible stdin, exit 2
//! blocks) and MCP from the sibling `mcp.json` (`mcpServers.<name> = {command, args}`, no
//! `type`). The TOML goes through `toml_edit` so the user's comments and other hooks survive.
//!
//! T86 (plugin offer + D21 singleton): Kimi owns its plugin store
//! (`<kimi home>/plugins/managed/`, no local directory to link), so install never writes
//! there — it prints the exact `/plugins install <resolved plugins/kimi path>` line
//! (dry-run and apply alike). While the plugin is installed
//! (`<kimi home>/plugins/managed/rtok/kimi.plugin.json` exists), setup strips rtok's own
//! `[[hooks]]` tables and `mcpServers.rtok` instead of adding them, so no event fires
//! twice and one `rtok mcp` serves the store.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rtok_agent_sdk::{KETCH_INSTALL, NO_CHANGES};
use serde_json::json;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use super::claude::{ENTRIES, is_ours};
use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

/// Kimi Code CLI + Desktop: `[[hooks]]` in `config.toml`, `mcpServers.rtok` in `mcp.json`.
/// The desktop app (`Kimi Code.app`) manages the same files as the CLI.
pub struct Kimi;

/// CLI (`kimi` on PATH) and Desktop (`Kimi Code.app`): one install writes the
/// same two files (T86).
static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Kimi Code",
        bins: &["kimi"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Kimi Code Desktop",
        bins: &[],
        apps: &[
            "/Applications/Kimi Code.app",
            "$LOCALAPPDATA/Programs/Kimi Code/Kimi Code.exe",
        ],
    },
];

/// `mcp.json` lives beside `config.toml`; one key configures both.
pub fn mcp_path(cfg: &Config) -> PathBuf {
    cfg.setup
        .kimi
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mcp.json")
}

impl Agent for Kimi {
    fn id(&self) -> &'static str {
        "kimi"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "hooks" | "mcp" => Support::Yes,
            "plugin" => Support::Flag("--yes"),
            "proxy" => Support::No(
                "Kimi Code providers are [providers.<name>] tables with their own base_url and keys; setup does not edit them",
            ),
            _ => Support::No(
                "Kimi Code plugins live in plugins/managed/, owned by `kimi plugin install`; there is no local plugin directory to link",
            ),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[
            rtok_plugin_sdk::Surface::Hook,
            rtok_plugin_sdk::Surface::Mcp,
        ]
    }

    fn shared(&self) -> bool {
        true
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.kimi.config_path.clone(), mcp_path(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        if super::read(&cfg.setup.kimi.config_path).contains("rtok hook") {
            out.push("hooks");
        }
        if super::read(&mcp_path(cfg)).contains("\"rtok\"") {
            out.push("mcp");
        }
        if plugin_detected(cfg) || cfg.setup.yes {
            // The offer is guidance, not state rtok owns: `--yes` accepts the
            // question it asks, so `plugin` reads back once accepted (like every
            // other flag module). Without the flag it stays declined, unreported.
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines = vec![offer_plugin(cfg, remove)?];
        if remove {
            lines.push(run(cfg, true)?);
            lines.push(unregister_mcp(cfg)?);
        } else if plugin_detected(cfg) {
            // D21 singleton: the plugin serves hooks and MCP, so rtok's own
            // tables go instead of coming (Cursor's `plugin_is_mcp` rule,
            // for hooks too).
            lines.push(run(cfg, true)?);
            lines.push(unregister_mcp(cfg)?);
        } else {
            lines.push(run(cfg, false)?);
            if cfg.setup.mcp {
                lines.push(register_mcp(cfg)?);
            }
        }
        Ok(lines)
    }
}

/// The plugin marker rtok can honestly read (T86): the managed copy Kimi
/// writes on `/plugins install <dir>` — `<kimi home>/plugins/managed/rtok/`,
/// where `<kimi home>` is the dir holding `config.toml`.
pub fn plugin_marker(cfg: &Config) -> PathBuf {
    cfg.setup
        .kimi
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("plugins")
        .join("managed")
        .join("rtok")
        .join("kimi.plugin.json")
}

/// True while Kimi's own install of the plugin serves hooks and MCP (D21):
/// setup then strips its own tables instead of adding them.
pub fn plugin_detected(cfg: &Config) -> bool {
    plugin_marker(cfg).is_file()
}

/// The `/plugins install <resolved plugins/kimi path>` line (T86): printed behind
/// `--yes` only, on dry-run and apply alike; rtok never writes `plugins/managed/`
/// or `installed.json` — that format is Kimi's and undocumented.
/// On remove the managed copy is left alone with its own remove line.
/// Gating on the flag keeps `expected()` honest: without `--yes` the offer stays
/// declined (`NO_CHANGES`), so no `did not read back` warning fires.
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    let a = apply(cfg);
    if a.dry_run {
        if !a.yes {
            return Ok(NO_CHANGES.into());
        }
        return Ok(format!(
            "offer plugins/kimi → /plugins install {} {KETCH_INSTALL}",
            super::plugin_src("plugins/kimi").display()
        ));
    }
    if remove {
        if plugin_detected(cfg) {
            return Ok(
                "keep plugins/managed/rtok (owned by Kimi; remove with `/plugins remove rtok`)"
                    .into(),
            );
        }
        return Ok(NO_CHANGES.into());
    }
    if !a.yes {
        return Ok(NO_CHANGES.into());
    }
    Ok(format!(
        "offer plugins/kimi → /plugins install {} {KETCH_INSTALL}",
        super::plugin_src("plugins/kimi").display()
    ))
}

/// Apply, dry-run, or remove the `[[hooks]]` tables.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.kimi.config_path;
    let mut doc = load(path)?;
    let report = if remove {
        strip_ours(&mut doc)
    } else {
        insert_ours(&mut doc, cfg.setup.hook_timeout_s)?
    };
    rtok_agent_sdk::write(&apply(cfg), path, &doc.to_string(), &report)?;
    Ok(report)
}

/// `mcpServers.rtok = {command, args}` in `mcp.json` — Kimi's documented shape carries no `type`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    let entry = json!({"command": cmd, "args": ["mcp"]});
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &mcp_path(cfg),
        "mcpServers",
        NAME,
        entry,
        &format!("{cmd} mcp"),
    )
}

/// Drop `mcpServers.rtok` from `mcp.json`.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_server(&apply(cfg), &mcp_path(cfg), "mcpServers", NAME)
}

/// An absent file is an empty document; an unreadable one is an error (never overwrite a
/// config that was not read).
fn load(path: &Path) -> Result<DocumentMut> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| path.display().to_string()),
    };
    raw.parse::<DocumentMut>()
        .with_context(|| path.display().to_string())
}

fn table_is_ours(t: &Table, event: &str, matcher: &str) -> bool {
    t.get("event").and_then(Item::as_str) == Some(event)
        && t.get("matcher").and_then(Item::as_str).unwrap_or("") == matcher
        && t.get("command")
            .and_then(Item::as_str)
            .is_some_and(|c| is_ours(c, event))
}

fn insert_ours(doc: &mut DocumentMut, timeout: u64) -> Result<String> {
    let hooks = doc
        .entry("hooks")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
    let Some(hooks) = hooks.as_array_of_tables_mut() else {
        bail!("`hooks` is not an array of tables");
    };
    let bin = super::rtok_hook_bin();
    let mut added = Vec::new();
    for &(event, matcher) in ENTRIES {
        if hooks.iter().any(|t| table_is_ours(t, event, matcher)) {
            continue;
        }
        let cmd = format!("{bin} hook {event}");
        let mut t = Table::new();
        t["event"] = value(event);
        if !matcher.is_empty() {
            t["matcher"] = value(matcher);
        }
        t["command"] = value(cmd.as_str());
        t["timeout"] = value(timeout as i64);
        hooks.push(t);
        let m = if matcher.is_empty() {
            String::new()
        } else {
            format!(" {matcher}")
        };
        added.push(format!("+ [[hooks]] {event}{m} {cmd}"));
    }
    if added.is_empty() {
        Ok(NO_CHANGES.into())
    } else {
        Ok(format!("{}\n{} additions", added.join("\n"), added.len()))
    }
}

fn strip_ours(doc: &mut DocumentMut) -> String {
    let Some(hooks) = doc.get_mut("hooks").and_then(Item::as_array_of_tables_mut) else {
        return NO_CHANGES.into();
    };
    let before = hooks.len();
    hooks.retain(|t| {
        let event = t.get("event").and_then(Item::as_str).unwrap_or("");
        !t.get("command")
            .and_then(Item::as_str)
            .is_some_and(|c| is_ours(c, event))
    });
    let removed = before - hooks.len();
    if hooks.is_empty() {
        doc.remove("hooks");
    }
    if removed == 0 {
        NO_CHANGES.into()
    } else {
        format!("{removed} removed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-kimi-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let mut c = Config::default();
        c.setup.kimi.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    /// Shared fixture (T86 tests live in `t86_tests` below and reuse it).

    #[test]
    fn dry_run_names_nine_tables_and_touches_nothing() {
        let (c, path) = cfg("dry", true);
        fs::write(&path, "# mine\n").unwrap();
        let out = run(&c, false).unwrap();
        assert!(out.contains("9 additions"), "{out}");
        assert!(out.contains("+ [[hooks]] PreToolUse Bash "), "{out}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "# mine\n");
        assert!(!mcp_path(&c).exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_keeps_comments_and_foreign_hooks_and_is_idempotent() {
        let (c, path) = cfg("apply", false);
        fs::write(
            &path,
            "# kimi config\nmodel = \"k2\"\n\n[[hooks]]\nevent = \"Stop\"\ncommand = \"echo other\"\n",
        )
        .unwrap();
        assert!(run(&c, false).unwrap().contains("9 additions"));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("# kimi config\nmodel = \"k2\"\n"), "{raw}");
        assert!(raw.contains("command = \"echo other\""), "{raw}");
        let doc: DocumentMut = raw.parse().unwrap();
        let hooks = doc["hooks"].as_array_of_tables().unwrap();
        assert_eq!(hooks.len(), 10);
        let bash = hooks
            .iter()
            .find(|t| t.get("matcher").and_then(Item::as_str) == Some("Bash"))
            .unwrap();
        assert_eq!(bash["event"].as_str(), Some("PreToolUse"));
        assert_eq!(bash["timeout"].as_integer(), Some(5));
        assert!(
            bash["command"]
                .as_str()
                .unwrap()
                .ends_with("rtok hook PreToolUse")
        );
        assert_eq!(Kimi.installed(&c, Kind::Cli), ["hooks"]);

        assert_eq!(run(&c, true).unwrap(), "9 removed");
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok"), "{raw}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    // --- Vfs twins (T56.3): TOML hook insert/strip in memory; keep disk e2e ---

    fn hooks_roundtrip_vfs(
        vfs: &mut crate::testutil::Vfs,
        path: &str,
        remove: bool,
        timeout: u64,
    ) -> String {
        let raw = vfs.read_str(path).unwrap_or("");
        let mut doc: DocumentMut = raw.parse().unwrap_or_default();
        let report = if remove {
            strip_ours(&mut doc)
        } else {
            insert_ours(&mut doc, timeout).unwrap()
        };
        vfs.write(path, doc.to_string());
        report
    }

    #[test]
    fn dry_run_names_nine_tables_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        let path = "config.toml";
        vfs.write(path, "# mine\n");
        let before = vfs.read_str(path).unwrap().to_string();
        let out = {
            // Report-only: parse + insert without writing back (dry twin).
            let mut doc: DocumentMut = before.parse().unwrap();
            insert_ours(&mut doc, 5).unwrap()
        };
        assert!(out.contains("9 additions"), "{out}");
        assert!(out.contains("+ [[hooks]] PreToolUse Bash "), "{out}");
        assert_eq!(vfs.read_str(path).unwrap(), before);
    }

    #[test]
    fn apply_keeps_comments_and_foreign_hooks_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        let path = "config.toml";
        vfs.write(
            path,
            "# kimi config\nmodel = \"k2\"\n\n[[hooks]]\nevent = \"Stop\"\ncommand = \"echo other\"\n",
        );
        assert!(hooks_roundtrip_vfs(&mut vfs, path, false, 5).contains("9 additions"));
        assert_eq!(hooks_roundtrip_vfs(&mut vfs, path, false, 5), NO_CHANGES);
        let raw = vfs.read_str(path).unwrap();
        assert!(raw.starts_with("# kimi config\nmodel = \"k2\"\n"), "{raw}");
        assert!(raw.contains("command = \"echo other\""), "{raw}");
        let doc: DocumentMut = raw.parse().unwrap();
        let hooks = doc["hooks"].as_array_of_tables().unwrap();
        assert_eq!(hooks.len(), 10);
        assert_eq!(hooks_roundtrip_vfs(&mut vfs, path, true, 5), "9 removed");
        assert_eq!(hooks_roundtrip_vfs(&mut vfs, path, true, 5), NO_CHANGES);
        let raw = vfs.read_str(path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok"), "{raw}");
    }

    #[test]
    fn spaced_profile_config_path_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        let path = "Users/Ivan Tuhai/.kimi-code/config.toml";
        vfs.write(path, "# mine\n");
        let out = hooks_roundtrip_vfs(&mut vfs, path, false, 5);
        assert!(out.contains("9 additions"), "{out}");
        assert!(vfs.read_str(path).unwrap().contains("rtok hook"));
    }

    #[test]
    fn mcp_lands_beside_the_config_without_a_type_field() {
        let (c, path) = cfg("mcp", false);
        let first = register_mcp(&c).unwrap();
        assert!(first.starts_with("mcpServers.rtok: "), "{first}");
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        assert_eq!(mcp_path(&c), path.parent().unwrap().join("mcp.json"));
        let raw = fs::read_to_string(mcp_path(&c)).unwrap();
        assert!(!raw.contains("\"type\""), "{raw}");
        assert!(raw.contains("\"mcp\""), "{raw}");
        assert_eq!(Kimi.installed(&c, Kind::Cli), ["mcp"]);
        assert_eq!(unregister_mcp(&c).unwrap(), "- mcpServers.rtok");
        assert!(!fs::read_to_string(mcp_path(&c)).unwrap().contains("rtok"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// `plugins/kimi/kimi.plugin.json` (T85) is the installer's hooks and MCP in Kimi's plugin
    /// shape: same entries in the same order, same default timeout, one server. A new `ENTRIES`
    /// row fails here until the manifest follows.
    #[test]
    fn plugin_manifest_matches_the_installer() {
        let m: serde_json::Value =
            serde_json::from_str(include_str!("../../../plugins/kimi/kimi.plugin.json")).unwrap();
        assert_eq!(m["name"], NAME);
        let timeout = Config::default().setup.hook_timeout_s;
        let want: Vec<_> = ENTRIES
            .iter()
            .map(|&(event, matcher)| {
                let mut h = json!({"event": event, "command": format!("rtok hook {event}"), "timeout": timeout});
                if !matcher.is_empty() {
                    h["matcher"] = json!(matcher);
                }
                h
            })
            .collect();
        assert_eq!(m["hooks"], json!(want));
        for h in m["hooks"].as_array().unwrap() {
            let (cmd, event) = (h["command"].as_str().unwrap(), h["event"].as_str().unwrap());
            assert!(is_ours(cmd, event), "{cmd}");
        }
        assert_eq!(
            m["mcpServers"],
            json!({NAME: {"command": "rtok", "args": ["mcp"]}})
        );
    }

    /// T86: the offer names `plugins/kimi` and `/plugins install` (dry-run and
    /// apply alike) and writes nothing; Kimi's store is never touched by rtok.
    #[test]
    fn offer_names_the_plugin_path_and_ketch_hint() {
        let (mut c, dir) = cfg("offer-dry", true);
        // Without --yes the offer stays declined: NO_CHANGES, nothing written.
        assert_eq!(offer_plugin(&c, false).unwrap(), NO_CHANGES);
        assert!(!dir.join("plugins").exists(), "dry-run writes nothing");
        c.setup.yes = true;
        let line = offer_plugin(&c, false).unwrap();
        assert!(line.contains("plugins/kimi"), "{line}");
        assert!(line.contains("/plugins install"), "{line}");
        assert!(line.contains("ketch install listepo/rtok"), "{line}");
        assert!(!dir.join("plugins").exists(), "dry-run writes nothing");
        c.setup.dry_run = false;
        let line = offer_plugin(&c, false).unwrap();
        assert!(line.contains("plugins/kimi"), "{line}");
        assert!(line.contains("/plugins install"), "{line}");
        assert!(!dir.join("plugins").exists(), "apply writes nothing either");
        let _ = fs::remove_dir_all(dir);
    }

    /// T86 D21 singleton: with a seeded `managed/rtok/kimi.plugin.json` a second
    /// install removes the nine tables and `mcpServers.rtok` and reports `plugin`;
    /// remove leaves the managed copy alone with its own remove line.
    /// (`--yes` accepted: the `Flag("--yes")` offer reads back as installed,
    /// like every other flag module.)
    #[test]
    fn plugin_detected_strips_own_tables_and_reports_plugin() {
        let (mut c, dir) = cfg("singleton", false);
        c.setup.yes = true;
        // Plain install first: hooks + MCP land in the user files (`plugin`
        // reads back too — the flag accepted the offer).
        let first = Kimi.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert_eq!(Kimi.installed(&c, Kind::Cli), ["hooks", "mcp", "plugin"]);
        assert!(first[0].contains("/plugins install"), "{first:?}");
        // Kimi installs the plugin: seed the managed copy it would write.
        let marker = plugin_marker(&c);
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, "{}").unwrap();
        let second = Kimi.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert_eq!(Kimi.installed(&c, Kind::Cli), ["plugin"]);
        assert!(!super::super::read(&c.setup.kimi.config_path).contains("rtok hook"));
        assert!(!super::super::read(&mcp_path(&c)).contains("\"rtok\""));
        assert!(second[0].contains("/plugins install"), "{second:?}");
        // Remove: own tables already gone, managed copy kept with its line.
        let rm = Kimi.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        assert!(rm[0].contains("/plugins remove rtok"), "{rm:?}");
        assert!(marker.is_file(), "remove leaves the managed copy alone");
        assert_eq!(Kimi.installed(&c, Kind::Cli), ["plugin"]);
        let _ = fs::remove_dir_all(dir);
    }

    /// T86: remove on a clean home prints `NO_CHANGES` for the plugin line.
    #[test]
    fn remove_without_plugin_is_no_changes() {
        let (c, dir) = cfg("rm-clean", false);
        let rm = Kimi.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        assert!(rm.iter().all(|l| l == NO_CHANGES), "{rm:?}");
        let _ = fs::remove_dir_all(dir);
    }
}
