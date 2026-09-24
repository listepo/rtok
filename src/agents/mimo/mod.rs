//! MiMo Code installer (`rtok agents install mimo`, plan T186).
//!
//! MiMo Code is Xiaomi's terminal coding agent, an OpenCode fork (`mimo`, install via
//! `curl -fsSL https://mimo.xiaomi.com/install | bash` or `npm i -g @mimo-ai/cli`). Its global
//! config, `[setup.mimo] config_path` (default `~/.config/mimocode/mimocode.json`,
//! `MIMOCODE_HOME`/`MIMOCODE_CONFIG` move it), carries MCP servers under `mcp.<name>` in the
//! exact shape OpenCode kept from upstream — `{type: "local", command: [..], enabled: true}`
//! (https://mimo.xiaomi.com/mimocode/mcp-servers, fetched 2026-09-24) — so install reuses
//! [`super::register_local_mcp`], the helper `opencode` was refactored onto rather than
//! respelling the same JSON here (T186, keeps `just dup` under its 2 % budget).
//!
//! v1 is CLI + MCP only. MiMo Desktop exists (early access) but no config path for it is
//! documented, so no Desktop variant ships. Hooks and a linked plugin are both `Support::No`:
//! MiMo has no shell hook events of its own — like OpenCode, `tool.execute.before`/`after`
//! only run in-process through a linked plugin file, and MiMo's own package for that
//! (`@mimo-ai/plugin`, mirroring `@opencode-ai/plugin`) is undocumented, so shipping one here
//! would be a guess, not a verified path; a plugin is left for a follow-up task once that
//! package is confirmed. Proxy is `Support::No`: no base-URL override is documented anywhere
//! in the config or env-var reference.

use std::path::PathBuf;

use anyhow::Result;

use super::{
    Agent, Kind, Mode, Support, Variant, apply_mcp_only, installed_mcp_only, register_local_mcp,
    support_mcp_only, unregister_local_mcp,
};
use crate::config::Config;

/// MiMo Code: one CLI binary, no documented Desktop config path yet.
pub struct Mimo;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "MiMo Code",
    bins: &["mimo"],
    apps: &[],
}];

fn config_path(cfg: &Config) -> PathBuf {
    cfg.setup.mimo.config_path.clone()
}

/// `mcp.rtok` — the local-argv shape confirmed against MiMo's own MCP docs (module doc).
pub fn register_mcp(cfg: &Config) -> Result<String> {
    register_local_mcp(cfg, &config_path(cfg), "mcp")
}

/// Drop `mcp.rtok` (`rtok agents remove mimo`).
pub fn unregister_mcp(cfg: &Config) -> Result<String> {
    unregister_local_mcp(cfg, &config_path(cfg), "mcp")
}

impl Agent for Mimo {
    fn id(&self) -> &'static str {
        "mimo"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        support_mcp_only(
            module,
            "MiMo Code has no shell hook events; like OpenCode, tool.execute.before/after run in-process through a linked plugin, which this task does not ship",
            "MiMo Code has no documented base-URL override; MIMOCODE_HOME/MIMOCODE_CONFIG relocate config, not the model endpoint",
            "MiMo's in-process plugin package (the @mimo-ai/plugin analogue of @opencode-ai/plugin) is undocumented; linking one here would be a guess, not a verified path",
        )
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<PathBuf> {
        vec![config_path(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        installed_mcp_only(&config_path(cfg))
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        apply_mcp_only(cfg, mode, register_mcp, unregister_mcp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::assert_local_mcp_roundtrip;
    use rtok_agent_sdk::NO_CHANGES;
    use std::fs;

    fn cfg(name: &str, dry: bool) -> (Config, PathBuf) {
        super::super::test_scratch_cfg("mimo", name, "mimocode.json", dry, |c, path| {
            c.setup.mimo.config_path = path;
        })
    }

    #[test]
    fn dry_run_names_the_change_and_creates_nothing() {
        let (c, path) = cfg("dry", true);
        let out = register_mcp(&c).unwrap();
        assert!(out.starts_with("mcp.rtok: "), "{out}");
        assert!(!path.exists());
        assert!(Mimo.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn mcp_entry_is_local_argv_idempotent_and_remove_keeps_foreign() {
        let (c, path) = cfg("mcp", false);
        assert_local_mcp_roundtrip(
            &path,
            || register_mcp(&c),
            || unregister_mcp(&c),
            || Mimo.installed(&c, Kind::Cli),
        );
        assert!(Mimo.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_is_idempotent_via_the_agent_trait() {
        let (c, path) = cfg("apply", false);
        let lines = Mimo.apply(&c, Kind::Cli, Mode::Install).unwrap();
        assert!(lines[0].starts_with("mcp.rtok: "), "{lines:?}");
        assert_eq!(
            Mimo.apply(&c, Kind::Cli, Mode::Install).unwrap(),
            [NO_CHANGES]
        );
        assert_eq!(
            Mimo.apply(&c, Kind::Cli, Mode::Remove).unwrap(),
            ["- mcp.rtok"]
        );
        assert_eq!(
            Mimo.apply(&c, Kind::Cli, Mode::Remove).unwrap(),
            [NO_CHANGES]
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn support_matches_mcp_yes_hooks_proxy_plugin_no() {
        assert!(matches!(Mimo.support(Kind::Cli, "mcp"), Support::Yes));
        assert!(matches!(Mimo.support(Kind::Cli, "hooks"), Support::No(_)));
        assert!(matches!(Mimo.support(Kind::Cli, "proxy"), Support::No(_)));
        assert!(matches!(Mimo.support(Kind::Cli, "plugin"), Support::No(_)));
    }
}
