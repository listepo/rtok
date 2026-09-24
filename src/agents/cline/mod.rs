//! Cline installer (`rtok agents install cline`, plan T96.1).
//!
//! Cline's CLI and VS Code extension both scan `~/Documents/Cline/Hooks`, so one
//! directory serves both surfaces (D21 singleton): per event T96 links
//! `plugins/cline/hooks/rtok-hook` as `<hooks_path>/<Event>` through one
//! [`HostPlugin`] per event. MCP is per surface: the CLI path
//! (`[setup.cline] mcp_path`) and the VS Code extension globalStorage file
//! (resolved from the `vscode` host user dir); both carry `mcpServers.rtok`.
//! The hook links ARE the plugin unit (T95) — `installed()` reports `plugin`
//! exactly when `hooks` is installed, and `apply` never links twice.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::json;

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

/// Cline: the `cline` CLI and the `saoudrizwan.claude-dev` VS Code extension.
pub struct Cline;

/// Cline MCP server name in `cline_mcp_settings.json`.
const NAME: &str = "rtok";

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Cline CLI",
        bins: &["cline"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Cline for VS Code",
        bins: &[],
        apps: &["/Applications/Visual Studio Code.app"],
    },
];

/// Event file names the installer links the hook as (T95 test pins the same set).
pub const EVENTS: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "TaskStart",
    "UserPromptSubmit",
    "SessionEnd",
];

/// One hook link per event (T95: the event is the file name). `dest` cannot vary
/// per event in a `static`, so each event gets its own descriptor.
macro_rules! hook {
    ($name:ident, $event:expr) => {
        /// Link `plugins/cline/hooks/rtok-hook` as `<hooks_path>/<Event>`.
        pub static $name: HostPlugin = HostPlugin {
            src_rel: "plugins/cline/hooks/rtok-hook",
            host: "Cline",
            label: None,
            dest: $event,
            // Out of scope for T164: Cline is not one of the five local-link-only
            // hosts, so a hook link still asks with `--yes` like omp's plugin link.
            default_install: false,
        };
    };
}

fn dest_pre(cfg: &Config) -> PathBuf {
    cfg.setup.cline.hooks_path.join("PreToolUse")
}
fn dest_post(cfg: &Config) -> PathBuf {
    cfg.setup.cline.hooks_path.join("PostToolUse")
}
fn dest_start(cfg: &Config) -> PathBuf {
    cfg.setup.cline.hooks_path.join("TaskStart")
}
fn dest_prompt(cfg: &Config) -> PathBuf {
    cfg.setup.cline.hooks_path.join("UserPromptSubmit")
}
fn dest_end(cfg: &Config) -> PathBuf {
    cfg.setup.cline.hooks_path.join("SessionEnd")
}
hook!(HOOK_PRE, dest_pre);
hook!(HOOK_POST, dest_post);
hook!(HOOK_START, dest_start);
hook!(HOOK_PROMPT, dest_prompt);
hook!(HOOK_END, dest_end);

/// Every per-event hook descriptor, in link order.
pub const HOOKS: [&HostPlugin; 5] = [&HOOK_PRE, &HOOK_POST, &HOOK_START, &HOOK_PROMPT, &HOOK_END];

impl Agent for Cline {
    fn id(&self) -> &'static str {
        "cline"
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
            "hooks" => Support::Yes,
            "mcp" => Support::Yes,
            // The hook links ARE the plugin unit (T95): no second link step —
            // `installed()` reports `plugin` exactly when `hooks` is installed,
            // and the flag is what `expected()` waits for before calling it done.
            "plugin" => Support::Flag("--yes"),
            _ => Support::No("the Anthropic base URL is extension state, not a file rtok may edit"),
        }
    }
    fn files(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        // `hooks_path` is a directory of per-event links, not a file `run()` can back up
        // (T164/T75 pattern: pi's `extensions_path` is excluded the same way) — only the
        // two MCP settings files are real files an install writes. Each is its own surface
        // (the CLI and the VS Code extension are different processes with different
        // config), so a variant's block reports only the file that variant actually reads —
        // `apply` still writes both every time (`shared`), but `files` never claims the
        // other surface's file as this one's.
        match kind {
            Kind::Cli => vec![cfg.setup.cline.mcp_path.clone()],
            Kind::Desktop => vec![ext_mcp_path(cfg)],
        }
    }
    fn markers(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        let mut paths = self.files(cfg, kind);
        paths.push(cfg.setup.cline.hooks_path.clone());
        paths
    }
    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        // Hooks (and with them the plugin unit): every per-event link is ours.
        if HOOKS.iter().all(|h| h.ours(cfg)) {
            out.push("hooks");
            out.push("plugin");
        }
        // MCP: either surface carrying `rtok` counts (per-surface files, one key).
        if super::read(&cfg.setup.cline.mcp_path).contains("\"rtok\"")
            || super::read(&ext_mcp_path(cfg)).contains("\"rtok\"")
        {
            out.push("mcp");
        }
        out
    }
    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines: Vec<String> = HOOKS
            .iter()
            .map(|h| h.offer(cfg, remove))
            .collect::<Result<_>>()?;
        if remove {
            lines.push(unregister_mcp(cfg, &cfg.setup.cline.mcp_path)?);
            lines.push(unregister_mcp(cfg, &ext_mcp_path(cfg))?);
        } else if cfg.setup.mcp {
            lines.push(register_mcp(cfg, &cfg.setup.cline.mcp_path)?);
            lines.push(register_mcp(cfg, &ext_mcp_path(cfg))?);
        }
        Ok(lines)
    }
}

/// VS Code extension MCP file:
/// `<user-dir>/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`,
/// where `<user-dir>` is the `vscode` host's stable profile dir (config override or OS default).
pub fn ext_mcp_path(cfg: &Config) -> PathBuf {
    let user_dir = if cfg.setup.vscode.code_user_dir.as_os_str().is_empty() {
        super::vscode::default_user_dir(false)
    } else {
        cfg.setup.vscode.code_user_dir.clone()
    };
    user_dir.join("globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json")
}

/// The `mcpServers.rtok` entry [`register_mcp`] writes.
fn mcp_entry(cmd: &str) -> serde_json::Value {
    json!({"command": cmd, "args": ["mcp"]})
}

/// `mcpServers.rtok = {command, args}` in a `cline_mcp_settings.json` — Cline's
/// documented shape carries no `type` (same call shape as kimi/omp).
pub fn register_mcp(cfg: &Config, path: &Path) -> Result<String> {
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_server(
        &apply(cfg),
        path,
        "mcpServers",
        NAME,
        mcp_entry(&cmd),
        &format!("{cmd} mcp"),
    )
}

/// Drop `mcpServers.rtok` from a `cline_mcp_settings.json`, unless the user edited
/// it (T246.2) — only what rtok wrote goes, and foreign servers are left alone.
pub fn unregister_mcp(cfg: &Config, path: &Path) -> Result<String> {
    super::unregister_ours(cfg, path, "mcpServers", NAME, &mcp_entry("rtok"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_agent_sdk::NO_CHANGES;
    use std::fs;

    fn cfg(name: &str, dry: bool, yes: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-cline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.setup.cline.hooks_path = dir.join("Hooks");
        c.setup.cline.mcp_path = dir.join("cline_mcp_settings.json");
        c.setup.dry_run = dry;
        c.setup.yes = yes;
        c.setup.backup = false;
        c.setup.mcp = false;
        c.setup.vscode.code_user_dir = dir.join("User");
        (c, dir)
    }

    #[test]
    fn dry_run_names_the_plugin_and_ketch_and_writes_nothing() {
        let (c, dir) = cfg("dry", true, false);
        let lines = Cline.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert_eq!(lines.len(), EVENTS.len(), "{lines:?}");
        for line in &lines {
            assert!(line.contains("plugins/cline"), "{lines:?}");
            assert!(line.contains("ketch install listepo/rtok"), "{lines:?}");
        }
        assert!(!c.setup.cline.hooks_path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yes_links_every_event_second_apply_is_no_changes() {
        let (c, dir) = cfg("yes", false, true);
        let first = Cline.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(first.iter().all(|l| l.starts_with("+ plugin")), "{first:?}");
        assert_eq!(Cline.installed(&c, Kind::Cli), ["hooks", "plugin"]);
        let again = Cline.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(again.iter().all(|l| l == NO_CHANGES), "{again:?}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn remove_takes_back_only_ours_and_keeps_foreign() {
        let (c, dir) = cfg("rm", false, true);
        // A foreign *directory* at an event slot: the SDK never wipes what it did
        // not put there (a plain foreign file counts as a link target and unlinks).
        let foreign = c.setup.cline.hooks_path.join("PreToolUse");
        fs::create_dir_all(&c.setup.cline.hooks_path).unwrap();
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("mine.sh"), "mine").unwrap();
        Cline.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(foreign.is_dir());
        assert_eq!(fs::read_to_string(foreign.join("mine.sh")).unwrap(), "mine");
        let lines = Cline.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.contains("leave") && l.contains("PreToolUse")),
            "{lines:?}"
        );
        assert_eq!(fs::read_to_string(foreign.join("mine.sh")).unwrap(), "mine");
        assert!(Cline.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
