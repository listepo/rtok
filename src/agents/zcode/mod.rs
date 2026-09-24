//! ZCode installer (`rtok agents install zcode`, plan T46.1).
//!
//! Z.ai's ZCode desktop app reads a Claude-compatible hook protocol from
//! `~/.zcode/cli/config.json` — `hooks.enabled`, `hooks.events.<Event>[]` with `timeoutMs` —
//! and MCP from `mcp.servers.<name> = {command, args}`. The hook entries are Claude's helpers;
//! only the shape around them differs. The app starts without a shell PATH, so both carry the
//! absolute `rtok` binary. With `--yes` the linked plugin (T77) carries both instead: ZCode
//! loads inline plugin roots listed in `plugins.dirs`, enabled by default.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, array_at, edit_json, object_at};
use serde_json::{Value, json};

use super::claude::{desktop_command, insert_ours, strip_ours};
use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

/// ZCode: hooks and MCP in one `config.json`; a desktop app only.
pub struct Zcode;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Desktop,
    name: "ZCode",
    bins: &[],
    apps: &[
        "/Applications/ZCode.app",
        "$LOCALAPPDATA/Programs/ZCode/ZCode.exe",
    ],
}];

/// The Claude entries ZCode documents, by name — no Skill matcher, no PreCompact,
/// PostCompact or SessionEnd. A positional `&ENTRIES[..5]` let the T62.1 Skill insert
/// silently swap SessionStart out of the installed set.
const ZCODE_ENTRIES: &[(&str, &str)] = &[
    ("PreToolUse", "Bash"),
    ("PreToolUse", "Read"),
    ("PostToolUse", "*"),
    ("UserPromptSubmit", ""),
    ("SessionStart", ""),
];

fn events() -> &'static [(&'static str, &'static str)] {
    ZCODE_ENTRIES
}

const NAME: &str = "rtok";

/// Apply, dry-run, or remove the hook entries under `hooks.events` (and `hooks.enabled`).
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    edit_json(&apply(cfg), &cfg.setup.zcode.config_path, |root| {
        if remove {
            return strip_ours(root.get_mut("hooks").and_then(|h| h.get_mut("events")));
        }
        let hooks = object_at(root, "hooks");
        let enable = hooks.get("enabled") != Some(&json!(true));
        hooks["enabled"] = json!(true);
        let report = insert_ours(
            object_at(hooks, "events"),
            events(),
            &desktop_command(),
            "timeoutMs",
            cfg.setup.hook_timeout_s * 1000,
        );
        match (enable, report == NO_CHANGES) {
            (false, _) => report,
            (true, true) => "+ hooks.enabled = true".into(),
            (true, false) => format!("+ hooks.enabled = true\n{report}"),
        }
    })
}

/// `mcp.servers.rtok` → `<abs rtok> mcp`.
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = desktop_command();
    let entry = json!({"command": cmd, "args": ["mcp"]});
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &cfg.setup.zcode.config_path,
        "mcp.servers",
        NAME,
        entry,
        &format!("{cmd} mcp"),
    )
}

/// Drop `mcp.servers.rtok` (`rtok agents remove zcode`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_server(
        &apply(cfg),
        &cfg.setup.zcode.config_path,
        "mcp.servers",
        NAME,
    )
}

const PLUGIN_SRC_REL: &str = "plugins/zcode";
const PLUGIN_LOCAL: &str = "~/.zcode/cli/plugins/local";

/// Local ZCode plugin dest: sibling of config.json → `<zcode-cli-dir>/plugins/local/rtok`.
pub fn plugin_dest(cfg: &Config) -> PathBuf {
    cfg.setup
        .zcode
        .config_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("plugins")
        .join("local")
        .join("rtok")
}

/// ZCode's own docs have no GitHub/subdir plugin install; the local link is the only path
/// there is, so it installs by default once ZCode itself is detected (T164).
pub static PLUGIN: HostPlugin = HostPlugin {
    src_rel: PLUGIN_SRC_REL,
    host: "ZCode",
    label: Some(PLUGIN_LOCAL),
    dest: plugin_dest,
    default_install: true,
};

/// Offer / link / unlink `plugins/zcode` (D21, T77). ZCode has no third-party marketplace to
/// push into; it loads inline plugin roots listed in `plugins.dirs` (enabled by default), so
/// linking also lists the dest there and unlinking drops the entry.
/// Dry-run and the unaccepted offer MUST contain the substrings `plugins/zcode`
/// and `~/.zcode/cli/plugins/local` and `ketch install listepo/rtok`.
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    let report = PLUGIN.offer(cfg, remove)?;
    let dirs = plugin_dirs(cfg, remove)?;
    let mut out = Vec::new();
    for line in [report, dirs] {
        if line != NO_CHANGES {
            out.push(line);
        }
    }
    Ok(if out.is_empty() {
        NO_CHANGES.into()
    } else {
        out.join("\n")
    })
}

/// `plugins.dirs` in `config.json` lists inline plugin roots; ZCode enables them by
/// default, so the link needs no `plugins.enabledPlugins` entry alongside it. The entry
/// exists exactly while an rtok-owned plugin sits at the dest: a declined offer adds
/// nothing, and a stale entry whose dest is gone (or foreign) is dropped.
fn plugin_dirs(cfg: &Config, remove: bool) -> Result<String> {
    let dest = plugin_dest(cfg).display().to_string();
    let live = PLUGIN.ours(cfg);
    edit_json(&apply(cfg), &cfg.setup.zcode.config_path, |root| {
        let dirs = array_at(object_at(root, "plugins"), "dirs");
        let has = dirs.iter().any(|v| v.as_str() == Some(dest.as_str()));
        match (remove || !live, has) {
            (true, false) | (false, true) => NO_CHANGES.into(),
            (true, true) => {
                dirs.retain(|v| v.as_str() != Some(dest.as_str()));
                format!("- plugins.dirs -= {dest}")
            }
            (false, false) => {
                dirs.push(json!(dest));
                format!("+ plugins.dirs += {dest}")
            }
        }
    })
}

/// True when the linked plugin is the call path for hooks and MCP (D21 singleton). Keyed on
/// [`HostPlugin::ours`], not `linked`: a foreign directory at the dest is "linked" too, and
/// treating it as the call path would strip a working plain install's `hooks.events` and
/// `mcp.servers.rtok` with nothing left to serve them (T196).
///
/// Judged only by the link, not `--yes`: a dry-run with `--yes` has not linked yet
/// and must still show what the config would get if the offer is declined.
pub fn plugin_serves(cfg: &Config, remove: bool) -> bool {
    !remove && PLUGIN.ours(cfg)
}

impl Agent for Zcode {
    fn id(&self) -> &'static str {
        "zcode"
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

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "hooks" | "mcp" | "plugin" => Support::Yes,
            _ => Support::No(
                "ZCode providers are per-id tables with their own keys and base URLs; setup does not edit them",
            ),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![cfg.setup.zcode.config_path.clone()]
    }

    fn markers(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        let mut paths = self.files(cfg, kind);
        paths.push(plugin_dest(cfg));
        paths
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let s = super::read(&cfg.setup.zcode.config_path);
        // T75: `ours`, not any metadata — a foreign directory at the plugin dest is not
        // an rtok install, so it cannot keep the green mark alive after an uninstall
        // that (rightly) left it alone.
        let plugin = PLUGIN.ours(cfg);
        let mut out = Vec::new();
        // The command is an absolute path, so match the tail rather than a bare `rtok hook`.
        // The linked plugin serves the hooks themselves (D21).
        if s.contains(" hook PreToolUse") || plugin {
            out.push("hooks");
        }
        let root: Value = serde_json::from_str(&s).unwrap_or(Value::Null);
        if root["mcp"]["servers"][NAME].is_object() || plugin {
            out.push("mcp");
        }
        if plugin {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        // Offer first: once the plugin is linked it is the only call path (D21 singleton),
        // so the config-file entries below are stripped rather than (re)added — also on
        // the very run that links, and against leftovers of a declined earlier offer.
        // Judged by the link alone: a dry run or a declined offer has not linked yet
        // and still gets the config-file install.
        let mut lines = vec![offer_plugin(cfg, remove)?];
        lines.push(run(cfg, remove || plugin_serves(cfg, remove))?);
        if remove || plugin_serves(cfg, remove) {
            lines.push(unregister_mcp(cfg)?);
        } else if cfg.setup.mcp {
            lines.push(register_mcp(cfg)?);
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-zcode-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let mut c = Config::default();
        c.setup.zcode.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    #[test]
    fn dry_run_names_five_entries_and_creates_nothing() {
        let (c, path) = cfg("dry", true);
        let report = run(&c, false).unwrap();
        assert!(report.starts_with("+ hooks.enabled = true\n"), "{report}");
        assert!(report.contains("5 additions"), "{report}");
        assert!(report.contains("+ SessionStart "), "{report}");
        assert!(!report.contains("SessionEnd"), "{report}");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn dry_run_offer_names_plugin_and_local() {
        let (c, path) = cfg("dry-offer", true);
        let s = offer_plugin(&c, false).unwrap();
        assert!(s.contains("plugins/zcode"), "{s}");
        assert!(s.contains("~/.zcode/cli/plugins/local"), "{s}");
        assert!(s.contains("ketch install listepo/rtok"), "{s}");
        assert!(!plugin_dest(&c).symlink_metadata().is_ok());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn yes_links_plugin_and_lists_dirs() {
        let (mut c, path) = cfg("yes", false);
        c.setup.yes = true;
        let first = offer_plugin(&c, false).unwrap();
        assert!(first.starts_with("+ plugin"), "{first}");
        assert!(first.contains("+ plugins.dirs +="), "{first}");
        assert!(PLUGIN.linked(&c));
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            root["plugins"]["dirs"][0].as_str().unwrap(),
            plugin_dest(&c).display().to_string()
        );
        assert_eq!(offer_plugin(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(
            Zcode.installed(&c, Kind::Desktop),
            ["hooks", "mcp", "plugin"]
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// The plugin installs by default (T164) and, once linked, is the only call path
    /// (D21): a plain install's config-file hooks and `mcp.servers.rtok` never appear.
    #[test]
    fn linked_plugin_is_the_only_call_path() {
        let (c, path) = cfg("singleton", false);
        let linking = Zcode
            .apply(&c, Kind::Desktop, Mode::Install)
            .unwrap()
            .join("\n");
        assert!(linking.contains("+ plugin"), "{linking}");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains(" hook PreToolUse"), "{raw}");
        let root: Value = serde_json::from_str(&raw).unwrap();
        assert!(root["mcp"]["servers"]["rtok"].is_null(), "{raw}");
        assert!(plugin_serves(&c, false));
        // A later full setup neither re-adds the config entries nor churns.
        let steady = Zcode
            .apply(&c, Kind::Desktop, Mode::Install)
            .unwrap()
            .into_iter()
            .filter(|l| l != NO_CHANGES)
            .count();
        assert_eq!(steady, 0, "steady state must be no changes");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// T196: `plugin_serves` keyed on `linked()` treated a foreign directory at the plugin
    /// dest (rightly refused by `PluginLink::run`) as though rtok's plugin were the call
    /// path — stripping a working plain install's `hooks.events` and `mcp.servers.rtok`
    /// with nothing left to serve them, so `agents install zcode` deleted both and
    /// installed nothing.
    #[test]
    fn foreign_plugin_dir_keeps_plain_hooks_and_mcp_working() {
        let (mut c, path) = cfg("foreign", false);
        c.setup.yes = true;
        // A foreign directory already sits at the plugin dest: not an rtok link, no owned
        // marker, different bytes from the shipped plugin.
        let dest = plugin_dest(&c);
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("mine.txt"), "not rtok's plugin").unwrap();
        // Seed a working plain install: hooks + mcp.servers.rtok already present.
        run(&c, false).unwrap();
        register_mcp(&c).unwrap();
        assert!(PLUGIN.linked(&c), "the foreign dir makes linked() true");
        assert!(!PLUGIN.ours(&c), "but it is not ours");

        let lines = Zcode
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
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(" hook PreToolUse"), "{raw}");
        let root: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(root["mcp"]["servers"]["rtok"]["args"][0], "mcp");
        assert_eq!(Zcode.installed(&c, Kind::Desktop), ["hooks", "mcp"]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// T164: default install never overwrites a foreign directory at the plugin
    /// destination, so `plugins.dirs` gains no entry for it either — and a stale entry
    /// left over from before (its dest no longer ours) is dropped.
    #[test]
    fn a_foreign_plugin_directory_adds_no_dirs_entry_and_drops_a_stale_one() {
        let (c, path) = cfg("declined", false);
        let dest = plugin_dest(&c);
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("mine.txt"), "keep").unwrap();
        let s = offer_plugin(&c, false).unwrap();
        assert!(s.contains("accept with --yes"), "{s}");
        assert!(dest.join("mine.txt").exists(), "foreign dir must survive");
        // A stale entry whose dest does not hold our plugin is dropped.
        fs::write(
            &path,
            json!({"plugins": {"dirs": [dest.display().to_string()]}}).to_string(),
        )
        .unwrap();
        let s = offer_plugin(&c, false).unwrap();
        assert!(s.contains("- plugins.dirs -="), "{s}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn remove_unlinks_plugin_and_drops_dirs_entry() {
        let (mut c, path) = cfg("remove-plugin", false);
        c.setup.yes = true;
        offer_plugin(&c, false).unwrap();
        assert!(PLUGIN.linked(&c));
        let report = offer_plugin(&c, true).unwrap();
        assert!(report.starts_with("- plugin"), "{report}");
        assert!(report.contains("- plugins.dirs -="), "{report}");
        assert!(!plugin_dest(&c).symlink_metadata().is_ok());
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            root["plugins"]["dirs"]
                .as_array()
                .is_none_or(|a| a.is_empty())
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_is_idempotent_and_remove_keeps_foreign() {
        let (c, path) = cfg("apply", false);
        fs::write(
            &path,
            json!({"hooks":{"enabled":false,"events":{"Stop":[{"hooks":[{"type":"command","command":"echo other"}]}]}},
                   "mcp":{"servers":{"other":{"command":"npx","args":["x"]}}}})
            .to_string(),
        )
        .unwrap();
        assert!(run(&c, false).unwrap().contains("5 additions"));
        assert!(register_mcp(&c).unwrap().starts_with("mcp.servers.rtok: "));
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["hooks"]["enabled"], true);
        let pre = &root["hooks"]["events"]["PreToolUse"][0];
        assert_eq!(pre["matcher"], "Bash");
        assert_eq!(pre["hooks"][0]["timeoutMs"], 5000);
        assert!(
            pre["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .ends_with(" hook PreToolUse")
        );
        assert_eq!(root["mcp"]["servers"]["rtok"]["args"][0], "mcp");
        assert_eq!(Zcode.installed(&c, Kind::Desktop), ["hooks", "mcp"]);

        assert!(run(&c, true).unwrap().contains("removed"));
        assert_eq!(unregister_mcp(&c).unwrap(), "- mcp.servers.rtok");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok"), "{raw}");
        let root: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(root["mcp"]["servers"]["other"]["command"], "npx");
        assert!(Zcode.installed(&c, Kind::Desktop).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
