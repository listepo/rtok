//! VS Code Copilot Chat installer (`rtok agents install vscode`, plan T48.8).
//!
//! Copilot Chat reads MCP from the user `mcp.json` (`servers.<name>`, `type: "stdio"`) in
//! the VS Code profile dir. User-dir resolution matches Claude Desktop: Application Support
//! on macOS, `%APPDATA%` on Windows, `~/.config` elsewhere, for both `Code` and
//! `Code - Insiders`. Hooks are not written: VS Code documents Claude-format agent hooks
//! (`hook_event_name` / `tool_name` in `.github/hooks` and `~/.copilot/hooks`), which the
//! T46.3 Copilot mapping does not serve; user `~/.copilot/hooks` is the `copilot` host.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::json;

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";
const CODE: &str = "Code";
const INSIDERS: &str = "Code - Insiders";

/// VS Code Copilot Chat: MCP in each edition's user `mcp.json`.
pub struct Vscode;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Desktop,
    name: "VS Code",
    bins: &["code", "code-insiders"],
    apps: &[
        "/Applications/Visual Studio Code.app",
        "/Applications/Visual Studio Code - Insiders.app",
        "$LOCALAPPDATA/Programs/Microsoft VS Code/Code.exe",
        "$LOCALAPPDATA/Programs/Microsoft VS Code Insiders/Code - Insiders.exe",
    ],
}];

/// `<product>/User` under the OS user-data root (`Code` or `Code - Insiders`).
pub fn user_dir(product: &str) -> PathBuf {
    let home = super::home_dir();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
            .join(product)
            .join("User")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join(product)
            .join("User")
    } else {
        home.join(".config").join(product).join("User")
    }
}

/// `<user-dir>/mcp.json`.
pub fn mcp_path(product: &str) -> PathBuf {
    user_dir(product).join("mcp.json")
}

fn paths() -> [PathBuf; 2] {
    [mcp_path(CODE), mcp_path(INSIDERS)]
}

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

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "mcp" => Support::Yes,
            "hooks" => Support::No(
                "VS Code agent hooks use hook_event_name; T46.3 Copilot mapping does not serve them",
            ),
            "proxy" => Support::No(
                "VS Code Copilot Chat has no documented base-URL setting to point at the proxy",
            ),
            _ => Support::No(
                "Copilot Chat has no local plugin directory to link; Agent Plugins are marketplace packages",
            ),
        }
    }

    fn files(&self, _cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        paths().into()
    }

    fn installed(&self, _cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        if paths().iter().any(|p| super::read(p).contains("\"rtok\"")) {
            vec!["mcp"]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        if mode == Mode::Remove {
            paths().iter().map(|p| unregister_mcp(cfg, p)).collect()
        } else if cfg.setup.mcp {
            paths().iter().map(|p| register_mcp(cfg, p)).collect()
        } else {
            Ok(vec![rtok_agent_sdk::NO_CHANGES.into()])
        }
    }
}

/// `servers.rtok = {type: "stdio", command, args}` — the stdio shape the VS Code MCP docs show.
pub fn register_mcp(cfg: &Config, path: &Path) -> Result<String> {
    let cmd = super::rtok_command();
    let entry = json!({"type": "stdio", "command": cmd, "args": ["mcp"]});
    rtok_agent_sdk::register_server(
        &apply(cfg),
        path,
        "servers",
        NAME,
        entry,
        &format!("{cmd} mcp"),
    )
}

/// Drop `servers.rtok` from `mcp.json`. Foreign servers are left alone.
pub fn unregister_mcp(cfg: &Config, path: &Path) -> Result<String> {
    rtok_agent_sdk::unregister_server(&apply(cfg), path, "servers", NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_dir_resolves_code_and_insiders_per_os() {
        for product in [CODE, INSIDERS] {
            let dir = user_dir(product);
            let mcp = mcp_path(product);
            assert_eq!(mcp, dir.join("mcp.json"));
            assert!(mcp.ends_with("mcp.json"), "{}", mcp.display());
            let s = dir.to_string_lossy();
            assert!(s.contains(product), "{s}");
            if cfg!(target_os = "macos") {
                assert!(s.contains("Library/Application Support"), "{s}");
            } else if cfg!(target_os = "windows") {
                let appdata = std::env::var_os("APPDATA").expect("APPDATA");
                assert!(dir.starts_with(&appdata), "{}", dir.display());
            } else {
                assert!(s.contains(".config"), "{s}");
            }
        }
    }
}
