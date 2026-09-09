//! Host installers (`rtok agent setup <host>`).

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

pub(crate) fn openai_proxy_url(cfg: &crate::config::Config) -> String {
    format!("http://{}:{}/v1", cfg.proxy.bind, cfg.proxy.port)
}
