//! Install rtok hooks into Claude Code `settings.json` (plan T2.3).
//!
//! What is Claude-specific is the `hooks` shape below; backup, the write gate and the
//! `mcpServers` entry come from `rtok-agent-sdk` (D28).

pub mod migrate;

use std::path::PathBuf;

use crate::config::Config;
use anyhow::Result;
use rtok_agent_sdk::{KETCH_INSTALL, NO_CHANGES, array_at, edit_json, object_at};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};

/// `(event, matcher)` — empty matcher omits the field. `SessionEnd` was missing, so no session
/// row got `ended_at` and no OTel session root ever shipped (`hooks::dispatch` handles it).
/// Hosts with a Claude-compatible hook shape (ZCode) take a prefix of this list.
pub(super) const ENTRIES: &[(&str, &str)] = &[
    ("PreToolUse", "Bash"),
    ("PreToolUse", "Read"),
    ("PreToolUse", "Skill"),
    ("PostToolUse", "*"),
    ("UserPromptSubmit", ""),
    ("SessionStart", ""),
    ("PreCompact", ""),
    ("PostCompact", ""),
    ("SessionEnd", ""),
];

fn command(bin: &str, event: &str) -> String {
    format!("{bin} hook {event}")
}

/// Exactly `<rtok-bin> hook <event>`. Matching tokens anywhere claimed a user's
/// chain (`notify-send hi && rtok hook Stop`). Suffix match keeps absolute paths
/// with spaces (quoted) as ours on Windows.
pub(super) fn is_ours(cmd: &str, event: &str) -> bool {
    let suffix = format!(" hook {event}");
    let Some(bin) = cmd.strip_suffix(&suffix) else {
        return false;
    };
    super::is_rtok_bin(super::unquote_bin(bin))
}

/// Apply, dry-run, or remove rtok hook entries.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    edit_json(&apply(cfg), &cfg.setup.claude.settings_path, |root| {
        if remove {
            strip_ours(root.get_mut("hooks"))
        } else {
            let bin = super::rtok_hook_bin();
            insert_ours(
                object_at(root, "hooks"),
                ENTRIES,
                &bin,
                "timeout",
                cfg.setup.hook_timeout_s,
            )
        }
    })
}

fn has_ours(entry: &Value, event: &str, matcher: &str) -> bool {
    let got = entry.get("matcher").and_then(Value::as_str).unwrap_or("");
    got == matcher
        && entry
            .get("hooks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|h| h.get("command").and_then(Value::as_str))
            .any(|c| is_ours(c, event))
}

/// Add `<bin> hook <event>` under `hooks.<event>[]` for each entry not already ours, and
/// bring the ones that are up to date (T242.1): an rtok hook written by another binary path
/// or with another timeout is rewritten in its slot, and an rtok hook on a pair `entries` no
/// longer lists (a changed matcher) is dropped. Foreign hooks are never touched.
/// `timeout_key` is the host's spelling (`timeout` seconds in Claude, `timeoutMs` in ZCode).
pub(super) fn insert_ours(
    hooks: &mut Value,
    entries: &[(&str, &str)],
    bin: &str,
    timeout_key: &str,
    timeout: u64,
) -> String {
    let mut changed = prune_ours(hooks, entries);
    for &(event, matcher) in entries {
        let want = command(bin, event);
        let mut found = false;
        for entry in array_at(hooks, event).iter_mut() {
            if !has_ours(entry, event, matcher) {
                continue;
            }
            for h in entry["hooks"].as_array_mut().into_iter().flatten() {
                if !h["command"].as_str().is_some_and(|c| is_ours(c, event)) {
                    continue;
                }
                found = true;
                if h["command"] != json!(want) || h[timeout_key] != json!(timeout) {
                    h["command"] = json!(want);
                    h[timeout_key] = json!(timeout);
                    changed.push(format!("~ {event}{} {want}", show(matcher)));
                }
            }
        }
        if found {
            continue;
        }
        let mut obj = serde_json::Map::new();
        if !matcher.is_empty() {
            obj.insert("matcher".into(), json!(matcher));
        }
        obj.insert(
            "hooks".into(),
            json!([{"type":"command","command":command(bin, event),timeout_key:timeout}]),
        );
        array_at(hooks, event).push(Value::Object(obj));
        changed.push(format!("+ {event}{} {want}", show(matcher)));
    }
    if changed.is_empty() {
        return NO_CHANGES.into();
    }
    let count = |p: char| changed.iter().filter(|l| l.starts_with(p)).count();
    let summary: Vec<String> = [('+', "additions"), ('~', "updates"), ('-', "removals")]
        .into_iter()
        .filter(|&(p, _)| count(p) > 0)
        .map(|(p, word)| format!("{} {word}", count(p)))
        .collect();
    format!("{}\n{}", changed.join("\n"), summary.join(", "))
}

/// ` <matcher>` for a report line; nothing for an empty matcher.
fn show(matcher: &str) -> String {
    if matcher.is_empty() {
        String::new()
    } else {
        format!(" {matcher}")
    }
}

/// Drop rtok hooks sitting on an `(event, matcher)` pair `entries` does not list — what an
/// older install wrote before a matcher changed or an event went (T242.1). Emptied entries
/// and event arrays go, as in [`strip_ours`]; one `- …` report line per dropped hook.
fn prune_ours(hooks: &mut Value, entries: &[(&str, &str)]) -> Vec<String> {
    let (mut removed, mut emptied) = (Vec::new(), Vec::new());
    let Some(map) = hooks.as_object_mut() else {
        return removed;
    };
    for (event, arr) in map.iter_mut() {
        let Some(arr) = arr.as_array_mut() else {
            continue;
        };
        let before = removed.len();
        for entry in arr.iter_mut() {
            let matcher = entry["matcher"].as_str().unwrap_or("").to_string();
            if entries.contains(&(event.as_str(), matcher.as_str())) {
                continue;
            }
            let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            inner.retain(|h| match h["command"].as_str() {
                Some(c) if is_ours(c, event) => {
                    removed.push(format!("- {event}{} {c}", show(&matcher)));
                    false
                }
                _ => true,
            });
        }
        if removed.len() > before {
            arr.retain(|e| {
                e.get("hooks")
                    .and_then(Value::as_array)
                    .is_none_or(|a| !a.is_empty())
            });
            if arr.is_empty() {
                emptied.push(event.clone());
            }
        }
    }
    map.retain(|k, _| !emptied.contains(k));
    removed
}

/// Remove every `<rtok> hook <event>` entry under the given `hooks` object; empty arrays go.
pub(super) fn strip_ours(hooks: Option<&mut Value>) -> String {
    let Some(hooks) = hooks.and_then(Value::as_object_mut) else {
        return NO_CHANGES.into();
    };
    let mut removed = 0usize;
    for (event, entries) in hooks.iter_mut() {
        let Some(arr) = entries.as_array_mut() else {
            continue;
        };
        for entry in arr.iter_mut() {
            let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            let n = inner.len();
            inner.retain(|h| {
                !h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| is_ours(c, event))
            });
            removed += n - inner.len();
        }
        arr.retain(|e| {
            e.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|a| !a.is_empty())
        });
    }
    hooks.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    if removed == 0 {
        NO_CHANGES.into()
    } else {
        format!("{removed} removed")
    }
}

/// Add `rtok mcp` to `mcpServers` in `~/.claude.json` (T4.7).
pub fn register_mcp(cfg: &Config) -> Result<String> {
    let cmd = super::rtok_command();
    rtok_agent_sdk::register_mcp(&apply(cfg), &cfg.doctor.claude_json, "rtok", &cmd, &["mcp"])
}

/// Drop `mcpServers.rtok` from `~/.claude.json` (`rtok agents remove claude`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    rtok_agent_sdk::unregister_mcp(&apply(cfg), &cfg.doctor.claude_json, "rtok")
}

/// The plugin tree (T114) and its id in the one-plugin marketplace that tree also is.
const PLUGIN_SRC: &str = "plugins/claude";
/// `doctor::plugin_hooks` (T173) also names this id when it walks
/// `installed_plugins.json` for the hooks the installed plugin carries.
pub(crate) const PLUGIN_ID: &str = "rtok@rtok";

/// GitHub `owner/repo` shorthand `claude plugin marketplace add` resolves (T139): the repo
/// root's `.claude-plugin/marketplace.json` names this one marketplace `rtok`, whose only
/// plugin is `./plugins/claude` (relative to the repo root, not the marketplace file). A
/// local path broke across a ketch upgrade (`store/rtok/vX.Y.Z/…`); GitHub does not move.
const MARKETPLACE_REPO: &str = "listepo/rtok";

/// Claude Code's config dir: the one `settings_path` lives in (`~/.claude`).
/// `pub(crate)`: `doctor::plugin_hooks` (T173) locates `installed_plugins.json` the
/// same way `plugin_installed` does.
pub(crate) fn config_dir(cfg: &Config) -> PathBuf {
    let s = &cfg.setup.claude.settings_path;
    s.parent().map(PathBuf::from).unwrap_or_else(|| s.clone())
}

/// The modules rtok's hook and MCP registrations carry in the two FILES a foreign importer
/// reads (`~/.claude/settings.json`, `~/.claude.json`) — the Claude plugin serves its own
/// and is not visible there, so Grok's `[compat.claude]` import counts these alone (T100).
pub fn files_serve_rtok(cfg: &Config) -> Vec<&'static str> {
    let s = super::read(&cfg.setup.claude.settings_path);
    let m = super::read(&cfg.doctor.claude_json);
    let mut out = Vec::new();
    if s.contains("rtok hook") {
        out.push("hooks");
    }
    if m.contains("\"rtok\"") {
        out.push("mcp");
    }
    out
}

/// True when Claude Code lists `rtok@rtok` as installed. Read from its own record, so a
/// plugin removed through `/plugin` stops counting at once (T75). `pub(crate)`: also the
/// install check `doctor::plugin_hooks` uses (T173), so the two never drift apart.
pub(crate) fn plugin_installed(cfg: &Config) -> bool {
    super::read(&config_dir(cfg).join("plugins/installed_plugins.json"))
        .contains(&format!("\"{PLUGIN_ID}\""))
}

/// What Claude Code's `known_marketplaces.json` says about the `rtok` marketplace.
#[derive(PartialEq, Eq)]
enum MarketplaceState {
    /// No `"rtok"` entry at all.
    Absent,
    /// Points at the GitHub repo `MARKETPLACE_REPO` already — T139's own shape,
    /// `{"source": {"source": "github", "repo": "listepo/rtok"}}` (verified against the
    /// Claude Code plugin-marketplaces docs).
    Github,
    /// A `"rtok"` entry exists but not with that shape — in practice the pre-T139 local
    /// ketch-store path, `{"source": {"source": "directory", "path": ".../store/rtok/vX.Y.Z/…"}}`
    /// (the literal shape a real `~/.claude/plugins/known_marketplaces.json` on this machine
    /// carried before this fix). `marketplace add` on top of it errors instead of re-pointing,
    /// so it must be removed first.
    Stale,
}

/// Parses the same file `plugin_installed` reads (T139 follow-up: a plain
/// `.contains("\"rtok\":")` could not tell the GitHub source from a stale local one, so a user
/// still holding the pre-T139 local marketplace kept installing from it forever — `marketplace
/// add` was skipped because *some* `"rtok"` entry existed, never checking what it pointed at).
fn marketplace_state(cfg: &Config) -> MarketplaceState {
    let text = super::read(&config_dir(cfg).join("plugins/known_marketplaces.json"));
    let Ok(root) = serde_json::from_str::<Value>(&text) else {
        return MarketplaceState::Absent;
    };
    let Some(entry) = root.get("rtok") else {
        return MarketplaceState::Absent;
    };
    let source = entry.get("source");
    let is_github = source.and_then(|s| s.get("source")).and_then(Value::as_str) == Some("github")
        && source.and_then(|s| s.get("repo")).and_then(Value::as_str) == Some(MARKETPLACE_REPO);
    if is_github {
        MarketplaceState::Github
    } else {
        MarketplaceState::Stale
    }
}

/// One `claude plugin …` call; `CLAUDE_CONFIG_DIR` only when `settings_path` is not the
/// default. Windows shim resolution and stderr handling live in `super::run_cli` (T140), so
/// Codex's own `codex_cli` reuses the same spawn logic instead of respelling it.
fn claude_cli(cfg: &Config, args: &[&str]) -> std::result::Result<(), String> {
    let dir = config_dir(cfg);
    let default = super::home_dir().join(".claude");
    let env = (dir != default).then_some(("CLAUDE_CONFIG_DIR", dir.as_path()));
    super::run_cli("claude", args, env)
}

/// Offer, install, or uninstall the plugin through the official `claude plugin` commands
/// (T115), from the GitHub marketplace `listepo/rtok` (T139). Installed by default — no
/// `--yes` needed — once `claude` is on PATH and the plugin is not already installed;
/// already installed from the GitHub marketplace (or already removed) is a no-op.
/// `marketplace add` is skipped once Claude already knows the *GitHub* marketplace, so a
/// rerun never errors; a `"rtok"` marketplace known under any other source (the pre-T139
/// local ketch-store path) is re-pointed — `marketplace remove` then `add` — instead of
/// silently installing from the stale path forever. A failing or missing `claude` keeps the
/// offer open instead of failing the install: the settings-file hooks still go in.
///
/// `installed_plugins.json` (verified against the real file this machine wrote under the
/// pre-T139 flow) carries no field naming which marketplace a plugin came from — only a
/// cache path, install time and version — so there is no direct signal to gate a forced
/// reinstall on. The `rtok` marketplace's own recorded source is used instead: `rtok@rtok`
/// can only ever have come from whatever the one marketplace named `rtok` pointed to, so a
/// `Stale` marketplace state is reason enough to uninstall and reinstall even when the
/// plugin already shows as installed. An `Absent` marketplace with the plugin already
/// installed is left alone (ambiguous, not evidence of anything stale).
fn plugin(cfg: &Config, remove: bool) -> Result<String> {
    let a = apply(cfg);
    let installed = plugin_installed(cfg);
    let state = marketplace_state(cfg);
    if remove {
        if !installed {
            return Ok(NO_CHANGES.into());
        }
    } else if installed && state != MarketplaceState::Stale {
        return Ok(NO_CHANGES.into());
    }
    let steps: Vec<&[&str]> = if remove {
        vec![
            &["plugin", "uninstall", PLUGIN_ID],
            &["plugin", "marketplace", "remove", "rtok"],
        ]
    } else {
        let mut v: Vec<&[&str]> = Vec::new();
        if state == MarketplaceState::Stale {
            if installed {
                v.push(&["plugin", "uninstall", PLUGIN_ID]);
            }
            v.push(&["plugin", "marketplace", "remove", "rtok"]);
        }
        if state != MarketplaceState::Github {
            v.push(&["plugin", "marketplace", "add", MARKETPLACE_REPO]);
        }
        v.push(&["plugin", "install", PLUGIN_ID]);
        v
    };
    let shown = steps
        .iter()
        .map(|s| format!("claude {}", s.join(" ")))
        .collect::<Vec<_>>()
        .join(" && ");
    if a.dry_run {
        return Ok(if remove {
            format!("- plugin {PLUGIN_ID} ({shown})")
        } else {
            format!("offer {PLUGIN_SRC} → {shown} {KETCH_INSTALL}")
        });
    }
    if !remove && super::find_on_path("claude").is_none() {
        return Ok(format!(
            "offer {PLUGIN_SRC} → {shown} (claude failed: claude not found on PATH) {KETCH_INSTALL}"
        ));
    }
    for step in steps {
        if let Err(e) = claude_cli(cfg, step) {
            return Ok(format!("offer {PLUGIN_SRC} → {shown} (claude failed: {e})"));
        }
    }
    Ok(if remove {
        format!("- plugin {PLUGIN_ID}")
    } else {
        format!("+ plugin {PLUGIN_SRC} → {PLUGIN_ID}")
    })
}

/// `agents update` over a plugin already installed from the GitHub marketplace (T242.3):
/// refresh the marketplace and `claude plugin update` in place; if either step fails,
/// reinstall (`uninstall` + `install`). No `--yes`: accepting a changed marketplace-declared
/// command stays the user's call, and a non-TTY refusal just takes the reinstall path.
/// Claude's own `installed_plugins.json` is the evidence — unchanged bytes read as
/// [`NO_CHANGES`], so an update that found nothing new says `already current`.
fn plugin_update(cfg: &Config) -> Result<String> {
    const UPDATE: [&[&str]; 2] = [
        &["plugin", "marketplace", "update", "rtok"],
        &["plugin", "update", PLUGIN_ID],
    ];
    const REINSTALL: [&[&str]; 2] = [
        &["plugin", "uninstall", PLUGIN_ID],
        &["plugin", "install", PLUGIN_ID],
    ];
    let shown = |steps: &[&[&str]]| {
        steps
            .iter()
            .map(|s| format!("claude {}", s.join(" ")))
            .collect::<Vec<_>>()
            .join(" && ")
    };
    if apply(cfg).dry_run {
        return Ok(format!("~ plugin {PLUGIN_ID} ({})", shown(&UPDATE)));
    }
    if super::find_on_path("claude").is_none() {
        return Ok(NO_CHANGES.into());
    }
    let record = config_dir(cfg).join("plugins/installed_plugins.json");
    let before = super::read(&record);
    let Err(e) = UPDATE.iter().try_for_each(|s| claude_cli(cfg, s)) else {
        return Ok(if super::read(&record) == before {
            NO_CHANGES.into()
        } else {
            format!("~ plugin {PLUGIN_ID} updated")
        });
    };
    match REINSTALL.iter().try_for_each(|s| claude_cli(cfg, s)) {
        Ok(()) => Ok(format!(
            "~ plugin {PLUGIN_ID} reinstalled (update failed: {e})"
        )),
        Err(e2) => Ok(format!(
            "offer {PLUGIN_SRC} → {} (claude failed: {e2})",
            shown(&REINSTALL)
        )),
    }
}

/// Where Claude Desktop reads `mcpServers`: its own file, not `~/.claude.json`.
pub fn desktop_path() -> PathBuf {
    let home = super::home_dir();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Claude/claude_desktop_config.json")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("Claude/claude_desktop_config.json")
    } else {
        home.join(".config/Claude/claude_desktop_config.json")
    }
}

/// A desktop app starts from the Dock or Start menu without a shell PATH, so a bare `rtok`
/// does not spawn there: write the absolute binary (Claude Desktop, ZCode).
pub(super) fn desktop_command() -> String {
    std::env::current_exe()
        .map(|exe| dunce::simplified(&exe).to_string_lossy().into_owned())
        .unwrap_or_else(|_| super::rtok_command())
}

/// Claude Code: hooks in `settings.json`, MCP in `~/.claude.json`, the proxy as
/// `env.ANTHROPIC_BASE_URL`. Claude Desktop: MCP only, in `claude_desktop_config.json`.
pub struct Claude;

static VARIANTS: [Variant; 2] = [
    Variant {
        kind: Kind::Cli,
        name: "Claude Code",
        bins: &["claude"],
        apps: &[],
    },
    Variant {
        kind: Kind::Desktop,
        name: "Claude Desktop",
        bins: &[],
        apps: &[
            "/Applications/Claude.app",
            "$LOCALAPPDATA/AnthropicClaude/claude.exe",
        ],
    },
];

impl Agent for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, kind: Kind, module: &str) -> Support {
        match (kind, module) {
            (_, "mcp") | (Kind::Cli, "hooks") => Support::Yes,
            (Kind::Cli, "proxy") => Support::Flag("--proxy"),
            // `plugin`: `plugins/claude` through `claude plugin install`, from the GitHub
            // marketplace `listepo/rtok`, installed by default once `claude` is on PATH
            // and not already installed (T139).
            (Kind::Cli, _) => Support::Yes,
            (Kind::Desktop, "hooks") => Support::No("Claude Desktop has no hook events"),
            (Kind::Desktop, "proxy") => Support::No(
                "Claude Desktop has no base-URL setting; its requests do not pass through the proxy",
            ),
            (Kind::Desktop, _) => Support::No(
                "Claude Desktop loads MCP from claude_desktop_config.json; there is no plugin directory to link",
            ),
        }
    }

    fn files(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        match kind {
            Kind::Desktop => vec![desktop_path()],
            Kind::Cli => vec![
                cfg.setup.claude.settings_path.clone(),
                cfg.doctor.claude_json.clone(),
            ],
        }
    }

    fn installed(&self, cfg: &Config, kind: Kind) -> Vec<&'static str> {
        if kind == Kind::Desktop {
            return if super::read(&desktop_path()).contains("\"rtok\"") {
                vec!["mcp"]
            } else {
                vec![]
            };
        }
        let s = super::read(&cfg.setup.claude.settings_path);
        // The installed plugin serves the hooks and the MCP itself (D21).
        let plugin = plugin_installed(cfg);
        let files = files_serve_rtok(cfg);
        let mut out = Vec::new();
        if files.contains(&"hooks") || plugin {
            out.push("hooks");
        }
        if files.contains(&"mcp") || plugin {
            out.push("mcp");
        }
        // The URL `register_proxy` writes. Matching the default port `8790` anywhere in the
        // file missed a proxy on another `[proxy] port`.
        let base = serde_json::from_str::<Value>(&s)
            .ok()
            .and_then(|v| v["env"]["ANTHROPIC_BASE_URL"].as_str().map(str::to_string));
        if base.as_deref() == Some(super::anthropic_proxy_url(cfg).as_str()) {
            out.push("proxy");
        }
        if plugin {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        if kind == Kind::Desktop {
            let (a, path) = (apply(cfg), desktop_path());
            // `--replace` is about Claude Code's hooks; on the desktop it is a plain install.
            return Ok(vec![
                if remove {
                    rtok_agent_sdk::unregister_mcp(&a, &path, "rtok")?
                } else {
                    rtok_agent_sdk::register_mcp(&a, &path, "rtok", &desktop_command(), &["mcp"])?
                },
                super::skill::sync("claude", cfg, remove)?,
            ]);
        }
        match mode {
            Mode::Replace => Ok(vec![
                migrate::run(cfg)?,
                super::skill::sync("claude", cfg, false)?,
            ]),
            Mode::Remove => Ok(vec![
                plugin(cfg, true)?,
                run(cfg, true)?,
                unregister_mcp(cfg)?,
                crate::proxy::cli::unregister_proxy(cfg)?,
                super::skill::sync("claude", cfg, true)?,
            ]),
            Mode::Install | Mode::Update => {
                // Offer first: once the plugin is installed it is the only call path (D21
                // singleton), so the settings-file hooks and MCP are stripped, not added —
                // judged by Claude's own record, so a dry run or a declined offer still gets
                // the settings-file install.
                let current = mode == Mode::Update
                    && plugin_installed(cfg)
                    && marketplace_state(cfg) == MarketplaceState::Github;
                let mut lines = vec![if current {
                    plugin_update(cfg)?
                } else {
                    plugin(cfg, false)?
                }];
                if plugin_installed(cfg) {
                    lines.push(run(cfg, true)?);
                    lines.push(unregister_mcp(cfg)?);
                } else {
                    lines.push(run(cfg, false)?);
                    if cfg.setup.mcp {
                        lines.push(register_mcp(cfg)?);
                    }
                }
                if cfg.setup.proxy {
                    lines.push(crate::proxy::cli::register_proxy(cfg)?);
                }
                lines.push(super::skill::sync("claude", cfg, false)?);
                Ok(lines)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(path: std::path::PathBuf, dry: bool) -> Config {
        let mut c = Config::default();
        c.setup.claude.settings_path = path;
        c.setup.dry_run = dry;
        c.setup.backup = false;
        c
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    #[test]
    fn dry_run_empty_is_nine_additions() {
        let path = tmp("setup-dry");
        let report = run(&cfg(path.clone(), true), false).unwrap();
        assert!(report.contains("9 additions"), "{report}");
        assert!(
            report.contains("+ SessionEnd rtok hook SessionEnd"),
            "{report}"
        );
        assert!(!path.exists());
    }

    #[test]
    fn mcp_dry_run_then_apply_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("rtok-mcp-setup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("claude.json");
        let mut c = Config::default();
        c.doctor.claude_json = path.clone();
        c.setup.backup = false;
        c.setup.dry_run = true;
        let dry = register_mcp(&c).unwrap();
        assert!(dry.contains("mcpServers.rtok"), "{dry}");
        assert!(!path.exists());
        c.setup.dry_run = false;
        assert!(register_mcp(&c).unwrap().contains("rtok mcp"));
        assert_eq!(register_mcp(&c).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"mcp\""), "{raw}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_twice_then_remove_keeps_foreign() {
        let path = tmp("setup-apply");
        fs::write(&path, r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo other"}]}]}}"#).unwrap();
        let first = run(&cfg(path.clone(), false), false).unwrap();
        assert!(first.contains("9 additions"), "{first}");
        assert_eq!(run(&cfg(path.clone(), false), false).unwrap(), NO_CHANGES);
        let rm = run(&cfg(path.clone(), false), true).unwrap();
        assert!(rm.contains("removed"), "{rm}");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("echo other") && !raw.contains("rtok hook"));
    }

    /// A user command that merely contains `rtok hook <event>` is theirs: remove keeps it.
    #[test]
    fn remove_keeps_a_user_command_that_chains_rtok() {
        let path = tmp("setup-chain");
        let chain = "notify-send hi && rtok hook PreToolUse";
        fs::write(
            &path,
            json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":chain}]}]}})
                .to_string(),
        )
        .unwrap();
        run(&cfg(path.clone(), false), false).unwrap();
        run(&cfg(path.clone(), false), true).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(chain), "{raw}");
        assert!(!raw.contains("\"rtok hook PreToolUse\""), "{raw}");
    }

    /// The desktop file lives under the platform's app-support dir and gets the absolute
    /// binary, since the app has no shell PATH. Remove takes it back out.
    #[test]
    fn desktop_writes_absolute_rtok_into_claude_desktop_config() {
        let p = desktop_path();
        assert!(
            p.ends_with("Claude/claude_desktop_config.json"),
            "{}",
            p.display()
        );
        if cfg!(target_os = "macos") {
            assert!(p.to_string_lossy().contains("Library/Application Support"));
        } else if !cfg!(windows) {
            assert!(p.to_string_lossy().contains("/.config/"));
        }
        assert!(std::path::Path::new(&desktop_command()).is_absolute());
        assert_eq!(Claude.support(Kind::Desktop, "mcp"), Support::Yes);
        assert!(matches!(
            Claude.support(Kind::Desktop, "hooks"),
            Support::No(_)
        ));
        // Register/unregister through the SDK at a temp path, the way `apply` does.
        let path = tmp("desktop-mcp");
        let a = apply(&cfg(path.clone(), false));
        rtok_agent_sdk::register_mcp(&a, &path, "rtok", &desktop_command(), &["mcp"]).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(&desktop_command()), "{raw}");
        assert_ne!(
            rtok_agent_sdk::unregister_mcp(&a, &path, "rtok").unwrap(),
            NO_CHANGES
        );
        assert!(!fs::read_to_string(&path).unwrap().contains("\"rtok\""));
    }

    /// Windows keeps the desktop file under `%APPDATA%\Claude`, not under the profile root.
    #[cfg(windows)]
    #[test]
    fn desktop_path_lives_under_appdata_on_windows() {
        let appdata = std::env::var_os("APPDATA").expect("APPDATA");
        assert!(
            desktop_path().starts_with(&appdata),
            "{}",
            desktop_path().display()
        );
        assert!(desktop_command().ends_with(".exe"), "{}", desktop_command());
    }

    // --- Vfs twins (T56.3): hook insert/strip against in-memory fixtures; keep disk e2e ---

    fn hooks_roundtrip_vfs(vfs: &mut crate::testutil::Vfs, path: &str, remove: bool) -> String {
        let raw = vfs.read_str(path).unwrap_or("{}");
        let mut root: Value =
            serde_json::from_str(if raw.is_empty() { "{}" } else { raw }).unwrap();
        let report = if remove {
            strip_ours(root.get_mut("hooks"))
        } else {
            insert_ours(object_at(&mut root, "hooks"), ENTRIES, "rtok", "timeout", 5)
        };
        vfs.write(path, serde_json::to_string_pretty(&root).unwrap());
        report
    }

    #[test]
    fn apply_twice_then_remove_keeps_foreign_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        let path = "settings.json";
        vfs.write(
            path,
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo other"}]}]}}"#,
        );
        let first = hooks_roundtrip_vfs(&mut vfs, path, false);
        assert!(first.contains("9 additions"), "{first}");
        assert_eq!(hooks_roundtrip_vfs(&mut vfs, path, false), NO_CHANGES);
        let rm = hooks_roundtrip_vfs(&mut vfs, path, true);
        assert!(rm.contains("removed"), "{rm}");
        let raw = vfs.read_str(path).unwrap();
        assert!(
            raw.contains("echo other") && !raw.contains("rtok hook"),
            "{raw}"
        );
    }

    #[test]
    fn remove_keeps_a_user_command_that_chains_rtok_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        let path = "settings.json";
        let chain = "notify-send hi && rtok hook PreToolUse";
        vfs.write(
            path,
            json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":chain}]}]}})
                .to_string(),
        );
        hooks_roundtrip_vfs(&mut vfs, path, false);
        hooks_roundtrip_vfs(&mut vfs, path, true);
        let raw = vfs.read_str(path).unwrap();
        assert!(raw.contains(chain), "{raw}");
        assert!(!raw.contains("\"rtok hook PreToolUse\""), "{raw}");
    }

    #[test]
    fn wrong_shaped_hooks_key_is_replaced_from_vfs() {
        for body in [
            r#"{"hooks":[]}"#,
            r#"{"hooks":"nope"}"#,
            r#"{"hooks":{"PreToolUse":"nope"}}"#,
        ] {
            let mut vfs = crate::testutil::Vfs::new();
            let path = "settings.json";
            vfs.write(path, body);
            let report = hooks_roundtrip_vfs(&mut vfs, path, false);
            assert!(report.contains("9 additions"), "{body} → {report}");
            let root: Value = serde_json::from_str(vfs.read_str(path).unwrap()).unwrap();
            assert!(root["hooks"]["PreToolUse"].is_array(), "{body}");
        }
    }

    #[test]
    fn dry_run_empty_is_nine_additions_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        // Absent file → empty object; dry-run style: mutate report only, do not require prior write.
        let path = "Users/Ivan Tuhai/.claude/settings.json";
        let report = hooks_roundtrip_vfs(&mut vfs, path, false);
        assert!(report.contains("9 additions"), "{report}");
        assert!(
            report.contains("+ SessionEnd rtok hook SessionEnd"),
            "{report}"
        );
        // Vfs now holds the written body (unlike disk dry_run); assert shape instead of absence.
        let root: Value = serde_json::from_str(vfs.read_str(path).unwrap()).unwrap();
        assert!(root["hooks"]["SessionEnd"].is_array());
    }

    /// A settings file whose `hooks` is not an object is user data, not a reason to panic
    /// (`hooks: []` used to hit `as_object_mut().unwrap()`).
    #[test]
    fn wrong_shaped_hooks_key_is_replaced_not_panicked_on() {
        for body in [
            r#"{"hooks":[]}"#,
            r#"{"hooks":"nope"}"#,
            r#"{"hooks":{"PreToolUse":"nope"}}"#,
        ] {
            let path = tmp(&format!("setup-shape-{}", body.len()));
            fs::write(&path, body).unwrap();
            let report = run(&cfg(path.clone(), false), false).unwrap();
            assert!(report.contains("9 additions"), "{body} → {report}");
            let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert!(root["hooks"]["PreToolUse"].is_array(), "{body}");
        }
    }

    /// T242.1: an rtok hook from an older binary path or timeout is rewritten in its own slot,
    /// the foreign hook beside it stays, and a second pass changes nothing.
    #[test]
    fn stale_bin_and_timeout_are_rewritten_in_place() {
        let mut hooks = json!({"PreToolUse":[{"matcher":"Bash","hooks":[
            {"type":"command","command":"echo mine"},
            {"type":"command","command":"/old/store/rtok/v0.1.0/rtok hook PreToolUse","timeout":3}
        ]}]});
        let report = insert_ours(&mut hooks, &ENTRIES[..1], "rtok", "timeout", 7);
        assert_eq!(
            report, "~ PreToolUse Bash rtok hook PreToolUse\n1 updates",
            "{report}"
        );
        let inner = &hooks["PreToolUse"][0]["hooks"];
        assert_eq!(inner[0]["command"], "echo mine");
        assert_eq!(inner[1]["command"], "rtok hook PreToolUse");
        assert_eq!(inner[1]["timeout"], 7);
        assert_eq!(hooks["PreToolUse"].as_array().unwrap().len(), 1);
        let before = hooks.clone();
        assert_eq!(
            insert_ours(&mut hooks, &ENTRIES[..1], "rtok", "timeout", 7),
            NO_CHANGES
        );
        assert_eq!(hooks, before);
    }

    /// T242.1: a pair rtok no longer installs (an old matcher, a dropped event) loses its rtok
    /// hook — foreign hooks on it stay — and the current pair is added, so one event never
    /// ends up with two rtok hooks.
    #[test]
    fn a_pair_rtok_no_longer_installs_is_pruned() {
        let mut hooks = json!({
            "PostToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"rtok hook PostToolUse"}]}],
            "Stop":[{"hooks":[
                {"type":"command","command":"rtok hook Stop"},
                {"type":"command","command":"notify-send done"}
            ]}],
            "Notification":[{"hooks":[{"type":"command","command":"rtok hook Notification"}]}]
        });
        let entries = [("PostToolUse", "*")];
        let report = insert_ours(&mut hooks, &entries, "rtok", "timeout", 5);
        assert!(report.ends_with("1 additions, 3 removals"), "{report}");
        assert!(
            report.contains("- PostToolUse Bash rtok hook PostToolUse"),
            "{report}"
        );
        let post = hooks["PostToolUse"].as_array().unwrap();
        assert_eq!(post.len(), 1, "{hooks}");
        assert_eq!(post[0]["matcher"], "*");
        assert_eq!(
            hooks["Stop"][0]["hooks"],
            json!([{"type":"command","command":"notify-send done"}])
        );
        assert!(hooks.get("Notification").is_none(), "{hooks}");
    }

    /// T242.1: re-running install on a current file writes nothing — the bytes stay.
    #[test]
    fn current_file_stays_byte_identical() {
        let path = tmp("setup-current");
        let c = cfg(path.clone(), false);
        run(&c, false).unwrap();
        let first = fs::read(&path).unwrap();
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        assert_eq!(fs::read(&path).unwrap(), first);
    }

    /// T114: `plugins/claude` carries the installer's hooks, one `rtok mcp`, and the marketplace
    /// the directory is its own marketplace (`claude plugin marketplace add plugins/claude`).
    #[test]
    fn plugin_tree_matches_the_installer() {
        let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap();
        let hooks = parse(include_str!("../../../plugins/claude/hooks/hooks.json"));
        let timeout = Config::default().setup.hook_timeout_s;
        let mut want = json!({});
        for &(event, matcher) in ENTRIES {
            // T178: `rtok` on PATH is exec'd from Claude Code's own shell; `hook.sh` (a second
            // shell, ~6 ms) only runs when PATH has no `rtok` (desktop app, fail-open hint).
            let cmd = format!(
                "command -v rtok >/dev/null 2>&1 && exec rtok hook {event}; \
                 exec \"${{CLAUDE_PLUGIN_ROOT}}/scripts/hook.sh\" {event}"
            );
            let mut e = json!({"hooks": [{"type": "command", "command": cmd, "timeout": timeout}]});
            if !matcher.is_empty() {
                e["matcher"] = json!(matcher);
            }
            array_at(&mut want, event).push(e);
        }
        assert_eq!(hooks["hooks"], want);
        let mcp = parse(include_str!("../../../plugins/claude/.mcp.json"));
        assert_eq!(
            mcp["mcpServers"],
            json!({"rtok": {"command": "${CLAUDE_PLUGIN_ROOT}/scripts/mcp.sh"}})
        );
        let manifest = parse(include_str!(
            "../../../plugins/claude/.claude-plugin/plugin.json"
        ));
        let market = parse(include_str!(
            "../../../plugins/claude/.claude-plugin/marketplace.json"
        ));
        assert_eq!(market["plugins"][0]["name"], manifest["name"]);
        assert_eq!(market["plugins"][0]["source"], "./");
    }

    // --- T139: `plugin()` decision logic — installed/known-marketplace/fresh, no real `claude` ---

    fn plugin_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("plugins")).unwrap();
        dir
    }

    fn plugin_cfg(dir: &std::path::Path, dry: bool) -> Config {
        let mut c = Config::default();
        c.setup.claude.settings_path = dir.join("settings.json");
        c.setup.dry_run = dry;
        c.setup.backup = false;
        c
    }

    /// Already installed → no-op, whether or not `--dry-run` is given.
    #[test]
    fn plugin_install_is_a_no_op_once_claude_already_has_it() {
        let dir = plugin_dir("plugin-installed");
        fs::write(
            dir.join("plugins/installed_plugins.json"),
            r#"{"rtok@rtok":{}}"#,
        )
        .unwrap();
        assert_eq!(plugin(&plugin_cfg(&dir, false), false).unwrap(), NO_CHANGES);
        assert_eq!(plugin(&plugin_cfg(&dir, true), false).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    /// Not installed and not removing it either → no-op (nothing to uninstall).
    #[test]
    fn plugin_remove_is_a_no_op_when_not_installed() {
        let dir = plugin_dir("plugin-remove-noop");
        assert_eq!(plugin(&plugin_cfg(&dir, false), true).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(dir);
    }

    /// A marketplace already pointed at the GitHub repo skips `marketplace add`: only the
    /// install step is offered. Shape verified against a real `known_marketplaces.json`
    /// this machine wrote for `claude plugin marketplace add listepo/rtok`.
    #[test]
    fn plugin_dry_run_skips_marketplace_add_when_already_known() {
        let dir = plugin_dir("plugin-known-market");
        fs::write(
            dir.join("plugins/known_marketplaces.json"),
            r#"{"rtok":{"source":{"source":"github","repo":"listepo/rtok"}}}"#,
        )
        .unwrap();
        let report = plugin(&plugin_cfg(&dir, true), false).unwrap();
        assert!(
            report.contains("claude plugin install rtok@rtok"),
            "{report}"
        );
        assert!(!report.contains("marketplace add"), "{report}");
        assert!(!report.contains("marketplace remove"), "{report}");
        let _ = fs::remove_dir_all(dir);
    }

    /// Nothing known yet: both steps are offered, from the GitHub marketplace, not a local path.
    #[test]
    fn plugin_dry_run_offers_both_steps_from_a_clean_install() {
        let dir = plugin_dir("plugin-fresh");
        let report = plugin(&plugin_cfg(&dir, true), false).unwrap();
        assert!(
            report.contains("claude plugin marketplace add listepo/rtok"),
            "{report}"
        );
        assert!(
            report.contains("claude plugin install rtok@rtok"),
            "{report}"
        );
        assert!(report.starts_with("offer plugins/claude → "), "{report}");
        let _ = fs::remove_dir_all(dir);
    }

    /// A `"rtok"` marketplace registered under the pre-T139 local ketch-store path (the
    /// literal shape a real `known_marketplaces.json` on this machine carried) is removed
    /// before it is re-added from GitHub — `marketplace add` on top of an existing entry
    /// errors, it does not re-point it.
    #[test]
    fn plugin_dry_run_repoints_a_stale_local_marketplace() {
        let dir = plugin_dir("plugin-stale-market");
        fs::write(
            dir.join("plugins/known_marketplaces.json"),
            r#"{"rtok":{"source":{"source":"directory","path":"/Users/x/.ketch/store/rtok/v0.6.3/plugins/claude"},"installLocation":"/Users/x/.ketch/store/rtok/v0.6.3/plugins/claude"}}"#,
        )
        .unwrap();
        let report = plugin(&plugin_cfg(&dir, true), false).unwrap();
        assert!(
            report.contains("claude plugin marketplace remove rtok && claude plugin marketplace add listepo/rtok && claude plugin install rtok@rtok"),
            "{report}"
        );
        assert!(!report.contains("uninstall"), "{report}");
        let _ = fs::remove_dir_all(dir);
    }

    /// The plugin already shows as installed, but `installed_plugins.json` carries no field
    /// naming its marketplace (verified against a real file this machine wrote) — the stale
    /// local marketplace is the only signal there is, so the plugin is uninstalled and
    /// reinstalled from GitHub instead of the usual already-installed no-op.
    #[test]
    fn plugin_dry_run_reinstalls_when_already_installed_from_a_stale_marketplace() {
        let dir = plugin_dir("plugin-stale-installed");
        fs::write(
            dir.join("plugins/installed_plugins.json"),
            r#"{"rtok@rtok":{}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("plugins/known_marketplaces.json"),
            r#"{"rtok":{"source":{"source":"directory","path":"/Users/x/.ketch/store/rtok/v0.6.3/plugins/claude"}}}"#,
        )
        .unwrap();
        let report = plugin(&plugin_cfg(&dir, true), false).unwrap();
        assert_ne!(report, NO_CHANGES);
        assert!(
            report.contains("claude plugin uninstall rtok@rtok && claude plugin marketplace remove rtok && claude plugin marketplace add listepo/rtok && claude plugin install rtok@rtok"),
            "{report}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
