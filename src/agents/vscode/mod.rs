//! VS Code Copilot Chat installer (`rtok agents install vscode`, plan T48.8, T117).
//!
//! GitHub Copilot in VS Code reads MCP servers from the user profile `mcp.json`
//! (`servers.<name>`, `type: "stdio"`) directly, or from a linked Agent Plugin's own
//! `.mcp.json` / `hooks/hooks.json` once the plugin's directory is registered in
//! `chat.pluginLocations` (an object, `<path>: <enabled>`, in `settings.json` — JSONC, T79).
//! VS Code's documented plugin formats include the Claude layout as-is (manifest at
//! `.claude-plugin/plugin.json`, `.mcp.json`, `hooks/hooks.json` — exactly `plugins/claude/`),
//! and its hook event names (`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
//! `PreCompact`, `SubagentStart`, `SubagentStop`, `Stop`) match rtok's own, so setup links that
//! tree instead of shipping a separate `plugins/vscode/` copy (T117 doc research, fetched
//! 2026-09-24 — see `## Docs` below). D21: while the plugin is registered, its hooks and MCP
//! are the unit — a plain `mcp.json` entry is stripped instead of written.
//!
//! The plugin lands at a fixed `<user-dir>/plugins/rtok` per profile (a symlink to
//! `plugins/claude/`, like every other linked host plugin) rather than directly at the
//! resolved repo/ketch-store path: `chat.pluginLocations` keys on that path, so a version
//! bump that moves the ketch-store source would otherwise leave a stale key behind on every
//! upgrade. [`super::plugin::HostPlugin`] already relinks a stale target in place, keeping
//! the registered path stable across versions.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rtok_agent_sdk::NO_CHANGES;
use serde_json::json;

use super::plugin::HostPlugin;
use super::{Agent, Kind, Mode, Support, Variant, apply, home_dir, jsonc};
use crate::config::Config;

const NAME: &str = "rtok";

/// VS Code stable and Insiders: one apply writes both profile `mcp.json`/`settings.json` files.
pub struct Vscode;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Desktop,
        name: "VS Code",
        bins: &["code"],
        apps: &[
            "/Applications/Visual Studio Code.app",
            "$LOCALAPPDATA/Programs/Microsoft VS Code/Code.exe",
        ],
    },
    Variant {
        kind: Kind::Desktop,
        name: "VS Code - Insiders",
        bins: &["code-insiders"],
        apps: &[
            "/Applications/Visual Studio Code - Insiders.app",
            "$LOCALAPPDATA/Programs/Microsoft VS Code Insiders/Code - Insiders.exe",
        ],
    },
];

impl Agent for Vscode {
    fn id(&self) -> &'static str {
        "vscode"
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
            "plugin" => Support::Flag("--yes"),
            "hooks" => Support::No(
                "no direct hooks file rtok writes; the linked plugin's hooks/hooks.json is what VS Code's Local harness runs (chat.useClaudeHooks)",
            ),
            "proxy" => Support::No(
                "Copilot in VS Code has no documented base-URL setting to point at the proxy",
            ),
            _ => Support::No("VS Code loads MCP from the user mcp.json, or from the linked plugin"),
        }
    }

    fn plugin_surfaces(&self) -> &'static [rtok_plugin_sdk::Surface] {
        &[
            rtok_plugin_sdk::Surface::Hook,
            rtok_plugin_sdk::Surface::Mcp,
        ]
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![
            mcp_path(cfg, false),
            mcp_path(cfg, true),
            settings_path(cfg, false),
            settings_path(cfg, true),
        ]
    }

    fn markers(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        let mut paths = self.files(cfg, kind);
        paths.push(PLUGIN_STABLE.path(cfg));
        paths.push(PLUGIN_INSIDERS.path(cfg));
        paths
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let stable_mcp = super::read(&mcp_path(cfg, false)).contains("\"rtok\"");
        let insiders_mcp = super::read(&mcp_path(cfg, true)).contains("\"rtok\"");
        // T75: a foreign directory at either dest must not hold the green mark.
        let plugin = PLUGIN_STABLE.ours(cfg) || PLUGIN_INSIDERS.ours(cfg);
        let mut out = Vec::new();
        if stable_mcp || insiders_mcp || plugin {
            out.push("mcp");
        }
        if plugin {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines = Vec::new();
        for (plugin, insiders) in [(&PLUGIN_STABLE, false), (&PLUGIN_INSIDERS, true)] {
            let line = offer_plugin(cfg, plugin, insiders, remove)?;
            if line != NO_CHANGES {
                lines.push(line);
            }
        }
        for insiders in [false, true] {
            let plugin = if insiders {
                &PLUGIN_INSIDERS
            } else {
                &PLUGIN_STABLE
            };
            let line = if remove {
                unregister_mcp(cfg, insiders)?
            } else if plugin.ours(cfg) {
                // D21: the plugin is the unit — cleared on every run while it serves, not
                // only the run that linked it (a declined-then-later-accepted offer, or a
                // hand-reverted settings edit, must not leave the two out of sync — T196).
                unregister_mcp(cfg, insiders)?
            } else if cfg.setup.mcp {
                register_mcp(cfg, insiders)?
            } else {
                NO_CHANGES.into()
            };
            if line != NO_CHANGES {
                lines.push(line);
            }
        }
        if lines.is_empty() {
            lines.push(NO_CHANGES.into());
        }
        Ok(lines)
    }
}

/// `<user-dir>/mcp.json` — Code or Code - Insiders profile.
pub fn mcp_path(cfg: &Config, insiders: bool) -> PathBuf {
    user_dir(cfg, insiders).join("mcp.json")
}

/// `<user-dir>/settings.json` — sibling of `mcp.json`; carries `chat.pluginLocations` (JSONC).
pub fn settings_path(cfg: &Config, insiders: bool) -> PathBuf {
    user_dir(cfg, insiders).join("settings.json")
}

fn user_dir(cfg: &Config, insiders: bool) -> PathBuf {
    let base = if insiders {
        &cfg.setup.vscode.insiders_user_dir
    } else {
        &cfg.setup.vscode.code_user_dir
    };
    if base.as_os_str().is_empty() {
        default_user_dir(insiders)
    } else {
        base.clone()
    }
}

/// OS-default VS Code user profile dir (without `mcp.json`).
pub fn default_user_dir(insiders: bool) -> PathBuf {
    let home = home_dir();
    if cfg!(target_os = "macos") {
        let app = if insiders { "Code - Insiders" } else { "Code" };
        return home
            .join("Library/Application Support")
            .join(app)
            .join("User");
    }
    if cfg!(windows) {
        let app = if insiders { "Code - Insiders" } else { "Code" };
        let root = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home);
        return root.join(app).join("User");
    }
    let app = if insiders { "Code - Insiders" } else { "Code" };
    home.join(".config").join(app).join("User")
}

/// `servers.rtok = {type: "stdio", command, args}` in the profile `mcp.json`.
pub fn register_mcp(cfg: &Config, insiders: bool) -> Result<String> {
    let path = mcp_path(cfg, insiders);
    let cmd = super::rtok_command();
    let entry = json!({"type": "stdio", "command": cmd, "args": ["mcp"]});
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &path,
        "servers",
        NAME,
        entry,
        &format!("{cmd} mcp"),
    )
}

/// Drop `servers.rtok` from the profile `mcp.json`.
pub fn unregister_mcp(cfg: &Config, insiders: bool) -> Result<String> {
    rtok_agent_sdk::unregister_server(&apply(cfg), &mcp_path(cfg, insiders), "servers", NAME)
}

/// The linked plugin, one per profile: both point at `plugins/claude/` (T117 — VS Code
/// accepts that tree's layout as-is), landing at a fixed `<user-dir>/plugins/rtok` so the
/// path registered in `chat.pluginLocations` never drifts across ketch version bumps.
pub static PLUGIN_STABLE: HostPlugin = HostPlugin {
    src_rel: "plugins/claude",
    host: "VS Code",
    label: None,
    dest: plugin_dest_stable,
    // Left asking with `--yes`: unlike the local-link-only hosts (T164), VS Code plugins are
    // registered by editing the user's own settings.json, not just filling an already-scanned
    // directory, so the plan card asks for that explicitly.
    default_install: false,
};

/// The Insiders profile's copy of [`PLUGIN_STABLE`].
pub static PLUGIN_INSIDERS: HostPlugin = HostPlugin {
    src_rel: "plugins/claude",
    host: "VS Code - Insiders",
    label: None,
    dest: plugin_dest_insiders,
    default_install: false,
};

fn plugin_dest_stable(cfg: &Config) -> PathBuf {
    plugin_dest(cfg, false)
}

fn plugin_dest_insiders(cfg: &Config) -> PathBuf {
    plugin_dest(cfg, true)
}

fn plugin_dest(cfg: &Config, insiders: bool) -> PathBuf {
    user_dir(cfg, insiders).join("plugins").join(NAME)
}

/// [`HostPlugin::offer`] plus the `chat.pluginLocations` registration VS Code needs beyond
/// the dest link itself — nothing scans the plugin directory, so setup must also point
/// `settings.json` at it (T117). One report, both steps.
fn offer_plugin(cfg: &Config, plugin: &HostPlugin, insiders: bool, remove: bool) -> Result<String> {
    let was_ours = plugin.ours(cfg);
    let link_line = plugin.offer(cfg, remove)?;
    if apply(cfg).dry_run {
        return Ok(link_line);
    }
    let dest = plugin.path(cfg);
    let settings_line = if remove {
        if was_ours {
            unregister_plugin_location(cfg, insiders, &dest)?
        } else {
            NO_CHANGES.into()
        }
    } else if plugin.ours(cfg) {
        register_plugin_location(cfg, insiders, &dest)?
    } else {
        NO_CHANGES.into()
    };
    Ok(if settings_line == NO_CHANGES {
        link_line
    } else {
        format!("{link_line}\n{settings_line}")
    })
}

/// `chat.pluginLocations.<dest> = true` in `settings.json`, JSONC-safe (T79/T117): comments,
/// trailing commas and every foreign registered plugin path survive.
fn register_plugin_location(cfg: &Config, insiders: bool, dest: &Path) -> Result<String> {
    let path = settings_path(cfg, insiders);
    let raw = jsonc::read_or_empty(&path)?;
    let key = dest.display().to_string();
    let (body, edit) =
        jsonc::upsert_member(&raw, &path, "chat.pluginLocations", &key, &json!(true))?;
    jsonc::parse(&body).with_context(|| path.display().to_string())?;
    let report = match edit {
        jsonc::Upsert::NoChange => NO_CHANGES.into(),
        jsonc::Upsert::Added => format!("+ chat.pluginLocations: {key}"),
        jsonc::Upsert::Replaced => format!("~ chat.pluginLocations: {key}"),
    };
    rtok_agent_sdk::write(&apply(cfg), &path, &body, &report)?;
    Ok(report)
}

/// Drop exactly our `chat.pluginLocations` entry, keeping every foreign one and every comment.
fn unregister_plugin_location(cfg: &Config, insiders: bool, dest: &Path) -> Result<String> {
    let path = settings_path(cfg, insiders);
    let raw = jsonc::read_or_empty(&path)?;
    let key = dest.display().to_string();
    let (body, removed) = jsonc::remove_member(&raw, &path, "chat.pluginLocations", &key)?;
    jsonc::parse(&body).with_context(|| path.display().to_string())?;
    let report = if removed {
        format!("- chat.pluginLocations: {key}")
    } else {
        NO_CHANGES.into()
    };
    rtok_agent_sdk::write(&apply(cfg), &path, &body, &report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(dir: PathBuf, dry: bool) -> Config {
        let mut c = Config::default();
        c.setup.vscode.code_user_dir = dir.clone();
        c.setup.vscode.insiders_user_dir = dir.join("insiders");
        c.setup.dry_run = dry;
        c.setup.backup = false;
        c
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-vscode-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(dir.join("insiders")).unwrap();
        dir
    }

    #[test]
    fn mcp_entry_is_stdio_and_idempotent() {
        let dir = tmp("mcp");
        let c = cfg(dir.clone(), false);
        let first = register_mcp(&c, false).unwrap();
        assert!(first.starts_with("servers.rtok: "), "{first}");
        assert_eq!(register_mcp(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(mcp_path(&c, false)).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let rtok = &doc["servers"]["rtok"];
        assert_eq!(rtok["type"], "stdio");
        assert_eq!(rtok["args"], json!(["mcp"]));
        assert_eq!(unregister_mcp(&c, false).unwrap(), "- servers.rtok");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn default_user_dir_names_code_and_insiders() {
        let stable = default_user_dir(false);
        let insiders = default_user_dir(true);
        assert!(stable.to_string_lossy().contains("Code"));
        assert!(insiders.to_string_lossy().contains("Insiders"));
        assert_ne!(stable, insiders);
    }

    /// T117 Check: settings round trip — add, idempotent, remove keeps foreign entries and
    /// comments, against `chat.pluginLocations` directly (below the `HostPlugin` offer).
    #[test]
    fn plugin_location_round_trip_keeps_comments_and_foreign_entries() {
        let dir = tmp("settings");
        let path = dir.join("settings.json");
        fs::write(
            &path,
            "{\n  // my theme\n  \"workbench.colorTheme\": \"Dark\",\n  \"chat.pluginLocations\": {\n    // someone else's plugin\n    \"/other/plugin\": true\n  }\n}\n",
        )
        .unwrap();
        let c = cfg(dir.clone(), false);
        let dest = plugin_dest(&c, false);

        let first = register_plugin_location(&c, false, &dest).unwrap();
        assert!(first.starts_with("+ chat.pluginLocations: "), "{first}");
        assert_eq!(
            register_plugin_location(&c, false, &dest).unwrap(),
            NO_CHANGES
        );
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("// my theme"), "{raw}");
        assert!(raw.contains("// someone else's plugin"), "{raw}");
        let root = jsonc::parse(&raw).unwrap();
        assert_eq!(root["chat.pluginLocations"]["/other/plugin"], json!(true));
        assert_eq!(
            root["chat.pluginLocations"][dest.display().to_string()],
            json!(true)
        );

        let removed = unregister_plugin_location(&c, false, &dest).unwrap();
        assert!(removed.starts_with("- chat.pluginLocations: "), "{removed}");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("// my theme") && raw.contains("// someone else's plugin"));
        let root = jsonc::parse(&raw).unwrap();
        assert_eq!(root["chat.pluginLocations"]["/other/plugin"], json!(true));
        assert!(
            root["chat.pluginLocations"]
                .get(dest.display().to_string())
                .is_none()
        );
        assert_eq!(
            unregister_plugin_location(&c, false, &dest).unwrap(),
            NO_CHANGES
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// Full `apply`: `--yes` links the plugin, registers `chat.pluginLocations`, and D21
    /// strips a leftover plain `mcp.json` entry; `remove` takes both back and leaves the
    /// dest a foreign directory would have kept untouched.
    #[test]
    fn yes_links_plugin_registers_settings_and_strips_leftover_mcp() {
        let dir = tmp("yes");
        let mut c = cfg(dir.clone(), false);
        c.setup.yes = true;
        // A plain install ran first (or the offer was declined earlier): mcp.json already
        // carries `servers.rtok` before the plugin is linked.
        register_mcp(&c, false).unwrap();
        register_mcp(&c, true).unwrap();

        let lines = Vscode.apply(&c, Kind::Desktop, Mode::Install).unwrap();
        let joined = lines.join("\n");
        assert!(joined.contains("chat.pluginLocations"), "{joined}");
        assert!(PLUGIN_STABLE.ours(&c));
        assert!(PLUGIN_INSIDERS.ours(&c));

        let settings = fs::read_to_string(settings_path(&c, false)).unwrap();
        let root = jsonc::parse(&settings).unwrap();
        let dest = plugin_dest(&c, false).display().to_string();
        assert_eq!(root["chat.pluginLocations"][dest.clone()], json!(true));

        // D21: the plugin now serves MCP — the leftover plain entry is gone.
        assert!(
            !fs::read_to_string(mcp_path(&c, false))
                .unwrap_or_default()
                .contains("rtok"),
            "leftover mcp.json entry must be cleared once the plugin is ours"
        );
        assert_eq!(Vscode.installed(&c, Kind::Desktop), ["mcp", "plugin"]);

        let removed = Vscode
            .apply(&c, Kind::Desktop, Mode::Remove)
            .unwrap()
            .join("\n");
        assert!(removed.contains("chat.pluginLocations"), "{removed}");
        assert!(!PLUGIN_STABLE.ours(&c));
        let settings = fs::read_to_string(settings_path(&c, false)).unwrap();
        assert!(
            !jsonc::parse(&settings)
                .unwrap()
                .pointer(&format!("/chat.pluginLocations/{dest}"))
                .is_some(),
            "{settings}"
        );
        assert!(Vscode.installed(&c, Kind::Desktop).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    /// Without `--yes` the offer is declined and nothing is written; `mcp.json` still gets
    /// the plain registration (matches every host's default, non-consenting `agents install`).
    #[test]
    fn dry_run_and_undeclared_offers_touch_nothing_but_plain_mcp() {
        let dir = tmp("declined");
        let c = cfg(dir.clone(), false);
        let lines = Vscode
            .apply(&c, Kind::Desktop, Mode::Install)
            .unwrap()
            .join("\n");
        assert!(lines.contains("servers.rtok: "), "{lines}");
        assert!(!plugin_dest(&c, false).exists());
        assert!(!settings_path(&c, false).exists());

        let dry = cfg(dir.join("dry"), true);
        fs::create_dir_all(&dry.setup.vscode.code_user_dir).unwrap();
        fs::create_dir_all(&dry.setup.vscode.insiders_user_dir).unwrap();
        let mut dry = dry;
        dry.setup.yes = true;
        let out = Vscode
            .apply(&dry, Kind::Desktop, Mode::Install)
            .unwrap()
            .join("\n");
        assert!(out.contains("plugins/claude"), "{out}");
        assert!(!plugin_dest(&dry, false).exists());
        assert!(!settings_path(&dry, false).exists());
        let _ = fs::remove_dir_all(dir);
    }
}
