//! `~/.rtok/config.toml` — every setting rtok has (plan T0.2, T12.1, decision D12).
//!
//! `config/default.toml` is the reference file: it is embedded with `include_str!`, written
//! verbatim on a fresh install (so its comments survive), and it must parse to exactly
//! [`Config::default()`] — a test asserts it.
//!
//! Every section is `#[serde(default, deny_unknown_fields)]`: a partial file keeps the
//! defaults, and a typo is an error rather than a silently ignored key.
//! Layering (default < user file < project file < env < flags) is [`layers`] (T12.2).

pub mod layers;
pub mod validate;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// The annotated reference file, written verbatim by `rtok config init`.
pub const DEFAULT_TOML: &str = include_str!("../../config/default.toml");

/// Plugin catalogue: `(id, default_on)`. The registry's manifests must match this list
/// (asserted by a test in `plugins`), and [`Plugins`] has one field per id.
pub const CATALOGUE: [(&str, bool); 10] = [
    ("measure", true),
    ("cmd", true),
    ("read", true),
    ("archive", true),
    ("proxy", true),
    ("inject", true),
    ("guard", true),
    ("memory", true),
    ("graph", true),
    ("toon", false),
];

/// Shorthand for the section attributes every table repeats.
macro_rules! section {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $field:ident : $ty:ty = $default:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct $name {
            $($(#[$fm])* pub $field: $ty,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: $default,)* }
            }
        }
    };
}

fn s(v: &str) -> String {
    v.to_string()
}

fn p(v: &str) -> PathBuf {
    PathBuf::from(v)
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

// ── core ────────────────────────────────────────────────────────────────────

section! {
    /// `[core]` — paths and logging shared by every surface.
    Core {
        db_path: PathBuf = p("~/.rtok/rtok.db"),
        archive_dir: PathBuf = p("~/.rtok/archive"),
        log_level: String = s("warn"),
        log_file: PathBuf = p("~/.rtok/rtok.log"),
        session_env: String = s("CLAUDE_SESSION_ID"),
        call_io_inline_bytes: u32 = 65536,
        retain_calls_days: u32 = 30,
        log_to_db: bool = true,
        /// Removed in T12.1: it is now `plugins.inject.budget_tokens`. Accepted from an old
        /// file with a warning, then dropped.
        #[serde(skip_serializing_if = "Option::is_none")]
        inject_budget_tokens: Option<u32> = None,
    }
}

section! {
    /// `[estimator]` — chars per token per class (plan T0.5), rewritten by `stats --calibrate`.
    Estimator {
        code: f32 = 3.5,
        prose: f32 = 4.2,
        json: f32 = 3.0,
        cjk: f32 = 1.0,
    }
}

// ── surfaces ────────────────────────────────────────────────────────────────

section! {
    /// `[hook]` — `rtok hook <event>`.
    Hook {
        host: String = s("claude"),
        max_ms: u64 = 10,
        fail_open: bool = true,
    }
}

section! {
    /// `[mcp]` — `rtok mcp`.
    Mcp {
        tools: Vec<String> = Vec::new(),
        max_description_tokens: u32 = 60,
        max_result_chars: u32 = 20000,
    }
}

section! {
    /// `[proxy]` — the proxy server itself; the usage-capture plugin is `[plugins.proxy]`.
    Proxy {
        bind: String = s("127.0.0.1"),
        port: u16 = 8790,
        mode: String = s("passthrough"),
        upstream: String = s("https://api.anthropic.com"),
        openai_upstream: String = s("https://api.openai.com"),
        timeout_s: u64 = 600,
        include_usage: bool = true,
        dry_run: bool = false,
    }
}

section! {
    /// `[stats]` — `rtok stats`.
    Stats {
        since: String = s("30d"),
        format: String = s("table"),
        plugin: String = String::new(),
        transcripts_dir: PathBuf = p("~/.claude/projects"),
        calibrate_samples: u32 = 30,
        baseline: String = String::new(),
    }
}

section! {
    /// `[bench]` — `rtok bench`.
    Bench {
        tasks: PathBuf = p("bench/tasks.toml"),
        runs: u32 = 3,
        dry_run: bool = false,
        timeout_s: u64 = 900,
        /// `[bench.configs]` — free-form `name = settings file`, so not a fixed struct.
        configs: BTreeMap<String, PathBuf> = [
            (s("a"), p("bench/configs/legacy.json")),
            (s("b"), p("bench/configs/rtok.json")),
        ].into_iter().collect(),
    }
}

section! {
    /// `[doctor]` — `rtok doctor`.
    Doctor {
        settings_path: PathBuf = p("~/.claude/settings.json"),
        claude_json: PathBuf = p("~/.claude.json"),
        mcp_json: PathBuf = p(".mcp.json"),
        probe_timeout_ms: u64 = 500,
        mcp_timeout_ms: u64 = 15000,
        instruction_warn_tokens: u32 = 1000,
        instructions: bool = false,
    }
}

section! {
    /// `[setup]` — `rtok setup <host>`.
    Setup {
        dry_run: bool = false,
        yes: bool = false,
        backup: bool = true,
        hook_timeout_s: u64 = 5,
        modes: Vec<String> = Vec::new(),
        mcp: bool = true,
        proxy: bool = false,
        claude: SetupClaude = SetupClaude::default(),
        cursor: SetupCursor = SetupCursor::default(),
        codex: SetupCodex = SetupCodex::default(),
        opencode: SetupOpenCode = SetupOpenCode::default(),
    }
}

section! {
    /// `[setup.claude]`
    SetupClaude { settings_path: PathBuf = p("~/.claude/settings.json") }
}

section! {
    /// `[setup.cursor]`
    SetupCursor { hooks_path: PathBuf = p("~/.cursor/hooks.json") }
}

section! {
    /// `[setup.codex]`
    SetupCodex { config_path: PathBuf = p("~/.codex/config.toml") }
}

section! {
    /// `[setup.opencode]`
    SetupOpenCode { config_path: PathBuf = p("~/.config/opencode/opencode.json") }
}

section! {
    /// `[expand]` — `rtok expand <id>`.
    Expand { max_lines: u32 = 0 }
}

section! {
    /// `[filter]` — `rtok filter --stdin` (T10.2).
    Filter { cmd: String = String::new() }
}

// ── plugins ─────────────────────────────────────────────────────────────────

section! {
    /// `[plugins.*]` — one field per catalogue id, in `CATALOGUE` order.
    Plugins {
        measure: Measure = Measure::default(),
        cmd: Cmd = Cmd::default(),
        read: Read = Read::default(),
        archive: Archive = Archive::default(),
        proxy: ProxyPlugin = ProxyPlugin::default(),
        inject: Inject = Inject::default(),
        guard: Guard = Guard::default(),
        memory: Memory = Memory::default(),
        graph: Graph = Graph::default(),
        toon: Toon = Toon::default(),
    }
}

section! {
    /// `[plugins.measure]`
    Measure { enabled: bool = true }
}

section! {
    /// `[plugins.cmd]`
    Cmd {
        enabled: bool = true,
        rewrite: bool = true,
        shell: String = String::new(),
        rules: PathBuf = p("~/.rtok/rules.toml"),
        trailer_min_lines: u32 = 40,
        fail_tail_lines: u32 = 80,
        never_wrap: Vec<String> = strs(&["rtok", "sudo"]),
    }
}

section! {
    /// `[plugins.read]`
    Read {
        enabled: bool = true,
        default_mode: String = s("full"),
        max_chars: u32 = 20000,
        native_max_bytes: u64 = 32768,
        advice: bool = true,
        allow_paths: Vec<PathBuf> = Vec::new(),
        search_max: u32 = 50,
        tree_depth: u32 = 2,
        languages: Vec<String> = strs(&["rust", "ts", "js", "python", "dart", "c", "go"]),
    }
}

section! {
    /// `[plugins.archive]`
    Archive {
        enabled: bool = true,
        keep_turns: u32 = 4,
        min_tokens: u32 = 1500,
        head_lines: u32 = 8,
        tail_lines: u32 = 4,
    }
}

section! {
    /// `[plugins.proxy]` — the usage-capture plugin, not the `[proxy]` server.
    ProxyPlugin { enabled: bool = true }
}

section! {
    /// `[plugins.inject]`
    Inject {
        enabled: bool = true,
        budget_tokens: u32 = 800,
        modes_dir: PathBuf = p("~/.rtok/modes"),
        modes: Vec<String> = Vec::new(),
    }
}

section! {
    /// `[plugins.guard]`
    Guard {
        enabled: bool = true,
        window_turns: u32 = 8,
    }
}

section! {
    /// `[plugins.memory]`
    Memory {
        enabled: bool = true,
        recall_titles: u32 = 5,
        recall_tokens: u32 = 200,
        checkpoint_tokens: u32 = 400,
        search_limit: u32 = 5,
    }
}

section! {
    /// `[plugins.graph]`
    Graph {
        enabled: bool = true,
        max_tokens: u32 = 2000,
    }
}

section! {
    /// `[plugins.toon]`
    Toon {
        enabled: bool = false,
        min_rows: u32 = 5,
    }
}

// ── the whole file ──────────────────────────────────────────────────────────

/// Every setting rtok has. Sections mirror the tables in `config/default.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub core: Core,
    pub estimator: Estimator,
    pub hook: Hook,
    pub mcp: Mcp,
    pub proxy: Proxy,
    pub stats: Stats,
    pub bench: Bench,
    pub doctor: Doctor,
    pub setup: Setup,
    pub expand: Expand,
    pub filter: Filter,
    pub plugins: Plugins,
    /// Directory the config was loaded from; not part of the file.
    #[serde(skip)]
    pub home: PathBuf,
}

impl Config {
    /// `$RTOK_HOME` or `$HOME/.rtok`.
    pub fn home_dir() -> PathBuf {
        if let Some(h) = std::env::var_os("RTOK_HOME") {
            return PathBuf::from(h);
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".rtok")
    }

    /// `<home>/config.toml`.
    pub fn path_for(home: &Path) -> PathBuf {
        home.join("config.toml")
    }

    pub fn load() -> Result<Self> {
        Self::load_with(None, None)
    }

    /// Read `<home>/config.toml`, creating it from the reference file when absent, then apply
    /// the full layering (user < project < env). Pins the user file to `home` so `RTOK_CONFIG`
    /// cannot leak into tests. No flags — see [`layers::load`] for that.
    pub fn load_from(home: &Path) -> Result<Self> {
        if !Self::path_for(home).exists() {
            Self::init(home, false)?;
        }
        layers::load(home, Some(&Self::path_for(home)), None)
    }

    /// CLI entry: optional `--config` / `RTOK_CONFIG`, plus the `flag` Dict from clap `Some`s.
    pub fn load_with(
        config_file: Option<&Path>,
        flags: Option<figment::value::Dict>,
    ) -> Result<Self> {
        let home = Self::home_dir();
        Self::ensure_user_file(&home, config_file)?;
        layers::load(&home, config_file, flags)
    }

    /// Warn and use defaults. Hooks (T12.3) never fail on a bad file.
    pub fn load_lenient(config_file: Option<&Path>, flags: Option<figment::value::Dict>) -> Self {
        match Self::load_with(config_file, flags) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("rtok: config ignored ({e}); using defaults");
                let home = Self::home_dir();
                let mut c = Self::default();
                c.finish(&home);
                c
            }
        }
    }

    /// Create `<home>/config.toml` from the reference when neither `--config` nor `RTOK_CONFIG`
    /// names a file and the user file is missing.
    pub fn ensure_user_file(home: &Path, config_file: Option<&Path>) -> Result<()> {
        if config_file.is_some() || std::env::var_os("RTOK_CONFIG").is_some() {
            return Ok(());
        }
        if !Self::path_for(home).exists() {
            Self::init(home, false)?;
        }
        Ok(())
    }

    /// Write the reference file verbatim, so its comments survive. Refuses to clobber
    /// an existing file unless `force`.
    pub fn init(home: &Path, force: bool) -> Result<PathBuf> {
        let path = Self::path_for(home);
        if path.exists() && !force {
            bail!("{} exists; pass --force to overwrite", path.display());
        }
        std::fs::create_dir_all(home)?;
        std::fs::write(&path, DEFAULT_TOML).with_context(|| path.display().to_string())?;
        Ok(path)
    }

    /// Migrate legacy keys and expand `~` in paths. Called after every parse.
    fn finish(&mut self, home: &Path) {
        if let Some(budget) = self.core.inject_budget_tokens.take() {
            eprintln!(
                "rtok: core.inject_budget_tokens is now plugins.inject.budget_tokens (using {budget})"
            );
            self.plugins.inject.budget_tokens = budget;
        }
        self.home = home.to_path_buf();
        for path in [
            &mut self.core.db_path,
            &mut self.core.archive_dir,
            &mut self.core.log_file,
            &mut self.stats.transcripts_dir,
            &mut self.doctor.settings_path,
            &mut self.doctor.claude_json,
            &mut self.setup.claude.settings_path,
            &mut self.setup.cursor.hooks_path,
            &mut self.setup.codex.config_path,
            &mut self.setup.opencode.config_path,
            &mut self.plugins.cmd.rules,
            &mut self.plugins.inject.modes_dir,
        ] {
            *path = expand(path, home);
        }
    }

    /// `[plugins.<id>] enabled`. `default_on` is the answer for an id that is not in the
    /// catalogue — an external plugin registered through `Registry::from_plugins`.
    pub fn plugin_enabled(&self, id: &str, default_on: bool) -> bool {
        let p = &self.plugins;
        match id {
            "measure" => p.measure.enabled,
            "cmd" => p.cmd.enabled,
            "read" => p.read.enabled,
            "archive" => p.archive.enabled,
            "proxy" => p.proxy.enabled,
            "inject" => p.inject.enabled,
            "guard" => p.guard.enabled,
            "memory" => p.memory.enabled,
            "graph" => p.graph.enabled,
            "toon" => p.toon.enabled,
            _ => default_on,
        }
    }
}

/// `~/.rtok/x` → `<home>/x` (so `RTOK_HOME` moves the whole tree), other `~/x` → `$HOME/x`.
fn expand(path: &Path, home: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix("~/.rtok/") {
        return home.join(rest);
    }
    match (raw.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(h)) => PathBuf::from(h).join(rest),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::Figment;
    use figment::providers::{Format, Toml};

    /// Parse a TOML string into a `Config` the same way the layered loader does (T12.2: figment's
    /// Toml provider, not the toml crate).
    fn parse(s: &str) -> Result<Config> {
        Figment::from(Toml::string(s)).extract().map_err(Into::into)
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-cfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The Check for T12.1: the reference file is the defaults, exactly.
    #[test]
    fn default_toml_is_the_defaults() {
        let parsed: Config = parse(DEFAULT_TOML).expect("default.toml parses");
        assert_eq!(
            parsed,
            Config::default(),
            "config/default.toml drifted from Config::default()"
        );
    }

    #[test]
    fn every_catalogue_id_is_answered() {
        let cfg = Config::default();
        for (id, on) in CATALOGUE {
            assert_eq!(cfg.plugin_enabled(id, !on), on, "{id}");
        }
        // An id outside the catalogue falls back to the manifest's default_on.
        assert!(cfg.plugin_enabled("external", true));
        assert!(!cfg.plugin_enabled("external", false));
    }

    #[test]
    fn creates_reference_file_and_budget_is_800() {
        let home = tmp("create");
        let cfg = Config::load_from(&home).unwrap();
        let written = std::fs::read_to_string(Config::path_for(&home)).unwrap();
        assert_eq!(
            written, DEFAULT_TOML,
            "init must write the reference verbatim"
        );
        assert_eq!(cfg.plugins.inject.budget_tokens, 800);
        assert_eq!(cfg.core.db_path, home.join("rtok.db"));
        assert_eq!(cfg.core.archive_dir, home.join("archive"));
        assert!(!cfg.plugin_enabled("toon", true));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn init_refuses_to_clobber_without_force() {
        let home = tmp("force");
        Config::init(&home, false).unwrap();
        std::fs::write(Config::path_for(&home), "[core]\n").unwrap();
        assert!(Config::init(&home, false).is_err());
        Config::init(&home, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(Config::path_for(&home)).unwrap(),
            DEFAULT_TOML
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn partial_file_keeps_defaults() {
        let cfg: Config =
            parse("[plugins.cmd]\nrewrite = false\n[estimator]\ncode = 4.0\n").unwrap();
        assert_eq!(cfg.plugins.inject.budget_tokens, 800);
        assert_eq!(cfg.estimator.code, 4.0);
        assert_eq!(cfg.estimator.prose, 4.2);
        assert!(!cfg.plugins.cmd.rewrite);
        assert!(cfg.plugins.cmd.enabled);
    }

    #[test]
    fn unknown_key_is_an_error() {
        let err = parse("[proxy]\nprot = 1\n").unwrap_err();
        assert!(err.to_string().contains("prot"), "{err}");
        assert!(parse("[nope]\nx = 1\n").is_err());
    }

    #[test]
    fn legacy_budget_key_migrates() {
        let mut cfg: Config = parse("[core]\ninject_budget_tokens = 250\n").unwrap();
        cfg.finish(Path::new("/tmp/rtok-legacy"));
        assert_eq!(cfg.plugins.inject.budget_tokens, 250);
        assert_eq!(cfg.core.inject_budget_tokens, None);
    }
}
