//! VS Code Copilot Chat installer (`rtok agents install vscode`, plan T48.8).
//!
//! GitHub Copilot in VS Code reads MCP servers from the user profile `mcp.json`
//! (`servers.<name>`, `type: "stdio"`). Agent hooks use Claude-format I/O
//! (PascalCase events in `.github/hooks/*.json` or `~/.copilot/hooks/`); the
//! Copilot CLI camelCase mapping (T46.3) does not apply, so setup writes MCP only.

use std::path::PathBuf;

use anyhow::Result;
use rtok_agent_sdk::NO_CHANGES;
use serde_json::json;

use super::{Agent, Kind, Mode, Support, Variant, apply, home_dir};
use crate::config::Config;

const NAME: &str = "rtok";

/// VS Code stable and Insiders: one apply writes both profile `mcp.json` files.
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
            "hooks" => Support::No(
                "VS Code agent hooks use Claude-format I/O; T46.3 copilot mapping serves only the Copilot CLI camelCase hooks in ~/.copilot/hooks/rtok.json",
            ),
            "proxy" => Support::No(
                "Copilot in VS Code has no documented base-URL setting to point at the proxy",
            ),
            _ => Support::No(
                "VS Code loads MCP from the user mcp.json; there is no local plugin directory to link",
            ),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![mcp_path(cfg, false), mcp_path(cfg, true)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let stable = super::read(&mcp_path(cfg, false)).contains("\"rtok\"");
        let insiders = super::read(&mcp_path(cfg, true)).contains("\"rtok\"");
        if stable || insiders {
            vec!["mcp"]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        let mut lines = Vec::new();
        for insiders in [false, true] {
            let line = if remove {
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
    let base = if insiders {
        &cfg.setup.vscode.insiders_user_dir
    } else {
        &cfg.setup.vscode.code_user_dir
    };
    if base.as_os_str().is_empty() {
        default_user_dir(insiders).join("mcp.json")
    } else {
        base.join("mcp.json")
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

    #[test]
    fn mcp_entry_is_stdio_and_idempotent() {
        let dir = std::env::temp_dir().join(format!("rtok-vscode-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(dir.join("insiders")).unwrap();
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
}
