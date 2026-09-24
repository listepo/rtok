//! CodeWhale TUI installer (`rtok agents install codewhale`, plan T185).
//!
//! `$CODEWHALE_HOME` (default `~/.codewhale`) holds `config.toml`, whose `[hooks]` table
//! carries `[[hooks.hooks]]` entries (https://github.com/Hmbown/Codewhale/blob/main/docs/HOOKS.md,
//! fetched 2026-09-24), and the sibling `mcp.json`, whose `mcpServers.<name>` shape
//! (https://github.com/Hmbown/Codewhale/blob/main/docs/MCP.md) matches every other host
//! rtok writes directly (`codewhale mcp add` is CodeWhale's own CLI path; rtok edits the
//! file, same as `kimi`/`gemini`).
//!
//! MCP first, hooks where the event map is honest (creator request 2026-09-22): of
//! CodeWhale's 15 hook events, only 3 are "steering" (can rewrite/deny/add context) and only
//! those 3 plus 7 observer events carry any stdin JSON at all — HOOKS.md is explicit that
//! `tool_call_before`, `shell_env`, `session_start`, `session_end`, `tool_call_after`,
//! `mode_change` and `on_error` "receive environment variables only, with no stdin payload".
//! rtok's hook contract is stdin-JSON-in (`src/hooks/mod.rs::dispatch_owned_strict` parses
//! stdin unconditionally); reconstructing input from env vars for `tool_call_before` (the
//! PreToolUse analogue) would mean changing that shared entry point every host relies on,
//! not a small per-host adapter, so it is left unwired — empty stdin already degrades safely
//! to `{}` today (fail-open, D1). The 7 JSON-bearing observer events (`turn_end`,
//! `subagent_spawn`, `subagent_complete`, `session_busy`, `session_idle`, `session_error`,
//! `waiting_for_user`) are pure observers — "Codewhale ignores the hook's **result**" — so a
//! plugin could run but never rewrite, deny, or add context, and their fields do not match
//! rtok's `SubagentStart`/`SessionStart` shapes. Only `message_submit` is both steering and
//! stdin-JSON-real: wired to `UserPromptSubmit` via `--host codewhale`
//! (`src/hooks/types.rs::adapt_codewhale`, `src/hooks/mod.rs::codewhale_output`), mirroring
//! T118.1's Gemini adapter.

use std::path::PathBuf;

use anyhow::{Result, bail};
use rtok_agent_sdk::NO_CHANGES;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";
/// The one steering event with real stdin JSON and a usable reply (see module docs).
const EVENT: &str = "message_submit";
const CLAUDE_EVENT: &str = "UserPromptSubmit";

/// CodeWhale: one TUI binary, no separate desktop app.
pub struct Codewhale;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "CodeWhale",
    bins: &["codewhale", "codew"],
    apps: &[],
}];

static SURFACES: [rtok_plugin_sdk::Surface; 2] = [
    rtok_plugin_sdk::Surface::Mcp,
    rtok_plugin_sdk::Surface::Hook,
];

pub fn config_path(cfg: &Config) -> PathBuf {
    cfg.setup.codewhale.dir.join("config.toml")
}

/// `mcp.json` lives beside `config.toml`, both under `$CODEWHALE_HOME`.
pub fn mcp_path(cfg: &Config) -> PathBuf {
    cfg.setup.codewhale.dir.join("mcp.json")
}

fn command(bin: &str) -> String {
    format!("{bin} hook {CLAUDE_EVENT} --host codewhale")
}

/// Exactly `<rtok-bin> hook UserPromptSubmit --host codewhale`, matching only the tail so an
/// absolute path (Windows) still counts.
fn is_ours(cmd: &str) -> bool {
    let suffix = format!(" hook {CLAUDE_EVENT} --host codewhale");
    cmd.strip_suffix(&suffix)
        .is_some_and(|bin| super::is_rtok_bin(super::unquote_bin(bin)))
}

fn table_is_ours(t: &Table) -> bool {
    t.get("event").and_then(Item::as_str) == Some(EVENT)
        && t.get("command").and_then(Item::as_str).is_some_and(is_ours)
}

/// `[hooks] enabled = true` plus one `[[hooks.hooks]]` table — the quick-start shape
/// `docs/HOOKS.md` shows. `enabled` is set only when the key is absent, so an explicit
/// `enabled = false` the user wrote stays theirs (installing a hook does not silently turn
/// every hook back on).
fn insert_ours(doc: &mut DocumentMut, timeout: u64) -> Result<String> {
    let hooks_tbl = doc.entry("hooks").or_insert(Item::Table(Table::new()));
    let Some(hooks_tbl) = hooks_tbl.as_table_mut() else {
        bail!("`hooks` is not a table");
    };
    if hooks_tbl.get("enabled").is_none() {
        hooks_tbl["enabled"] = value(true);
    }
    let arr = hooks_tbl
        .entry("hooks")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
    let Some(arr) = arr.as_array_of_tables_mut() else {
        bail!("`hooks.hooks` is not an array of tables");
    };
    if arr.iter().any(table_is_ours) {
        return Ok(NO_CHANGES.into());
    }
    let bin = super::rtok_hook_bin();
    let cmd = command(&bin);
    let mut t = Table::new();
    t["name"] = value(NAME);
    t["event"] = value(EVENT);
    t["command"] = value(cmd.as_str());
    t["timeout_secs"] = value(i64::try_from(timeout).unwrap_or(30));
    arr.push(t);
    Ok(format!("+ [[hooks.hooks]] {EVENT} {cmd}\n1 addition"))
}

fn strip_ours(doc: &mut DocumentMut) -> String {
    let Some(hooks_tbl) = doc.get_mut("hooks").and_then(Item::as_table_mut) else {
        return NO_CHANGES.into();
    };
    let Some(arr) = hooks_tbl
        .get_mut("hooks")
        .and_then(Item::as_array_of_tables_mut)
    else {
        return NO_CHANGES.into();
    };
    let before = arr.len();
    arr.retain(|t| !table_is_ours(t));
    let removed = before - arr.len();
    if arr.is_empty() {
        hooks_tbl.remove("hooks");
    }
    if removed == 0 {
        NO_CHANGES.into()
    } else {
        format!("{removed} removed")
    }
}

/// Apply, dry-run, or remove the `[[hooks.hooks]]` entry.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = config_path(cfg);
    let mut doc = super::load_toml(&path)?;
    let report = if remove {
        strip_ours(&mut doc)
    } else {
        insert_ours(&mut doc, cfg.setup.hook_timeout_s)?
    };
    rtok_agent_sdk::write(&apply(cfg), &path, &doc.to_string(), &report)?;
    Ok(report)
}

/// `mcpServers.rtok = {type: "stdio", command, args}` in `mcp.json` — the shape
/// `docs/MCP.md` shows (`env`/`disabled`/remote fields are optional and unused here).
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_mcp(&apply(cfg), &mcp_path(cfg), NAME, &cmd, &["mcp"])
}

/// Drop `mcpServers.rtok` from `mcp.json`.
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    super::unregister_mcp_ours(cfg, &mcp_path(cfg), NAME)
}

impl Agent for Codewhale {
    fn id(&self) -> &'static str {
        "codewhale"
    }

    // Support and the plugin surfaces it implies come first: the id/variants/readme
    // boilerplate every host repeats sits last, out of the way of the parts that are
    // actually specific to CodeWhale.
    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "hooks" | "mcp" => Support::Yes,
            "proxy" => Support::No(
                "CodeWhale providers are [providers.<name>] tables with their own base_url and keys (docs/CONFIGURATION.md); setup does not edit them",
            ),
            _ => Support::No(
                "CodeWhale's plugin-bundle system needs a reviewed remote source and exact environment-source references (docs/PLUGIN_BUNDLES.md); there is no local directory to link",
            ),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &SURFACES
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![config_path(cfg), mcp_path(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        if super::read(&config_path(cfg))
            .contains(&format!(" hook {CLAUDE_EVENT} --host codewhale"))
        {
            out.push("hooks");
        }
        if super::read(&mcp_path(cfg)).contains("\"rtok\"") {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        super::apply_hook_and_mcp(cfg, mode == Mode::Remove, run, register_mcp, unregister_mcp)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("rtok-codewhale-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.setup.codewhale.dir = dir.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, dir)
    }

    #[test]
    fn dry_run_names_one_addition_and_creates_nothing() {
        let (c, dir) = cfg("dry", true);
        let out = run(&c, false).unwrap();
        assert!(
            out.contains("+ [[hooks.hooks]] message_submit") && out.contains("1 addition"),
            "{out}"
        );
        assert!(!config_path(&c).exists());
        assert!(Codewhale.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_is_idempotent_and_remove_keeps_foreign_hooks_and_servers() {
        let (c, dir) = cfg("apply", false);
        fs::write(
            config_path(&c),
            "# mine\n[[hooks.hooks]]\nevent = \"turn_end\"\ncommand = \"echo hi\"\n",
        )
        .unwrap();
        fs::write(
            mcp_path(&c),
            json!({"mcpServers": {"not-ours": {"command": "uvx", "args": ["y"]}}}).to_string(),
        )
        .unwrap();

        assert!(run(&c, false).unwrap().contains("1 addition"));
        assert!(register_mcp(&c).unwrap().starts_with("mcpServers.rtok: "));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);

        let raw = fs::read_to_string(config_path(&c)).unwrap();
        assert!(raw.contains("enabled = true"), "{raw}");
        assert!(raw.contains("echo hi"), "foreign hook stays: {raw}");
        let doc: DocumentMut = raw.parse().unwrap();
        let hooks = doc["hooks"]["hooks"].as_array_of_tables().unwrap();
        assert_eq!(hooks.len(), 2);
        let ours = hooks
            .iter()
            .find(|t| t["event"].as_str() == Some(EVENT))
            .unwrap();
        assert!(
            ours["command"]
                .as_str()
                .unwrap()
                .ends_with("rtok hook UserPromptSubmit --host codewhale")
        );
        assert_eq!(ours["timeout_secs"].as_integer(), Some(5));
        assert_eq!(Codewhale.installed(&c, Kind::Cli), ["hooks", "mcp"]);

        assert_eq!(run(&c, true).unwrap(), "1 removed");
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        assert_eq!(unregister_mcp(&c).unwrap(), "- mcpServers.rtok");
        let raw = fs::read_to_string(config_path(&c)).unwrap();
        assert!(!raw.contains("rtok hook"), "{raw}");
        assert!(
            raw.contains("echo hi"),
            "foreign hook survives remove: {raw}"
        );
        let raw_mcp = fs::read_to_string(mcp_path(&c)).unwrap();
        assert!(
            raw_mcp.contains("\"not-ours\""),
            "foreign server survives: {raw_mcp}"
        );
        assert!(!raw_mcp.contains("\"rtok\""));
        assert!(Codewhale.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn install_does_not_flip_an_explicit_enabled_false() {
        let (c, dir) = cfg("disabled", false);
        fs::write(config_path(&c), "[hooks]\nenabled = false\n").unwrap();
        run(&c, false).unwrap();
        let raw = fs::read_to_string(config_path(&c)).unwrap();
        assert!(raw.contains("enabled = false"), "{raw}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn support_matches_hooks_mcp_yes_proxy_and_plugin_no() {
        assert!(matches!(
            Codewhale.support(Kind::Cli, "hooks"),
            Support::Yes
        ));
        assert!(matches!(Codewhale.support(Kind::Cli, "mcp"), Support::Yes));
        assert!(matches!(
            Codewhale.support(Kind::Cli, "proxy"),
            Support::No(_)
        ));
        assert!(matches!(
            Codewhale.support(Kind::Cli, "plugin"),
            Support::No(_)
        ));
    }
}
