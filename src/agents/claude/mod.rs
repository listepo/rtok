//! Install rtok hooks into Claude Code `settings.json` (plan T2.3).
//!
//! What is Claude-specific is the `hooks` shape below; backup, the write gate and the
//! `mcpServers` entry come from `rtok-agent-sdk` (D28).

pub mod migrate;

use std::path::PathBuf;

use crate::config::Config;
use anyhow::Result;
use rtok_agent_sdk::{NO_CHANGES, array_at, edit_json, object_at};
use serde_json::{Value, json};

use super::{Agent, Kind, Mode, Support, Variant, apply};

/// `(event, matcher)` — empty matcher omits the field. `SessionEnd` was missing, so no session
/// row got `ended_at` and no OTel session root ever shipped (`hooks::dispatch` handles it).
/// Hosts with a Claude-compatible hook shape (ZCode) take a prefix of this list.
pub(super) const ENTRIES: &[(&str, &str)] = &[
    ("PreToolUse", "Bash"),
    ("PreToolUse", "Read"),
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
            (Kind::Cli, _) => Support::No(
                "Claude Code loads hooks and MCP from its own settings; there is no plugin directory to link",
            ),
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
        let mut out = Vec::new();
        if s.contains("rtok hook") {
            out.push("hooks");
        }
        if m.contains("\"rtok\"") {
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
        out
    }

    fn apply(&self, cfg: &Config, kind: Kind, mode: Mode) -> Result<Vec<String>> {
        if kind == Kind::Desktop {
            let (a, path) = (apply(cfg), desktop_path());
            // `--replace` is about Claude Code's hooks; on the desktop it is a plain install.
            return Ok(vec![if mode == Mode::Remove {
                rtok_agent_sdk::unregister_mcp(&a, &path, "rtok")?
            } else {
                rtok_agent_sdk::register_mcp(&a, &path, "rtok", &desktop_command(), &["mcp"])?
            }]);
        }
        match mode {
            Mode::Replace => Ok(vec![migrate::run(cfg)?]),
            Mode::Remove => Ok(vec![
                run(cfg, true)?,
                unregister_mcp(cfg)?,
                crate::proxy::cli::unregister_proxy(cfg)?,
            ]),
            Mode::Install => {
                let mut lines = vec![run(cfg, false)?];
                if cfg.setup.mcp {
                    lines.push(register_mcp(cfg)?);
                }
                if cfg.setup.proxy {
                    lines.push(crate::proxy::cli::register_proxy(cfg)?);
                }
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
    fn dry_run_empty_is_eight_additions() {
        let path = tmp("setup-dry");
        let report = run(&cfg(path.clone(), true), false).unwrap();
        assert!(report.contains("8 additions"), "{report}");
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
        assert!(first.contains("8 additions"), "{first}");
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
            assert!(report.contains("8 additions"), "{body} → {report}");
            let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert!(root["hooks"]["PreToolUse"].is_array(), "{body}");
        }
    }
}
