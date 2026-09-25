//! Devin CLI + Desktop (`rtok agents install devin`, plan T89).
//!
//! Both surfaces read the same user files. Hooks live under the `"hooks"` key of
//! `config.json` (event names at the top of that object, each an array of
//! `{matcher?, hooks: [{type, command, timeout}]}` — the same groups as
//! `plugins/devin/hooks.json`, which has no wrapper key). MCP is `mcpServers.rtok`
//! in the sibling `mcp_config.json`.
//!
//! Devin does not document where `devin plugins install --local` records a plugin
//! (plugins overview, fetched 2026-09-25; this machine's `~/.config/devin/` has no
//! plugin store, and `devin plugins list` is a live command, not a file). There is
//! no marker `installed()` can honestly read, so it never reports `plugin`. Setup
//! still writes the user files — that is the path without the plugin — and prints
//! the install line behind `--yes`. It does not guess the store and does not strip
//! on a hunch (a guess either double-fires or deletes hooks the plugin still needs).

use std::path::{Path, PathBuf};

use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, array_at, edit_json, object_at};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

/// `(event, matcher)` in Devin's names. Empty matcher omits the field. Mapped from
/// Claude's `ENTRIES`: `Bash`/`Read` → `^exec$`/`^read$`, no `Skill` (Devin has no
/// such tool hook here), `PreCompact`+`PostCompact` → the one event `PostCompaction`.
/// The same list `plugins/devin/hooks.json` carries (T88).
const ENTRIES: &[(&str, &str)] = &[
    ("PreToolUse", "^exec$"),
    ("PreToolUse", "^read$"),
    ("PostToolUse", ""),
    ("UserPromptSubmit", ""),
    ("SessionStart", ""),
    ("PostCompaction", ""),
    ("SessionEnd", ""),
];

/// CLI (`devin` on PATH) and Desktop (`Devin.app`): one install writes the same files.
pub struct Devin;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Devin CLI",
        bins: &["devin"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Devin",
        bins: &[],
        apps: &[
            "/Applications/Devin.app",
            "$LOCALAPPDATA/Programs/Devin/Devin.exe",
        ],
    },
];

impl Agent for Devin {
    fn id(&self) -> &'static str {
        "devin"
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
            "plugin" => Support::Offer("--yes"),
            "proxy" => Support::No(
                "Devin's proxy key is an HTTP proxy for CLI traffic, not a model API base URL",
            ),
            _ => Support::No("not a Devin surface"),
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
        vec![config_path(cfg), mcp_path(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let mut out = Vec::new();
        if super::read(&config_path(cfg)).contains("rtok hook") {
            out.push("hooks");
        }
        if super::read(&mcp_path(cfg)).contains("\"rtok\"") {
            out.push("mcp");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        if remove {
            return Ok(vec![
                offer_plugin(cfg, true)?,
                run(cfg, true)?,
                unregister_mcp(cfg)?,
            ]);
        }
        let mut lines = vec![run(cfg, false)?];
        if cfg.setup.mcp {
            lines.push(register_mcp(cfg)?);
        }
        // A repeat install is all `NO_CHANGES`, including the offer (kimi's rule):
        // every line counts toward `already installed`.
        if lines.iter().any(|l| l != NO_CHANGES) && cfg.setup.yes {
            lines.insert(0, offer_plugin(cfg, false)?);
        } else {
            lines.insert(0, NO_CHANGES.into());
        }
        Ok(lines)
    }
}

/// `config.json`. On Windows the shipped default (`~/.config/devin/config.json`) is
/// not where Devin reads; the docs name `%APPDATA%\devin\config.json`. An explicit
/// path (tests, `rtok config set`) is kept.
pub fn config_path(cfg: &Config) -> PathBuf {
    let path = &cfg.setup.devin.config_path;
    if cfg!(windows)
        && is_xdg_default(path)
        && let Some(appdata) = std::env::var_os("APPDATA")
    {
        return PathBuf::from(appdata).join("devin").join("config.json");
    }
    path.clone()
}

fn is_xdg_default(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('\\', "/")
        .ends_with(".config/devin/config.json")
}

/// `mcp_config.json` beside [`config_path`] (v3000.3+; older builds kept
/// `mcpServers` inside `config.json` and migrate on startup).
pub fn mcp_path(cfg: &Config) -> PathBuf {
    config_path(cfg)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mcp_config.json")
}

fn command(bin: &str, event: &str) -> String {
    let args = format!("hook {event} --host devin");
    if cfg!(windows) || bin != "rtok" {
        return format!("{bin} {args}");
    }
    super::hook_resolver(&args, None)
}

/// The POSIX resolver's `exec rtok hook <event> --host devin;` marker, or a bare
/// `<rtok-bin> hook <event> --host devin` (Windows, absolute path).
fn is_ours(cmd: &str, event: &str) -> bool {
    let suffix = format!(" hook {event} --host devin");
    if let Some(bin) = cmd.strip_suffix(&suffix) {
        return super::is_rtok_bin(super::unquote_bin(bin));
    }
    cmd.contains(&format!("exec rtok hook {event} --host devin;"))
}

/// The plugin's `hooks.json`: top-level event names, no `"hooks"` wrapper.
/// Built from [`ENTRIES`] and [`command`], so the installer and the plugin cannot drift.
pub fn hooks_doc() -> Value {
    let mut hooks = serde_json::Map::new();
    for &(event, matcher) in ENTRIES {
        let cmd = command("rtok", event);
        let mut group = json!({
            "hooks": [{
                "type": "command",
                "command": cmd,
                "timeout": 5
            }]
        });
        if !matcher.is_empty() {
            group["matcher"] = json!(matcher);
        }
        hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("event array")
            .push(group);
    }
    Value::Object(hooks)
}

/// `devin plugins install --local <resolved plugins/devin>`. rtok never writes
/// the store. `installed` is always false: there is no marker to read.
pub fn offer_plugin(cfg: &Config, remove: bool) -> Result<String> {
    Ok(super::print_offer(
        cfg,
        remove,
        false,
        "plugins/devin",
        &format!(
            "devin plugins install --local {}",
            super::plugin_src("plugins/devin").display()
        ),
        "keep the Devin plugin (its store is undocumented; remove with `devin plugins remove rtok --local`)",
    ))
}

pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = config_path(cfg);
    edit_json(&apply(cfg), &path, |root| {
        let hooks = object_at(root, "hooks");
        if remove {
            strip_ours(&apply(cfg), &path, hooks, cfg.setup.hook_timeout_s)
        } else {
            insert_ours(hooks, &super::rtok_hook_bin(), cfg.setup.hook_timeout_s)
        }
    })
}

fn matcher_of(entry: &Value) -> &str {
    entry.get("matcher").and_then(Value::as_str).unwrap_or("")
}

fn insert_ours(hooks: &mut Value, bin: &str, timeout: u64) -> String {
    let mut changed = Vec::new();
    for &(event, matcher) in ENTRIES {
        let want = command(bin, event);
        let mut found = false;
        for entry in array_at(hooks, event).iter_mut() {
            if matcher_of(entry) != matcher {
                continue;
            }
            let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            for h in inner {
                if !h["command"].as_str().is_some_and(|c| is_ours(c, event)) {
                    continue;
                }
                found = true;
                if h["command"] != json!(want) || h["timeout"] != json!(timeout) {
                    h["command"] = json!(want);
                    h["timeout"] = json!(timeout);
                    changed.push(format!("~ {event} {matcher}"));
                }
            }
        }
        if found {
            continue;
        }
        let mut group = json!({
            "hooks": [{"type": "command", "command": want, "timeout": timeout}]
        });
        if !matcher.is_empty() {
            group["matcher"] = json!(matcher);
        }
        array_at(hooks, event).push(group);
        changed.push(format!("+ {event} {matcher}"));
    }
    if changed.is_empty() {
        NO_CHANGES.into()
    } else {
        changed.join("\n")
    }
}

/// Drop hooks whose command is ours. A hook still shaped as we write it goes;
/// one the user edited stays unless [`super::takes_hook`] says otherwise.
/// Foreign commands on the same event stay.
fn strip_ours(
    apply: &rtok_agent_sdk::Apply,
    path: &Path,
    hooks: &mut Value,
    timeout: u64,
) -> String {
    let Some(obj) = hooks.as_object_mut() else {
        return NO_CHANGES.into();
    };
    let (mut removed, mut kept) = (0usize, Vec::new());
    for arr in obj.values_mut().filter_map(Value::as_array_mut) {
        for entry in arr.iter_mut() {
            let bare = entry
                .as_object()
                .is_some_and(|o| o.len() == 1 || (o.len() == 2 && o.contains_key("matcher")));
            let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            inner.retain(|h| {
                let Some(cmd) = h["command"].as_str() else {
                    return true;
                };
                let Some(event) = ENTRIES.iter().map(|(e, _)| *e).find(|e| is_ours(cmd, e)) else {
                    return true;
                };
                let want = json!({"type": "command", "command": cmd, "timeout": timeout});
                let at = || format!("hooks.{event} in {}", path.display());
                let take = super::takes_hook(apply, bare && *h == want, at, &mut kept);
                removed += usize::from(take);
                !take
            });
        }
        arr.retain(|e| {
            e.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|a| !a.is_empty())
        });
    }
    obj.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    super::with_kept(kept, super::removed_report(removed))
}

fn mcp_entry(cmd: &str) -> Value {
    json!({"command": cmd, "args": ["mcp"]})
}

pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_server(
        &apply(cfg),
        &mcp_path(cfg),
        "mcpServers",
        NAME,
        mcp_entry(&cmd),
        &format!("{cmd} mcp"),
    )
}

pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    super::unregister_ours(cfg, &mcp_path(cfg), "mcpServers", NAME, &mcp_entry("rtok"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> (Config, PathBuf) {
        let (mut c, path) =
            super::super::test_scratch_cfg("devin", name, "config.json", false, |c, path| {
                c.setup.devin.config_path = path;
            });
        c.setup.yes = true;
        (c, path)
    }

    fn install(c: &Config) -> Vec<String> {
        Devin.apply(c, Kind::Cli, Mode::Install).unwrap()
    }

    #[test]
    fn install_writes_hooks_and_mcp_and_is_idempotent() {
        let (c, path) = scratch("install");
        let first = install(&c);
        assert!(
            first.iter().any(|l| l.contains("plugins/devin")
                && l.contains("devin plugins install --local")),
            "{first:?}"
        );
        let hooks =
            serde_json::from_str::<Value>(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(hooks["hooks"], hooks_doc());
        let mcp: Value = serde_json::from_str(
            &std::fs::read_to_string(path.parent().unwrap().join("mcp_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(mcp["mcpServers"]["rtok"]["command"], "rtok");
        assert_eq!(mcp["mcpServers"]["rtok"]["args"], json!(["mcp"]));
        let second = install(&c);
        assert!(second.iter().all(|l| l == NO_CHANGES), "{second:?}");
    }

    #[test]
    fn remove_takes_ours_and_foreign_hooks_survive_both() {
        let (c, path) = scratch("foreign");
        std::fs::write(
            &path,
            r#"{"version":1,"hooks":{"PreToolUse":[{"matcher":"exec","hooks":[{"type":"command","command":"echo foreign","timeout":30}]}],"Stop":[{"hooks":[{"type":"command","command":"echo stop","timeout":9}]}]}}"#,
        )
        .unwrap();
        std::fs::write(
            path.parent().unwrap().join("mcp_config.json"),
            r#"{"mcpServers":{"other":{"command":"other","args":[]}}}"#,
        )
        .unwrap();
        install(&c);
        Devin.apply(&c, Kind::Cli, Mode::Remove).unwrap();
        let hooks: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let text = hooks.to_string();
        assert!(text.contains("echo foreign"), "{text}");
        assert!(text.contains("echo stop"), "{text}");
        assert!(!text.contains("rtok"), "{text}");
        assert_eq!(hooks["version"], 1);
        let mcp: Value = serde_json::from_str(
            &std::fs::read_to_string(path.parent().unwrap().join("mcp_config.json")).unwrap(),
        )
        .unwrap();
        assert!(mcp["mcpServers"].get("rtok").is_none(), "{mcp}");
        assert!(mcp["mcpServers"].get("other").is_some(), "{mcp}");
    }

    #[test]
    fn plugin_manifest_matches_the_installer() {
        let file: Value =
            serde_json::from_str(include_str!("../../../plugins/devin/hooks.json")).unwrap();
        assert_eq!(hooks_doc(), file);
    }

    #[test]
    fn installed_never_reports_plugin() {
        let (c, _) = scratch("plugin");
        install(&c);
        let found = Devin.installed(&c, Kind::Cli);
        assert!(
            found.contains(&"hooks") && found.contains(&"mcp"),
            "{found:?}"
        );
        assert!(!found.contains(&"plugin"), "{found:?}");
    }
}
