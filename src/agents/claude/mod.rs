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

/// Add `<bin> hook <event>` under `hooks.<event>[]` for each entry not already ours.
/// `timeout_key` is the host's spelling (`timeout` seconds in Claude, `timeoutMs` in ZCode).
pub(super) fn insert_ours(
    hooks: &mut Value,
    entries: &[(&str, &str)],
    bin: &str,
    timeout_key: &str,
    timeout: u64,
) -> String {
    let mut added = Vec::new();
    for &(event, matcher) in entries {
        if array_at(hooks, event)
            .iter()
            .any(|e| has_ours(e, event, matcher))
        {
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
        let m = if matcher.is_empty() {
            String::new()
        } else {
            format!(" {matcher}")
        };
        added.push(format!("+ {event}{m} {}", command(bin, event)));
    }
    if added.is_empty() {
        NO_CHANGES.into()
    } else {
        format!("{}\n{} additions", added.join("\n"), added.len())
    }
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
const PLUGIN_ID: &str = "rtok@rtok";

/// GitHub `owner/repo` shorthand `claude plugin marketplace add` resolves (T139): the repo
/// root's `.claude-plugin/marketplace.json` names this one marketplace `rtok`, whose only
/// plugin is `./plugins/claude` (relative to the repo root, not the marketplace file). A
/// local path broke across a ketch upgrade (`store/rtok/vX.Y.Z/…`); GitHub does not move.
const MARKETPLACE_REPO: &str = "listepo/rtok";

/// Claude Code's config dir: the one `settings_path` lives in (`~/.claude`).
fn config_dir(cfg: &Config) -> PathBuf {
    let s = &cfg.setup.claude.settings_path;
    s.parent().map(PathBuf::from).unwrap_or_else(|| s.clone())
}

/// True when Claude Code lists `rtok@rtok` as installed. Read from its own record, so a
/// plugin removed through `/plugin` stops counting at once (T75).
pub(super) fn plugin_installed(cfg: &Config) -> bool {
    super::read(&config_dir(cfg).join("plugins/installed_plugins.json"))
        .contains(&format!("\"{PLUGIN_ID}\""))
}

/// True when Claude Code already knows the `rtok` marketplace. Read the same way
/// `plugin_installed` reads its own file, so a rerun skips `marketplace add` instead of
/// erroring on a marketplace that already exists (T139).
fn marketplace_known(cfg: &Config) -> bool {
    super::read(&config_dir(cfg).join("plugins/known_marketplaces.json")).contains("\"rtok\":")
}

/// One `claude plugin …` call; `CLAUDE_CONFIG_DIR` only when `settings_path` is not the default.
fn claude_cli(cfg: &Config, args: &[&str]) -> std::result::Result<(), String> {
    let mut cmd = std::process::Command::new("claude");
    cmd.args(args).stdin(std::process::Stdio::null());
    let dir = config_dir(cfg);
    if dir != super::home_dir().join(".claude") {
        cmd.env("CLAUDE_CONFIG_DIR", &dir);
    }
    match cmd.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr)
            .lines()
            .next()
            .unwrap_or("non-zero exit")
            .to_string()),
        Err(e) => Err(format!("claude: {e}")),
    }
}

/// Offer, install, or uninstall the plugin through the official `claude plugin` commands
/// (T115), from the GitHub marketplace `listepo/rtok` (T139). Installed by default — no
/// `--yes` needed — once `claude` is on PATH and the plugin is not already installed;
/// already installed (or already removed) is a no-op. `marketplace add` is skipped once
/// Claude already knows the marketplace, so a rerun never errors, and a failing or missing
/// `claude` keeps the offer open instead of failing the install: the settings-file hooks
/// still go in.
fn plugin(cfg: &Config, remove: bool) -> Result<String> {
    let a = apply(cfg);
    if remove != plugin_installed(cfg) {
        return Ok(NO_CHANGES.into());
    }
    let steps: Vec<&[&str]> = if remove {
        vec![
            &["plugin", "uninstall", PLUGIN_ID],
            &["plugin", "marketplace", "remove", "rtok"],
        ]
    } else if marketplace_known(cfg) {
        vec![&["plugin", "install", PLUGIN_ID]]
    } else {
        vec![
            &["plugin", "marketplace", "add", MARKETPLACE_REPO],
            &["plugin", "install", PLUGIN_ID],
        ]
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
        let m = super::read(&cfg.doctor.claude_json);
        // The installed plugin serves the hooks and the MCP itself (D21).
        let plugin = plugin_installed(cfg);
        let mut out = Vec::new();
        if s.contains("rtok hook") || plugin {
            out.push("hooks");
        }
        if m.contains("\"rtok\"") || plugin {
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
            Mode::Install => {
                // Offer first: once the plugin is installed it is the only call path (D21
                // singleton), so the settings-file hooks and MCP are stripped, not added —
                // judged by Claude's own record, so a dry run or a declined offer still gets
                // the settings-file install.
                let mut lines = vec![plugin(cfg, false)?];
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

    /// T114: `plugins/claude` carries the installer's hooks, one `rtok mcp`, and the marketplace
    /// the directory is its own marketplace (`claude plugin marketplace add plugins/claude`).
    #[test]
    fn plugin_tree_matches_the_installer() {
        let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap();
        let hooks = parse(include_str!("../../../plugins/claude/hooks/hooks.json"));
        let timeout = Config::default().setup.hook_timeout_s;
        let mut want = json!({});
        for &(event, matcher) in ENTRIES {
            let cmd = format!("\"${{CLAUDE_PLUGIN_ROOT}}/scripts/hook.sh\" {event}");
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

    /// A known marketplace skips `marketplace add`: only the install step is offered.
    #[test]
    fn plugin_dry_run_skips_marketplace_add_when_already_known() {
        let dir = plugin_dir("plugin-known-market");
        fs::write(
            dir.join("plugins/known_marketplaces.json"),
            r#"{"rtok":{"source":"listepo/rtok"}}"#,
        )
        .unwrap();
        let report = plugin(&plugin_cfg(&dir, true), false).unwrap();
        assert!(
            report.contains("claude plugin install rtok@rtok"),
            "{report}"
        );
        assert!(!report.contains("marketplace add"), "{report}");
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
}
