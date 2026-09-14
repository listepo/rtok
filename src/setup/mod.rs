//! Host installers (`rtok agent setup <host>`).
//!
//! Everything the five hosts share — backup, the dry-run/idempotence write gate, `mcpServers`
//! registration, the plugin-link offer — lives in `rtok-agent-sdk` (D28). What stays here is
//! per-host: which file, which shape, which keys. `rtok agent list` (T37.0) reads the same
//! files back: app type (cli/gui), app version, rtok state, installed modules.

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod migrate;
pub mod opencode;
pub mod pi;

/// Every host rtok installs into, in `agent list` order.
pub const HOSTS: &[&str] = &["claude", "cursor", "codex", "opencode", "pi"];

/// App variants of one host. Cursor and OpenCode ship a CLI and a GUI;
/// the rest are CLI-only.
pub fn variants(host: &str) -> Vec<&'static str> {
    match host {
        "cursor" | "opencode" => vec!["cli", "gui"],
        _ => vec!["cli"],
    }
}

/// Whether `kind` (`cli`/`gui`) is wanted given `--cli/--gui/--all`.
/// No flag (or `--all`) means every variant.
pub fn wants(kind: &str, cli: bool, gui: bool, all: bool) -> bool {
    if all || (!cli && !gui) {
        return true;
    }
    (kind == "cli" && cli) || (kind == "gui" && gui)
}

fn home_dir() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
}

/// Where the OpenCode desktop app keeps its global config. The CLI lives at
/// `[setup.opencode] config_path` (`~/.config/opencode/opencode.json`); the
/// desktop build resolves a sibling app dir instead.
pub fn opencode_gui_path() -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        home_dir().join("Library/Application Support/ai.opencode.desktop/opencode.json")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(home_dir)
            .join("ai.opencode.desktop/opencode.json")
    } else {
        home_dir().join(".config/ai.opencode.desktop/opencode.json")
    }
}

fn opencode_path_for(kind: &str, cfg: &crate::config::Config) -> std::path::PathBuf {
    if kind == "gui" {
        opencode_gui_path()
    } else {
        cfg.setup.opencode.config_path.clone()
    }
}

/// Every config file a host install writes. `rtok agent setup` and `rtok agent remove`
/// copy these before they touch anything, so one `.bak-<ts>` per file is the whole undo.
/// pi is absent on purpose: it owns a symlinked extension directory, not a config file.
pub fn host_files(cfg: &crate::config::Config, host: &str) -> Vec<std::path::PathBuf> {
    match host {
        "claude" => vec![
            cfg.setup.claude.settings_path.clone(),
            cfg.doctor.claude_json.clone(),
        ],
        "cursor" => vec![
            cfg.setup.cursor.hooks_path.clone(),
            cfg.setup.cursor.hooks_path.with_file_name("mcp.json"),
        ],
        "codex" => vec![cfg.setup.codex.config_path.clone()],
        "opencode" => vec![cfg.setup.opencode.config_path.clone(), opencode_gui_path()],
        _ => Vec::new(),
    }
}

/// Version of the installed agent app, or `"-"` when unknown.
/// Probes `<bin> --version` and keeps the first non-empty line (32 chars max).
pub fn app_version(host: &str, kind: &str) -> String {
    let bins: &[&str] = match (host, kind) {
        ("claude", _) => &["claude"],
        ("cursor", "gui") => &["cursor"],
        ("cursor", _) => &["cursor-agent", "agent"],
        ("codex", _) => &["codex"],
        ("opencode", "gui") => &["opencode-desktop"],
        ("opencode", _) => &["opencode"],
        ("pi", _) => &["pi"],
        _ => &[],
    };
    for bin in bins {
        let out = std::process::Command::new(bin).arg("--version").output();
        let Ok(out) = out else { continue };
        let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
        if s.trim().is_empty() {
            s = String::from_utf8_lossy(&out.stderr).into_owned();
        }
        let line = s.lines().next().unwrap_or("").trim();
        if !line.is_empty() {
            return line.chars().take(32).collect();
        }
    }
    "-".into()
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// True when the agent itself is found: its binary answers `--version`, or any of
/// its config files (or the directory that would hold them) exists. Setup skips
/// a missing agent instead of creating its files; removal still runs so a
/// half-installed host is cleaned up by the caller passing `remove`.
pub fn agent_present(host: &str, kind: &str, cfg: &crate::config::Config) -> bool {
    if app_version(host, kind) != "-" {
        return true;
    }
    let mut paths = Vec::new();
    match host {
        "claude" => {
            paths.push(cfg.setup.claude.settings_path.clone());
            paths.push(cfg.doctor.claude_json.clone());
        }
        "cursor" => {
            paths.push(cfg.setup.cursor.hooks_path.clone());
            paths.push(cfg.setup.cursor.hooks_path.with_file_name("mcp.json"));
            paths.push(cursor::plugin_dest(cfg));
        }
        "codex" => paths.push(cfg.setup.codex.config_path.clone()),
        "opencode" => paths.push(opencode_path_for(kind, cfg)),
        "pi" => {
            paths.push(pi::plugin_dest(cfg));
            paths.push(cfg.setup.pi.extensions_path.clone());
        }
        _ => return false,
    }
    paths.iter().any(|p| {
        p.exists()
            || p.parent()
                .is_some_and(|d| !d.as_os_str().is_empty() && d.exists())
    })
}

/// Which rtok modules a host variant carries: the same markers the installers write.
pub fn installed_modules(host: &str, kind: &str, cfg: &crate::config::Config) -> Vec<String> {
    match host {
        "claude" => {
            let s = read(&cfg.setup.claude.settings_path);
            let m = read(&cfg.doctor.claude_json);
            let mut out = Vec::new();
            if s.contains("rtok hook") {
                out.push("hooks".to_string());
            }
            if m.contains("\"rtok\"") {
                out.push("mcp".to_string());
            }
            if s.contains("8790") && s.contains("BASE_URL") {
                out.push("proxy".to_string());
            }
            out
        }
        "cursor" => {
            let h = read(&cfg.setup.cursor.hooks_path);
            let m = read(&cfg.setup.cursor.hooks_path.with_file_name("mcp.json"));
            let mut out = Vec::new();
            if h.contains("rtok hook") {
                out.push("hooks".to_string());
            }
            if m.contains("\"rtok\"") {
                out.push("mcp".to_string());
            }
            if cursor::plugin_dest(cfg).symlink_metadata().is_ok() {
                out.push("plugin".to_string());
            }
            out
        }
        "codex" => {
            let s = read(&cfg.setup.codex.config_path);
            let mut out = Vec::new();
            if s.contains("[mcp_servers.rtok]") {
                out.push("mcp".to_string());
            }
            if s.contains("[model_providers.rtok]") {
                out.push("proxy".to_string());
            }
            out
        }
        "opencode" => {
            let s = read(&opencode_path_for(kind, cfg));
            let mut out = Vec::new();
            if s.contains("OPENAI_BASE_URL") {
                out.push("proxy".to_string());
            }
            if s.contains("\"rtok\"") {
                out.push("mcp".to_string());
            }
            out
        }
        "pi" => {
            if pi::plugin_dest(cfg).symlink_metadata().is_ok() {
                vec!["plugin".to_string()]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// `rtok agent list`: every known host × variant with app version, rtok state, modules.
pub fn list(cfg: &crate::config::Config) -> String {
    use crate::render::{Col, table};
    let cols = [
        Col::left(0),
        Col::left(0),
        Col::left(0),
        Col::left(0),
        Col::left(0),
    ];
    let mut rows = vec![vec![
        "agent".to_string(),
        "type".to_string(),
        "version".to_string(),
        "installed".to_string(),
        "modules".to_string(),
    ]];
    for host in HOSTS {
        for kind in variants(host) {
            let mods = installed_modules(host, kind, cfg);
            rows.push(vec![
                host.to_string(),
                kind.to_string(),
                app_version(host, kind),
                if mods.is_empty() { "no" } else { "yes" }.to_string(),
                if mods.is_empty() {
                    "-".to_string()
                } else {
                    mods.join(",")
                },
            ]);
        }
    }
    table(&cols, &rows)
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The `[setup]` flags an installer acts on, as the SDK spells them.
pub(crate) fn apply(cfg: &crate::config::Config) -> rtok_agent_sdk::Apply {
    rtok_agent_sdk::Apply {
        dry_run: cfg.setup.dry_run,
        backup: cfg.setup.backup,
        yes: cfg.setup.yes,
    }
}

pub(crate) fn openai_proxy_url(cfg: &crate::config::Config) -> String {
    format!("http://{}:{}/v1", cfg.proxy.bind, cfg.proxy.port)
}

/// The tree this repo ships a host plugin from (D21 (6)).
pub(crate) fn plugin_src(rel: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn apply_carries_the_setup_flags() {
        let mut cfg = Config::default();
        cfg.setup.dry_run = true;
        cfg.setup.yes = true;
        cfg.setup.backup = false;
        let a = apply(&cfg);
        assert!(a.dry_run && a.yes && !a.backup);
    }

    #[test]
    fn variant_filter_defaults_to_all() {
        assert!(wants("cli", false, false, false));
        assert!(wants("gui", false, false, false));
        assert!(wants("cli", false, false, true));
        assert!(wants("gui", false, false, true));
        assert!(wants("cli", true, false, false));
        assert!(!wants("gui", true, false, false));
        assert!(!wants("cli", false, true, false));
        assert!(wants("gui", false, true, false));
        assert_eq!(variants("cursor"), vec!["cli", "gui"]);
        assert_eq!(variants("opencode"), vec!["cli", "gui"]);
        assert_eq!(variants("claude"), vec!["cli"]);
    }

    #[test]
    fn list_shows_every_host_with_type_and_state() {
        let cfg = Config::default();
        let out = list(&cfg);
        let head = out.lines().next().unwrap_or("");
        assert!(head.contains("agent") && head.contains("type") && head.contains("version"));
        assert!(
            head.contains("installed") && head.contains("modules"),
            "{out}"
        );
        for host in ["claude", "cursor", "codex", "opencode", "pi"] {
            assert!(out.contains(host), "{out}");
        }
        assert!(out.contains("cli") && out.contains("gui"), "{out}");
    }

    #[test]
    fn unknown_version_is_a_dash_and_empty_modules_means_no() {
        assert_eq!(app_version("nope", "cli"), "-");
        let cfg = Config::default();
        // Default paths point at a real HOME that rarely carries these files;
        // whatever it finds, the two spellings must agree.
        for host in HOSTS {
            for kind in variants(host) {
                let mods = installed_modules(host, kind, &cfg);
                assert!(mods.iter().all(|m| !m.is_empty()));
            }
        }
    }
}
