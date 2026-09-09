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

/// D21 (6): the host plugin is offered, never forced. `--yes` accepts without asking; on a
/// terminal we ask, and dialoguer owns that prompt — it restores the terminal afterwards and
/// reads Ctrl-C and EOF as a no. Anywhere without a terminal — CI, a pipe, a host running setup
/// for the user — an unanswered offer is a no, so `agent setup` stays non-interactive by default.
/// `--dry-run` never reaches here: it describes the offer and returns before anything is asked.
pub(crate) fn accepted(cfg: &crate::config::Config, question: &str) -> bool {
    use std::io::IsTerminal;
    if cfg.setup.yes {
        return true;
    }
    if !std::io::stdin().is_terminal() {
        return false;
    }
    dialoguer::Confirm::new()
        .with_prompt(question)
        .default(true)
        .interact()
        .unwrap_or(false)
}

pub(crate) fn openai_proxy_url(cfg: &crate::config::Config) -> String {
    format!("http://{}:{}/v1", cfg.proxy.bind, cfg.proxy.port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// The test harness has no terminal, which is exactly the CI / piped / host-driven case:
    /// the offer must decline itself rather than block waiting for an answer nobody can give.
    #[test]
    fn without_a_terminal_only_yes_accepts() {
        let mut cfg = Config::default();
        assert!(
            !accepted(&cfg, "install?"),
            "a headless run must not accept"
        );
        cfg.setup.yes = true;
        assert!(
            accepted(&cfg, "install?"),
            "--yes must accept without asking"
        );
    }
}
