//! Host installers (`rtok agent setup <host>`).
//!
//! Everything the five hosts share — backup, the dry-run/idempotence write gate, `mcpServers`
//! registration, the plugin-link offer — lives in `rtok-agent-sdk` (D28). What stays here is
//! per-host: which file, which shape, which keys.

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod migrate;
pub mod opencode;
pub mod pi;

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
        "opencode" => vec![cfg.setup.opencode.config_path.clone()],
        _ => Vec::new(),
    }
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
}
