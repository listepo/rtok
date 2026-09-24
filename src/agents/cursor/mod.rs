//! Cursor installer (`rtok agents install cursor`) and field mapping (plan T10.1).
//!
//! Cursor shell stdin uses top-level `command` and `conversation_id`.
//! `beforeShellExecution` → PreToolUse; `afterShellExecution` → PostToolUse
//! (`output`/`stdout` → `tool_response`) so guard/read caches populate.
//! [`crate::hooks::types::HookInput::adapt_cursor`] performs that map when
//! `[hook] host` is `cursor` (also `--host cursor`).
//! Cursor `hooks.json` is `{version, hooks.before|afterShellExecution[].command}`.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, array_at, edit_json, object_at};
use serde_json::{Value, json};

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

/// Cursor: shell hooks in `hooks.json`, MCP in `mcp.json` or through the linked plugin.
/// The CLI (`cursor-agent`) and the desktop app read the same `~/.cursor` files.
pub struct Cursor;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Cursor CLI",
        bins: &["cursor-agent", "agent"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Cursor",
        bins: &["cursor"],
        apps: &[
            "/Applications/Cursor.app",
            "$LOCALAPPDATA/Programs/cursor/Cursor.exe",
            "/opt/Cursor",
        ],
    },
];

impl Agent for Cursor {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
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

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "hooks" | "mcp" | "plugin" => Support::Yes,
            _ => Support::No("Cursor has no base-URL setting to point at the proxy"),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.cursor.hooks_path.clone(), mcp_path(cfg)]
    }

    fn markers(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        let mut paths = self.files(cfg, kind);
        paths.push(plugin_dest(cfg));
        paths
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let h = super::read(&cfg.setup.cursor.hooks_path);
        let m = super::read(&mcp_path(cfg));
        // T75: `ours`, not any metadata — a foreign directory at the plugin dest is not
        // an rtok install, so it cannot keep the green mark alive after an uninstall
        // that (rightly) left it alone.
        let plugin = PLUGIN.ours(cfg);
        let mut out = Vec::new();
        // The linked plugin serves the hooks too (D21), and setup then strips `hooks.json`.
        if h.contains("rtok hook") || plugin {
            out.push("hooks");
        }
        // The linked plugin serves the MCP itself (D21), and setup then skips `mcp.json`.
        if m.contains("\"rtok\"") || plugin {
            out.push("mcp");
        }
        if plugin {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        // Plugin first: once linked it carries the same hook events, so `hooks.json` keeps
        // none of ours or every shell command would fire rtok twice (D21, T244).
        let plugin = offer_plugin(cfg, remove)?;
        let mut lines = vec![run(cfg, remove || PLUGIN.ours(cfg))?, plugin];
        if remove {
            lines.push(unregister_mcp(cfg)?);
        } else if cfg.setup.mcp && !plugin_is_mcp(cfg, remove) {
            lines.push(register_mcp(cfg)?);
        }
        Ok(lines)
    }
}

fn pre_cmd() -> String {
    format!("{} hook PreToolUse --host cursor", super::rtok_hook_bin())
}

fn post_cmd() -> String {
    format!("{} hook PostToolUse --host cursor", super::rtok_hook_bin())
}

fn compact_cmd() -> String {
    format!("{} hook PreCompact --host cursor", super::rtok_hook_bin())
}

/// Apply, dry-run, or remove Cursor before/after shell hook entries.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    edit_json(&apply(cfg), &cfg.setup.cursor.hooks_path, |root| {
        if remove {
            strip_ours(root)
        } else {
            insert_ours(root)
        }
    })
}

/// `~/.cursor/mcp.json` — the sibling of `hooks.json`.
fn mcp_path(cfg: &Config) -> PathBuf {
    cfg.setup.cursor.hooks_path.with_file_name("mcp.json")
}

/// Register `rtok mcp` in `~/.cursor/mcp.json` (sibling of `hooks.json`).
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_mcp(&apply(cfg), &mcp_path(cfg), "rtok", &cmd, &["mcp"])
}

/// Drop `mcpServers.rtok` from `~/.cursor/mcp.json` (`rtok agents remove cursor`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    super::unregister_mcp_ours(cfg, &mcp_path(cfg), "rtok")
}

/// The linked Cursor plugin (D21, T10.5). Dry-run and the unaccepted offer MUST contain the
/// substrings `plugins/cursor` and `~/.cursor/plugins/local` and `ketch install listepo/rtok`.
pub static PLUGIN: HostPlugin = HostPlugin {
    src_rel: "plugins/cursor",
    host: "Cursor",
    label: Some("~/.cursor/plugins/local"),
    dest: plugin_dest,
    // Cursor's own docs have no GitHub/subdir plugin install; the local link is the only
    // path there is, so it installs by default once Cursor itself is detected (T164).
    default_install: true,
};

/// Local Cursor plugin dest: sibling of hooks.json → `<cursor-dir>/plugins/local/rtok`.
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup
        .cursor
        .hooks_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("plugins")
        .join("local")
        .join("rtok")
}

/// [`PLUGIN`]'s offer plus Cursor's singleton rule: the plugin *is* the MCP, so a previous
/// `mcpServers.rtok` entry from a plain install must go, else two writers serve one store.
/// Cleared on every run while the plugin is ours — not only on the first `+ plugin` — so a
/// leftover from a declined earlier offer is not kept. Keyed on [`HostPlugin::ours`], not
/// `linked`: a foreign directory at the dest is "linked" too, and unregistering the plain
/// MCP entry for it would strip a working install for nothing (T196).
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    let report = PLUGIN.offer(cfg, remove)?;
    if !remove && PLUGIN.ours(cfg) {
        let cleared = unregister_mcp(cfg)?;
        if cleared != NO_CHANGES {
            return Ok(format!("{report}\n{cleared}"));
        }
    }
    Ok(report)
}

/// True when the Cursor plugin is ours: it *is* the MCP (D21 singleton), so
/// setup must not also register `mcpServers.rtok` in `mcp.json`. A foreign directory at
/// the dest is not ours (T196): `plugin_is_mcp` must stay false so the plain `mcp.json`
/// install still runs instead of being silently suppressed.
///
/// Judged only by the link, not `--yes`: a dry-run with `--yes` has not linked
/// yet and must still show what `mcp.json` would do if the offer is declined.
pub fn plugin_is_mcp(cfg: &Config, remove: bool) -> bool {
    !remove && PLUGIN.ours(cfg)
}

fn insert_ours(root: &mut Value) -> String {
    let hooks = object_at(root, "hooks");
    let mut added = Vec::new();
    let pre = pre_cmd();
    let post = post_cmd();
    let compact = compact_cmd();
    for (event, cmd) in [
        ("beforeShellExecution", pre.as_str()),
        ("afterShellExecution", post.as_str()),
        ("preCompact", compact.as_str()),
    ] {
        let arr = array_at(hooks, event);
        // T242.5: an rtok hook written by another binary path is rewritten in its slot.
        let mut found = false;
        for e in arr.iter_mut().filter(|e| is_ours(e)) {
            found = true;
            if !is_cmd(e, cmd) {
                e["command"] = json!(cmd);
                added.push(format!("~ {event} {cmd}"));
            }
        }
        if !found {
            arr.push(json!({"command": cmd}));
            added.push(format!("+ {event} {cmd}"));
        }
    }
    root.as_object_mut()
        .expect("object_at made it an object")
        .entry("version")
        .or_insert(json!(1));
    if added.is_empty() {
        NO_CHANGES.into()
    } else {
        added.join("\n")
    }
}

fn strip_ours(root: &mut Value) -> String {
    let mut removed = Vec::new();
    for event in ["beforeShellExecution", "afterShellExecution", "preCompact"] {
        let Some(arr) = root
            .pointer_mut(&format!("/hooks/{event}"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = arr.len();
        arr.retain(|e| !is_ours(e));
        if arr.len() != before {
            removed.push(format!("- {event}"));
        }
    }
    if removed.is_empty() {
        NO_CHANGES.into()
    } else {
        removed.join("\n")
    }
}

fn is_cmd(entry: &Value, cmd: &str) -> bool {
    entry.get("command").and_then(Value::as_str) == Some(cmd)
}

fn is_ours(entry: &Value) -> bool {
    let Some(cmd) = entry.get("command").and_then(Value::as_str) else {
        return false;
    };
    // The events `insert_ours` writes (`beforeShellExecution` etc. are the
    // Cursor-side names; these are the `rtok hook <event>` spellings).
    for event in ["PreToolUse", "PostToolUse", "PreCompact"] {
        let suffix = format!(" hook {event} --host cursor");
        if let Some(bin) = cmd.strip_suffix(&suffix)
            && super::is_rtok_bin(super::unquote_bin(bin))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{dispatch, types::HookInput};
    use crate::plugin::Runtime;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-cursor-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cfg(hooks: PathBuf, dry: bool) -> Config {
        let mut c = Config::default();
        c.setup.cursor.hooks_path = hooks;
        c.setup.dry_run = dry;
        c.setup.backup = false;
        c
    }

    #[test]
    fn dry_run_offer_names_plugin_and_local() {
        let dir = tmp("offer-dry");
        let path = dir.join("hooks.json");
        let c = cfg(path, true);
        let s = offer_plugin(&c, false).unwrap();
        assert!(s.contains("plugins/cursor"), "{s}");
        assert!(s.contains("~/.cursor/plugins/local"), "{s}");
        assert!(s.contains("ketch install listepo/rtok"), "{s}");
        assert!(!plugin_dest(&c).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yes_links_plugin_second_apply_no_changes() {
        let dir = tmp("offer-yes");
        let mut c = cfg(dir.join("hooks.json"), false);
        c.setup.yes = true;
        c.setup.backup = false;
        let first = offer_plugin(&c, false).unwrap();
        assert!(first.starts_with("+ plugin"), "{first}");
        assert!(PLUGIN.linked(&c));
        assert_eq!(offer_plugin(&c, false).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn linked_plugin_clears_leftover_mcp_json_on_later_setup() {
        let dir = tmp("singleton-clean");
        let mut c = cfg(dir.join("hooks.json"), false);
        c.setup.yes = true;
        c.setup.backup = false;
        // Simulate: earlier declined plugin left mcp.json; plugin linked later.
        let mcp = dir.join("mcp.json");
        fs::write(
            &mcp,
            r#"{"mcpServers":{"rtok":{"type":"stdio","command":"rtok","args":["mcp"]}}}"#,
        )
        .unwrap();
        assert!(offer_plugin(&c, false).unwrap().starts_with("+ plugin"));
        assert!(PLUGIN.linked(&c));
        let body = fs::read_to_string(&mcp).unwrap();
        assert!(
            !body.contains("\"rtok\""),
            "fresh link must drop mcpServers.rtok: {body}"
        );
        // Re-seed a leftover while the plugin stays linked (manual re-add, or an
        // older setup that only cleaned on `+ plugin`).
        fs::write(
            &mcp,
            r#"{"mcpServers":{"rtok":{"type":"stdio","command":"rtok","args":["mcp"]},"other":{"command":"x"}}}"#,
        )
        .unwrap();
        // T196: the plugin offer itself is a no-op (already linked, up to date), but the
        // cleared leftover must be reported, not silently discarded.
        assert_eq!(
            offer_plugin(&c, false).unwrap(),
            format!("{NO_CHANGES}\n- mcpServers.rtok")
        );
        let body = fs::read_to_string(&mcp).unwrap();
        assert!(
            !body.contains("\"rtok\""),
            "already-linked setup must still clear leftover rtok: {body}"
        );
        assert!(body.contains("other"), "foreign servers must stay: {body}");
        assert!(plugin_is_mcp(&c, false));
        let _ = fs::remove_dir_all(dir);
    }

    /// T196: `plugin_is_mcp`/`offer_plugin` keyed on `linked()` treated a foreign directory
    /// at the plugin dest (rightly refused by `PluginLink::run`) as though rtok's plugin were
    /// serving MCP — wiping a working plain install's `mcpServers.rtok` and then suppressing
    /// `register_mcp`, so `agents install cursor` deleted the MCP entry and installed nothing.
    #[test]
    fn foreign_plugin_dir_keeps_plain_mcp_working() {
        let dir = tmp("foreign");
        let mut c = cfg(dir.join("hooks.json"), false);
        c.setup.yes = true;
        c.setup.backup = false;
        // A foreign directory already sits at the plugin dest: not an rtok link, no owned
        // marker, different bytes from the shipped plugin.
        let dest = plugin_dest(&c);
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("mine.txt"), "not rtok's plugin").unwrap();
        // Seed a working plain install: hooks + mcpServers.rtok already present.
        run(&c, false).unwrap();
        register_mcp(&c).unwrap();
        assert!(PLUGIN.linked(&c), "the foreign dir makes linked() true");
        assert!(!PLUGIN.ours(&c), "but it is not ours");

        let lines = Cursor
            .apply(&c, Kind::Desktop, Mode::Install)
            .unwrap()
            .join("\n");
        // The offer is declined (foreign dir refused), not silently treated as linked.
        assert!(lines.contains("accept with --yes"), "{lines}");
        assert_eq!(
            fs::read_to_string(dest.join("mine.txt")).unwrap(),
            "not rtok's plugin",
            "foreign dir must stay untouched"
        );
        let hooks_raw = fs::read_to_string(&c.setup.cursor.hooks_path).unwrap();
        assert!(hooks_raw.contains(&pre_cmd()), "{hooks_raw}");
        let mcp_raw = fs::read_to_string(mcp_path(&c)).unwrap();
        assert!(
            mcp_raw.contains("\"rtok\""),
            "plain mcpServers.rtok must survive: {mcp_raw}"
        );
        assert_eq!(Cursor.installed(&c, Kind::Desktop), ["hooks", "mcp"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn plugin_is_mcp_requires_a_real_link_not_just_yes() {
        let dir = tmp("is-mcp-yes");
        let mut c = cfg(dir.join("hooks.json"), false);
        c.setup.yes = true;
        assert!(
            !plugin_is_mcp(&c, false),
            "--yes alone must not suppress mcp.json registration"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dry_run_then_apply_is_idempotent() {
        let dir = tmp("setup");
        let path = dir.join("hooks.json");
        let dry = run(&cfg(path.clone(), true), false).unwrap();
        assert!(dry.contains("beforeShellExecution"), "{dry}");
        assert!(dry.contains("afterShellExecution"), "{dry}");
        assert!(dry.contains("preCompact"), "{dry}");
        assert!(!path.exists());
        let c = cfg(path.clone(), false);
        assert!(run(&c, false).unwrap().contains(&pre_cmd()));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"version\""));
        assert!(raw.contains(&pre_cmd()));
        assert!(raw.contains(&post_cmd()));
        assert!(raw.contains(&compact_cmd()));
        assert!(raw.contains("afterShellExecution"));
        assert!(raw.contains("preCompact"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cursor_payload_wraps_command() {
        let dir = tmp("wrap");
        let mut c = Config::default();
        c.hook.host = "cursor".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        let cx = Runtime::open(c, "cur").unwrap();
        let raw = json!({
            "hook_event_name": "beforeShellExecution",
            "command": "ls -la",
            "cwd": "/tmp",
            "conversation_id": "sess-1"
        });
        let bytes = serde_json::to_vec(&raw).unwrap();
        let mut input: HookInput = serde_json::from_slice(&bytes).unwrap();
        input.adapt_cursor("PreToolUse");
        let out = dispatch(&bytes, &input, &cx);
        let v: Value = serde_json::from_slice(&out).unwrap();
        let cmd = v["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap_or("");
        assert!(cmd.contains("rtok run --") && cmd.contains("ls -la"), "{v}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cursor_after_shell_reaches_post_tool_use() {
        let dir = tmp("after");
        let mut c = Config::default();
        c.hook.host = "cursor".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        let cx = Runtime::open(c, "cur").unwrap();
        let raw = json!({
            "hook_event_name": "afterShellExecution",
            "command": "echo hi",
            "output": "hi\n",
            "cwd": "/tmp",
            "conversation_id": "sess-2"
        });
        let bytes = serde_json::to_vec(&raw).unwrap();
        let mut input: HookInput = serde_json::from_slice(&bytes).unwrap();
        input.adapt_cursor("PostToolUse");
        assert_eq!(input.hook_event_name, "PostToolUse");
        assert!(input.post_tool().is_some());
        let out = dispatch(&bytes, &input, &cx);
        // Fail open: PostToolUse may be {} when plugins add no context.
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert!(v.is_object(), "{v}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn setup_writes_after_shell_hook() {
        let dir = tmp("after-setup");
        let path = dir.join("hooks.json");
        let c = cfg(path.clone(), false);
        let report = run(&c, false).unwrap();
        assert!(report.contains("afterShellExecution"), "{report}");
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let after = root["hooks"]["afterShellExecution"].as_array().unwrap();
        assert!(after.iter().any(|e| e["command"] == post_cmd()), "{root}");
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    /// T58.2 added `preCompact` to insert/strip but not to `is_ours`, so remove
    /// left `rtok hook PreCompact --host cursor` behind and `agents list` kept
    /// reporting hooks installed (integration `cursor_remove_…` / `list_…`).
    #[test]
    fn remove_strips_every_hook_including_pre_compact() {
        let dir = tmp("remove-all");
        let path = dir.join("hooks.json");
        let c = cfg(path.clone(), false);
        run(&c, false).unwrap();
        let report = run(&c, true).unwrap();
        assert!(report.contains("- preCompact"), "{report}");
        let left = fs::read_to_string(&path).unwrap();
        assert!(!left.contains("rtok hook"), "{left}");
        assert!(
            left.contains("preCompact"),
            "foreign-safe shape stays: {left}"
        );
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pre_compact_writes_a_checkpoint_note() {
        let dir = tmp("precompact");
        let mut c = Config::default();
        c.hook.host = "cursor".into();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        let pre = serde_json::json!({
            "conversation_id": "cur-compact",
            "trigger": "auto"
        });
        let mut out = Vec::new();
        crate::hooks::run("PreCompact", pre.to_string().as_bytes(), &mut out, &c);
        assert_eq!(out, b"{}");
        let note = crate::store::Store::open(&c.core.db_path)
            .unwrap()
            .latest_note("checkpoint:cur-compact")
            .unwrap();
        assert!(note.is_some(), "preCompact must save a checkpoint");
        let _ = fs::remove_dir_all(dir);
    }

    /// T242.5: an rtok hook on another binary path is rewritten in its slot, a foreign hook
    /// beside it stays, and a second pass changes nothing.
    #[test]
    fn stale_rtok_hook_is_rewritten_in_place() {
        let mut root = json!({"hooks": {"beforeShellExecution": [
            {"command": "audit.sh"},
            {"command": "/old/store/rtok/v0.1.0/rtok hook PreToolUse --host cursor"}
        ]}});
        let out = insert_ours(&mut root);
        assert!(
            out.contains(&format!("~ beforeShellExecution {}", pre_cmd())),
            "{out}"
        );
        let arr = root["hooks"]["beforeShellExecution"].as_array().unwrap();
        assert_eq!(arr.len(), 2, "{root}");
        assert_eq!(arr[0]["command"], "audit.sh");
        assert_eq!(arr[1]["command"], pre_cmd());
        let current = root.clone();
        assert_eq!(insert_ours(&mut root), NO_CHANGES);
        assert_eq!(root, current);
    }
}
