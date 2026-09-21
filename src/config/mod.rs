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

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// The annotated reference file, written verbatim by `rtok config init`.
pub const DEFAULT_TOML: &str = include_str!("../../config/default.toml");

/// Write a config file with the installers' atomic swap: temp file, then rename. With a plain
/// `fs::write`, a kill or a full disk halfway through left `config.toml` truncated, and every
/// later rtok command, hooks included, failed to parse it.
pub(crate) fn write_file(path: &Path, body: &str) -> Result<()> {
    rtok_agent_sdk::write(&rtok_agent_sdk::Apply::default(), path, body, "config")
}

/// Plugin catalogue: `(id, default_on)`. The registry's manifests must match this list
/// (asserted by a test in `plugins`), and [`Plugins`] has one field per id.
pub const CATALOGUE: [(&str, bool); 11] = [
    ("measure", true),
    ("cmd", true),
    ("read", true),
    ("archive", true),
    ("proxy", true),
    ("inject", true),
    ("guard", true),
    ("memory", true),
    ("graph", true),
    ("toon", true),
    ("compress", true),
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
    /// `[core]` — paths shared by every surface. Logging lives in `[log]` (D26).
    Core {
        /// Kill-switch for business logic. When false the process stays up; the HTTP
        /// proxy (and any other long-running surface that would apply plugins/bookkeeping)
        /// runs as a plain forwarder until the process exits. Disabling never stops the listener.
        enabled: bool = true,
        db_path: PathBuf = p("~/.rtok/rtok.db"),
        archive_dir: PathBuf = p("~/.rtok/archive"),
        /// Removed in T24.5: it is now `log.level`. Accepted from an old file with a
        /// warning, then dropped.
        #[serde(skip_serializing_if = "Option::is_none")]
        log_level: Option<String> = None,
        /// Removed in T24.5: it is now `log.path`. Accepted from an old file with a
        /// warning, then dropped.
        #[serde(skip_serializing_if = "Option::is_none")]
        log_file: Option<PathBuf> = None,
        session_env: String = s("CLAUDE_SESSION_ID"),
        call_io_inline_bytes: u32 = 65536,
        retain_calls_days: u32 = 30,
        /// Removed in T24.5: it is now `log.to_db`. Accepted from an old file with a
        /// warning, then dropped.
        #[serde(skip_serializing_if = "Option::is_none")]
        log_to_db: Option<bool> = None,
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
    /// `[log]` — rtok's own log (P24, D26). One rotating text file, plus the `logs` rows
    /// `rtok otel` exports. Legacy `[core] log_file` / `log_level` / `log_to_db` migrate here.
    Log {
        path: PathBuf = p("~/.rtok/logs/rtok.log"),
        max_bytes: u64 = 1_048_576,
        files: u32 = 5,
        lines: usize = 200,
        level: String = s("info"),
        to_db: bool = true,
    }
}

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
        /// Above this, MCP tool results use head/tail + archive id.
        max_result_chars: u32 = 20000,
    }
}

section! {
    /// `[proxy.tools_rewrite]` — opt-in `tools[]` description rewrite (T59.5). Off: bytes
    /// identical. Empty `allow` keeps every tool not in `deny`. `input_schema` is never touched.
    ToolsRewrite {
        enabled: bool = false,
        max_description_tokens: u32 = 60,
        allow: Vec<String> = Vec::new(),
        deny: Vec<String> = Vec::new(),
    }
}

section! {
    /// `[proxy]` — the proxy server itself; the usage-capture plugin is `[plugins.proxy]`.
    Proxy {
        /// When false the HTTP listener stays up but every request is byte-forwarded with
        /// no compress / bookkeeping / request shaping. Only killing the process stops HTTP.
        enabled: bool = true,
        bind: String = s("127.0.0.1"),
        port: u16 = 8790,
        mode: String = s("passthrough"),
        upstream: String = s("https://api.anthropic.com"),
        openai_upstream: String = s("https://api.openai.com"),
        gemini_upstream: String = s("https://generativelanguage.googleapis.com"),
        timeout_s: u64 = 600,
        include_usage: bool = true,
        /// Opt-in Anthropic server-side context editing (T51.2): add
        /// `context_management.edits: [{type: "clear_tool_uses_20250919"}]` plus the
        /// `context-management-2025-06-27` beta header on `/v1/messages` requests that
        /// do not already carry the field. Other wires are unaffected.
        context_management: bool = false,
        dry_run: bool = false,
        tools_rewrite: ToolsRewrite = ToolsRewrite::default(),
    }
}

section! {
    /// `[web]` — `rtok web` (P19). Slint WASM UI + WebSocket API, the same data as `rtok tui`.
    Web {
        host: String = s("127.0.0.1"),
        port: u16 = 3333,
    }
}

section! {
    /// `[tui]` — `rtok tui` (P15). The terminal rendering of the one operator model (D23).
    /// `tab` names the opening tab (`""` = first page); `tick_secs` is the model
    /// re-read cadence (2 = the web socket's tick in `src/web/mod.rs`, so both
    /// surfaces go stale at the same rate).
    Tui {
        tab: String = String::new(),
        tick_secs: u64 = 2,
    }
}

section! {
    /// `[demon]` — `rtok demon` (P20, D22). Supervises the long-running surfaces.
    Demon {
        services: Vec<String> = strs(&["proxy"]),
        state_dir: PathBuf = p("~/.rtok/demon"),
        backoff_ms: u64 = 1000,
        max_backoff_ms: u64 = 30000,
        healthy_ms: u64 = 10000,
        poll_ms: u64 = 200,
    }
}

section! {
    /// `[stats]` — `rtok stats`.
    Stats {
        since: String = s("30d"),
        format: String = s("table"),
        plugin: String = String::new(),
        transcripts_dir: PathBuf = p("~/.claude/projects"),
        /// Codex CLI session logs, read as one more `api` row (T49.2).
        codex_dir: PathBuf = p("~/.codex/sessions"),
        calibrate_samples: u32 = 30,
        baseline: String = String::new(),
        /// Show per-model USD costs from `prices` (`rtok stats --price`, T49.1).
        price: bool = false,
        /// USD per MTok per model id (`rtok stats --price`, T49.1). A `usage` row
        /// whose model has no entry here is listed with `-`, never priced by guess.
        prices: BTreeMap<String, ModelPrice> = default_stats_prices(),
    }
}

section! {
    /// One `[stats.prices."<model>"]` row — USD per MTok (T49.1). `cache_write` is
    /// the 5-minute cache-creation price; providers without a separate write price
    /// repeat `input`.
    ModelPrice {
        input: f64 = 0.0,
        cache_write: f64 = 0.0,
        cache_read: f64 = 0.0,
        output: f64 = 0.0,
    }
}

/// The shipped `[stats.prices]` rows (T49.1). Sources, fetched 2026-09-17:
/// Anthropic `claude-sonnet-5` / `claude-haiku-4-5` from
/// https://platform.claude.com/docs/en/about-claude/pricing (input / 5m write /
/// read / output per MTok); OpenAI `gpt-5` / `gpt-5-mini` from
/// https://platform.openai.com/docs/pricing (short-context input / cached input /
/// output; no separate write price, so `cache_write = input`).
fn default_stats_prices() -> BTreeMap<String, ModelPrice> {
    [
        (
            "claude-sonnet-5",
            ModelPrice {
                input: 2.0,
                cache_write: 2.5,
                cache_read: 0.2,
                output: 10.0,
            },
        ),
        (
            "claude-haiku-4-5",
            ModelPrice {
                input: 1.0,
                cache_write: 1.25,
                cache_read: 0.1,
                output: 5.0,
            },
        ),
        (
            "gpt-5",
            ModelPrice {
                input: 1.25,
                cache_write: 1.25,
                cache_read: 0.125,
                output: 10.0,
            },
        ),
        (
            "gpt-5-mini",
            ModelPrice {
                input: 0.25,
                cache_write: 0.25,
                cache_read: 0.025,
                output: 2.0,
            },
        ),
    ]
    .into_iter()
    .map(|(k, v)| (s(k), v))
    .collect()
}

section! {
    /// `[report]` — `rtok report` (P22, D24). `format` grows `html` (T22.2) and `pdf`
    /// (T22.3); `ai` and `budget_tokens` landed with T22.4, `charts` with its task.
    Report {
        format: String = s("md"),
        out: PathBuf = PathBuf::new(),
        since: String = s("30d"),
        ai: bool = false,
        budget_tokens: u32 = 8000,
    }
}

section! {
    /// `[bench]` — `rtok bench`.
    Bench {
        tasks: PathBuf = p("bench/tasks.toml"),
        runs: u32 = 3,
        dry_run: bool = false,
        timeout_s: u64 = 900,
        /// `""` = T9.1 six tasks; `"graph"` = T68.9 with/without MCP.
        suite: String = String::new(),
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
    /// `[setup]` — `rtok agents install <host>`.
    Setup {
        dry_run: bool = false,
        yes: bool = false,
        backup: bool = true,
        hook_timeout_s: u64 = 5,
        /// Seconds to wait for the post-install/uninstall restart prompt (T76).
        /// `0` (default) waits forever; a positive value treats silence as No.
        restart_prompt_timeout_seconds: u64 = 0,
        modes: Vec<String> = Vec::new(),
        mcp: bool = true,
        proxy: bool = false,
        claude: SetupClaude = SetupClaude::default(),
        cursor: SetupCursor = SetupCursor::default(),
        codex: SetupCodex = SetupCodex::default(),
        opencode: SetupOpenCode = SetupOpenCode::default(),
        kilo: SetupKilo = SetupKilo::default(),
        pi: SetupPi = SetupPi::default(),
        omp: SetupOmp = SetupOmp::default(),
        zcode: SetupZcode = SetupZcode::default(),
        kimi: SetupKimi = SetupKimi::default(),
        vscode: SetupVscode = SetupVscode::default(),
        copilot: SetupCopilot = SetupCopilot::default(),
        aider: SetupAider = SetupAider::default(),
        windsurf: SetupWindsurf = SetupWindsurf::default(),
        zed: SetupZed = SetupZed::default(),
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
    /// `[setup.kilo]` — `kilo.json`, merged by Kilo with a user's `kilo.jsonc` (T97).
    SetupKilo { config_path: PathBuf = p("~/.config/kilo/kilo.json") }
}

section! {
    /// `[setup.pi]`
    SetupPi {
        extensions_path: PathBuf = p("~/.pi/agent/extensions"),
        /// Register the measured MCP tools through `pi.registerTool` (T70.3).
        /// Off: those descriptions do not ride every pi request.
        tools: bool = false,
    }
}

section! {
    /// `[setup.omp]` — oh my pi: the linked pi extension and native `mcp.json` (T92). A named
    /// omp profile is a path override.
    SetupOmp {
        extensions_path: PathBuf = p("~/.omp/agent/extensions"),
        mcp_path: PathBuf = p("~/.omp/agent/mcp.json"),
    }
}

section! {
    /// `[setup.zcode]`
    SetupZcode { config_path: PathBuf = p("~/.zcode/cli/config.json") }
}

section! {
    /// `[setup.kimi]` — `mcp.json` is read beside `config_path`.
    SetupKimi { config_path: PathBuf = p("~/.kimi-code/config.toml") }
}

section! {
    /// `[setup.copilot]` — `mcp-config.json` and `hooks/rtok.json` live under `dir`.
    SetupCopilot { dir: PathBuf = p("~/.copilot") }
}

section! {
    /// `[setup.vscode]` — profile `mcp.json` under Code / Code - Insiders user dirs (T48.8).
    SetupVscode {
        code_user_dir: PathBuf = PathBuf::new(),
        insiders_user_dir: PathBuf = PathBuf::new(),
    }
}

section! {
    /// `[setup.aider]` — `.aider.conf.yml` carries `openai-api-base` (T48.7).
    SetupAider { config_path: PathBuf = p("~/.aider.conf.yml") }
}

section! {
    /// `[setup.windsurf]` — Cascade's `mcp_config.json` (T48.5).
    SetupWindsurf { config_path: PathBuf = p("~/.codeium/windsurf/mcp_config.json") }
}

section! {
    /// `[setup.zed]` — Zed's `settings.json` carries `context_servers` (T48.6).
    SetupZed { config_path: PathBuf = p("~/.config/zed/settings.json") }
}

section! {
    /// `[expand]` — `rtok expand <id>`.
    Expand {
        /// 0 = unlimited; caps stdout line count from `rtok expand`.
        max_lines: u32 = 0,
        /// Ceiling on the live-zone re-read rate (T22.5): above it, `rtok report`
        /// finds the compression lossier in practice than it looks.
        max_rate: f64 = 0.05,
    }
}

section! {
    /// `[filter]` — `rtok filter --stdin` (T10.2).
    Filter { cmd: String = String::new() }
}

section! {
    /// `[otel]` — OpenTelemetry export (D19, P16). Off until `endpoint` resolves.
    Otel {
        endpoint: String = String::new(),
        headers: String = String::new(),
        service_name: String = s("rtok"),
        content: bool = true,
        content_bytes: u32 = 65536,
        flush_secs: u32 = 5,
    }
}

/// Where a flush posts: `[otel] endpoint`, else `OTEL_EXPORTER_OTLP_ENDPOINT`; same for headers.
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    /// Base URL without a trailing slash; `/v1/traces` etc. are appended.
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl Otel {
    /// `None` when neither the key nor the env var names an endpoint — export is off.
    pub fn resolve(&self) -> Option<Endpoint> {
        self.resolve_with(|k| std::env::var(k).ok())
    }

    pub fn resolve_with(&self, env: impl Fn(&str) -> Option<String>) -> Option<Endpoint> {
        let pick = |key: &str, var: &str| -> String {
            if key.trim().is_empty() {
                env(var).unwrap_or_default()
            } else {
                key.to_string()
            }
        };
        let url = pick(&self.endpoint, "OTEL_EXPORTER_OTLP_ENDPOINT");
        let url = url.trim().trim_end_matches('/').to_string();
        if url.is_empty() {
            return None;
        }
        let headers = pick(&self.headers, "OTEL_EXPORTER_OTLP_HEADERS")
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .filter(|(k, _)| !k.is_empty())
            .collect();
        Some(Endpoint { url, headers })
    }
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
        compress: Compress = Compress::default(),
        wasm: Wasm = Wasm::default(),
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
        /// Drop-in dir: every `*.toml` merges after `rules` in name order (T50.2).
        rules_dir: PathBuf = p("~/.rtok/rules.d"),
        trailer_min_lines: u32 = 40,
        fail_tail_lines: u32 = 80,
        never_wrap: Vec<String> = strs(&["rtok", "sudo"]),
        interactive_stems: Vec<String> = strs(&["python", "node", "psql", "sqlite3", "irb", "bash", "sh", "zsh", "docker", "kubectl"]),
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
        /// Max bytes `search` will read from one file (T55.5). Larger files are skipped.
        search_max_bytes: u64 = 1_048_576,
        tree_depth: u32 = 2,
        /// T58.1: re-read of a changed file returns a unified diff vs the last archive.
        delta: bool = true,
        /// Serve the full file when the diff is not below this fraction of the file.
        delta_max_ratio: f32 = 0.6,
        /// Deprecated compatibility key; grammars are Cargo features now, so this is ignored.
        languages: Vec<String> = Vec::new(),
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
        /// Opt-in L0/L1/L2 tiered loading (P33). Default off — v0.1 archive+inject unchanged.
        /// Behaviour spec: OpenViking (AGPL-3.0); rtok does not vendor, link, or subprocess it
        /// (D6). Gates native implementation in T33.2.
        tiers: bool = false,
        /// Opt-in live-zone blob shrinking (T51.1): nested JSON dumps and `data:` blobs
        /// in user content blocks (never tool results, system, tools, or the last two
        /// turns). Default off until a bench shows cost per passed task does not rise.
        live_blobs: bool = false,
        /// Opt-in skill body archiving in the live zone (T61.2).
        skills: bool = false,
    }
}

section! {
    /// `[plugins.proxy.semantic_cache]` — opt-in response cache (P31). Off until Gate P31.
    SemanticCache {
        enabled: bool = false,
        threshold: f32 = 0.99,
        ttl_s: u64 = 300,
        max_messages: u32 = 1,
        require_empty_tools: bool = true,
        embed_backend: String = s("hash"),
        cache_by_model: bool = true,
        cache_by_provider: bool = true,
    }
}

section! {
    /// `[plugins.proxy]` — the usage-capture plugin, not the `[proxy]` server.
    ProxyPlugin {
        enabled: bool = true,
        semantic_cache: SemanticCache = SemanticCache::default(),
    }
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
        /// Opt-in (T50.4): deny native `Grep`/`Glob` in PreToolUse and point at
        /// MCP `search`/`tree`. Off by default; also stays silent while the
        /// `read` plugin is disabled (no `search`/`tree` to point at).
        deny_grep_glob: bool = false,
        /// Opt-in (T62.1): on `PreToolUse(Skill)` a `SKILL.md` over `skill_max_bytes`
        /// is archived and denied with its markdown map plus the `expand` pointer,
        /// unless its frontmatter carries a key the host applies on invocation.
        skills: bool = false,
        skill_max_bytes: u32 = 8192,
    }
}

section! {
    /// `[plugins.memory.embed]` — optional vector search beside FTS5 (P29; off by default).
    MemoryEmbed {
        enabled: bool = false,
        provider: String = s("local"),
        model: String = s("all-MiniLM-L6-v2"),
        dimensions: u32 = 384,
        hybrid: bool = true,
    }
}

section! {
    /// `[plugins.memory]`
    Memory {
        enabled: bool = true,
        recall_titles: u32 = 5,
        recall_tokens: u32 = 200,
        prompt_recall: u32 = 0,
        checkpoint_tokens: u32 = 400,
        search_limit: u32 = 5,
        sync_tokens: u32 = 300,
        /// SessionStart `source = startup` restores the newest `session:*` note (T71.2). Off
        /// until a P7-style A/B shows cost per passed task does not rise.
        startup_recall: bool = false,
        /// Sub-agent handoff MCP tool (T59.6); off by default.
        handoff: bool = false,
        embed: MemoryEmbed = MemoryEmbed::default(),
    }
}

section! {
    /// `[plugins.graph]`
    Graph {
        enabled: bool = true,
        max_tokens: u32 = 2000,
        map_tokens: u32 = 0,
        body_lines: u32 = 40,
        auto_index: bool = true,
        backend: String = s("tags"),
        watch: String = s("off"),
        exclude: Vec<String> = vec![],
        include: Vec<String> = vec![],
        extensions: std::collections::HashMap<String, String> = std::collections::HashMap::new(),
    }
}

section! {
    /// `[plugins.toon]`
    Toon {
        enabled: bool = true,
        min_rows: u32 = 5,
    }
}

section! {
    /// `[plugins.compress]`
    Compress {
        enabled: bool = true,
    }
}

section! {
    /// `[plugins.wasm]` — out-of-tree `.wasm` plugin host (P32). Not a catalogue id;
    /// loading is implemented in T32.2 behind Cargo feature `wasm-host`.
    Wasm {
        enabled: bool = false,
        dir: PathBuf = p("~/.rtok/plugins"),
    }
}

// ── the whole file ──────────────────────────────────────────────────────────

/// Every setting rtok has. Sections mirror the tables in `config/default.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub core: Core,
    pub estimator: Estimator,
    pub log: Log,
    pub hook: Hook,
    pub mcp: Mcp,
    pub proxy: Proxy,
    pub web: Web,
    pub tui: Tui,
    /// Renamed in T21.3: `[dashboard]` is now `[web]`. Accepted from an old file with a
    /// warning, then dropped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<Web>,
    pub demon: Demon,
    pub stats: Stats,
    pub report: Report,
    pub bench: Bench,
    pub doctor: Doctor,
    pub setup: Setup,
    pub expand: Expand,
    pub filter: Filter,
    pub otel: Otel,
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
        env_user_home().unwrap_or_default().join(".rtok")
    }

    /// `<home>/config.toml`.
    pub fn path_for(home: &Path) -> PathBuf {
        home.join("config.toml")
    }

    /// User config file: `--config`, else `RTOK_CONFIG`, else [`path_for`].
    pub fn user_path(home: &Path, config_file: Option<&Path>) -> PathBuf {
        config_file
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("RTOK_CONFIG").map(PathBuf::from))
            .unwrap_or_else(|| Self::path_for(home))
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
        Self::init_maybe(home, None, force, false).map(|(p, _)| p)
    }

    /// [`Config::init`] with a preview: `dry_run` renders the `git diff` it would write and
    /// leaves the disk alone. The diff is empty when the file already is the reference file.
    pub fn init_maybe(
        home: &Path,
        config_file: Option<&Path>,
        force: bool,
        dry_run: bool,
    ) -> Result<(PathBuf, String)> {
        let path = Self::user_path(home, config_file);
        if path.exists() && !force {
            bail!("{} exists; pass --force to overwrite", path.display());
        }
        let before = std::fs::read_to_string(&path).unwrap_or_default();
        let diff = crate::render::file_diff(&path, &before, DEFAULT_TOML);
        if !dry_run {
            write_file(&path, DEFAULT_TOML)?;
        }
        Ok((path, diff))
    }

    /// Migrate legacy keys and expand `~` in paths. Called after every parse.
    fn finish(&mut self, home: &Path) {
        apply_legacy_fold(self);
        if let Some(budget) = self.core.inject_budget_tokens.take()
            && self.plugins.inject.budget_tokens == budget
        {
            eprintln!(
                "rtok: core.inject_budget_tokens is now plugins.inject.budget_tokens (using {budget})"
            );
        }
        if let Some(web) = self.dashboard.take()
            && self.web == web
        {
            eprintln!("rtok: [dashboard] is now [web] (using it)");
        }
        // T24.5 / D26: `[core] log_*` → `[log]`. Taken once so they are not re-read.
        if let Some(path) = self.core.log_file.take()
            && self.log.path == path
        {
            eprintln!(
                "rtok: core.log_file is now log.path (using {})",
                path.display()
            );
        }
        if let Some(level) = self.core.log_level.take()
            && self.log.level == level
        {
            eprintln!("rtok: core.log_level is now log.level (using {level})");
        }
        if let Some(to_db) = self.core.log_to_db.take()
            && self.log.to_db == to_db
        {
            eprintln!("rtok: core.log_to_db is now log.to_db (using {to_db})");
        }
        self.home = home.to_path_buf();
        for path in [
            &mut self.core.db_path,
            &mut self.core.archive_dir,
            &mut self.log.path,
            &mut self.demon.state_dir,
            &mut self.stats.transcripts_dir,
            &mut self.stats.codex_dir,
            &mut self.report.out,
            &mut self.bench.tasks,
            &mut self.doctor.settings_path,
            &mut self.doctor.claude_json,
            &mut self.doctor.mcp_json,
            &mut self.setup.claude.settings_path,
            &mut self.setup.cursor.hooks_path,
            &mut self.setup.codex.config_path,
            &mut self.setup.opencode.config_path,
            &mut self.setup.kilo.config_path,
            &mut self.setup.pi.extensions_path,
            &mut self.setup.zcode.config_path,
            &mut self.setup.kimi.config_path,
            &mut self.setup.copilot.dir,
            &mut self.setup.vscode.code_user_dir,
            &mut self.setup.vscode.insiders_user_dir,
            &mut self.setup.aider.config_path,
            &mut self.setup.windsurf.config_path,
            &mut self.setup.zed.config_path,
            &mut self.plugins.cmd.rules,
            &mut self.plugins.cmd.rules_dir,
            &mut self.plugins.inject.modes_dir,
            &mut self.plugins.wasm.dir,
        ] {
            *path = expand(path, home);
        }
        for path in self.bench.configs.values_mut() {
            *path = expand(path, home);
        }
        for path in &mut self.plugins.read.allow_paths {
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
            "compress" => p.compress.enabled,
            _ => default_on,
        }
    }

    /// The writer mirror of [`Config::plugin_enabled`]: set `[plugins.<id>] enabled` on
    /// the in-memory copy after the file was written through `validate::set` (the TUI's
    /// plugin toggle, T15.4). An id outside the catalogue is nothing to mirror here —
    /// `validate::set` is what refuses it against the schema.
    pub fn set_plugin_enabled(&mut self, id: &str, on: bool) {
        let p = &mut self.plugins;
        match id {
            "measure" => p.measure.enabled = on,
            "cmd" => p.cmd.enabled = on,
            "read" => p.read.enabled = on,
            "archive" => p.archive.enabled = on,
            "proxy" => p.proxy.enabled = on,
            "inject" => p.inject.enabled = on,
            "guard" => p.guard.enabled = on,
            "memory" => p.memory.enabled = on,
            "graph" => p.graph.enabled = on,
            "toon" => p.toon.enabled = on,
            "compress" => p.compress.enabled = on,
            _ => {}
        }
    }
}

/// Fold legacy file keys into their replacements only while each target is still at
/// [`Config::default()`]. Env, flags, and an explicit new key win (T36.8).
pub(crate) fn apply_legacy_fold(cfg: &mut Config) {
    let defaults = Config::default();
    if let Some(budget) = cfg.core.inject_budget_tokens
        && cfg.plugins.inject.budget_tokens == defaults.plugins.inject.budget_tokens
    {
        cfg.plugins.inject.budget_tokens = budget;
    }
    if let Some(dash) = cfg.dashboard.as_ref()
        && cfg.web.host == defaults.web.host
        && dash.host != defaults.web.host
    {
        cfg.web.host = dash.host.clone();
    }
    if let Some(dash) = cfg.dashboard.as_ref()
        && cfg.web.port == defaults.web.port
        && dash.port != defaults.web.port
    {
        cfg.web.port = dash.port;
    }
    if let Some(path) = cfg.core.log_file.as_ref()
        && cfg.log.path == defaults.log.path
    {
        cfg.log.path = path.clone();
    }
    if let Some(level) = cfg.core.log_level.as_ref()
        && cfg.log.level == defaults.log.level
    {
        cfg.log.level = level.clone();
    }
    if let Some(to_db) = cfg.core.log_to_db
        && cfg.log.to_db == defaults.log.to_db
    {
        cfg.log.to_db = to_db;
    }
}

/// User home for `~` expansion and `$HOME/.rtok`.
///
/// Prefer `HOME` (Unix and Git Bash). On native Windows PowerShell `HOME` is
/// often unset — fall back to `USERPROFILE` so `rtok agents install` finds
/// `~/.claude` / `~/.cursor` instead of skipping with "not found".
pub(crate) fn env_user_home() -> Option<PathBuf> {
    user_home_from(std::env::var_os("HOME"), std::env::var_os("USERPROFILE"))
}

/// Resolve home from explicit env values (testable without mutating the process).
pub(crate) fn user_home_from(
    home: Option<std::ffi::OsString>,
    userprofile: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    // Empty HOME must not block USERPROFILE (Windows PowerShell often has
    // HOME="" rather than unset).
    nonempty_home(home).or_else(|| nonempty_home(userprofile))
}

fn nonempty_home(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `~/.rtok/x` → `<home>/x` (so `RTOK_HOME` moves the whole tree), other `~/x` → `$HOME/x`.
/// Bare `~` and `~/.rtok` (no trailing slash) expand too — leaving them literal is how
/// tests without `finish` used to create a `./~` directory in the repo.
fn expand(path: &Path, home: &Path) -> PathBuf {
    expand_with(path, home, env_user_home().as_deref())
}

/// Like [`expand`], but takes an explicit user-home so Windows `USERPROFILE`
/// fallback can be tested without mutating process env.
fn expand_with(path: &Path, rtok_home: &Path, user_home: Option<&Path>) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = strip_rtok_home_prefix(&raw) {
        return match rest {
            "" => rtok_home.to_path_buf(),
            rest => join_tilde_rest(rtok_home, rest),
        };
    }
    if raw == "~" {
        return user_home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = strip_home_prefix(&raw) {
        return match user_home {
            Some(h) => join_tilde_rest(h, rest),
            None => path.to_path_buf(),
        };
    }
    path.to_path_buf()
}

fn strip_rtok_home_prefix(raw: &str) -> Option<&str> {
    for prefix in ["~/.rtok/", "~/.rtok\\", "~\\.rtok\\", "~\\.rtok/"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            return Some(rest);
        }
    }
    match raw {
        "~/.rtok" | "~/.rtok/" | "~/.rtok\\" | "~\\.rtok" | "~\\.rtok\\" | "~\\.rtok/" => Some(""),
        _ => None,
    }
}

fn strip_home_prefix(raw: &str) -> Option<&str> {
    raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
}

/// Join a tilde-relative remainder that may use `/` or `\\` separators.
fn join_tilde_rest(base: &Path, rest: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in rest.split(['/', '\\']).filter(|s| !s.is_empty()) {
        out.push(part);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::Figment;
    use figment::providers::{Format, Toml};
    use rstest::rstest;

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

    /// Every `PathBuf` leaf in a finished config — catches a new path key without `expand`.
    fn path_leaves(cfg: &Config) -> Vec<&PathBuf> {
        let mut out = vec![
            &cfg.core.db_path,
            &cfg.core.archive_dir,
            &cfg.log.path,
            &cfg.demon.state_dir,
            &cfg.stats.transcripts_dir,
            &cfg.report.out,
            &cfg.bench.tasks,
            &cfg.doctor.settings_path,
            &cfg.doctor.claude_json,
            &cfg.doctor.mcp_json,
            &cfg.setup.claude.settings_path,
            &cfg.setup.cursor.hooks_path,
            &cfg.setup.codex.config_path,
            &cfg.setup.opencode.config_path,
            &cfg.setup.kilo.config_path,
            &cfg.setup.pi.extensions_path,
            &cfg.setup.zcode.config_path,
            &cfg.setup.kimi.config_path,
            &cfg.setup.copilot.dir,
            &cfg.setup.aider.config_path,
            &cfg.setup.windsurf.config_path,
            &cfg.setup.zed.config_path,
            &cfg.plugins.cmd.rules,
            &cfg.plugins.cmd.rules_dir,
            &cfg.plugins.inject.modes_dir,
            &cfg.plugins.wasm.dir,
        ];
        out.extend(cfg.bench.configs.values());
        out.extend(&cfg.plugins.read.allow_paths);
        if let Some(path) = &cfg.core.log_file {
            out.push(path);
        }
        out
    }

    fn assert_paths_expanded(cfg: &Config) {
        for path in path_leaves(cfg) {
            assert!(
                !path.to_string_lossy().starts_with('~'),
                "unexpanded path {}",
                path.display()
            );
        }
    }

    /// The Check for T12.1: the reference file is the defaults, exactly.
    #[test]
    fn otel_is_off_until_an_endpoint_resolves() {
        let o = Otel::default();
        assert_eq!(o.resolve_with(|_| None), None);
        let env = |k: &str| match k {
            "OTEL_EXPORTER_OTLP_ENDPOINT" => Some("http://localhost:4318/".to_string()),
            "OTEL_EXPORTER_OTLP_HEADERS" => Some("a=1, b=x=y".to_string()),
            _ => None,
        };
        let e = o.resolve_with(env).unwrap();
        assert_eq!(e.url, "http://localhost:4318");
        assert_eq!(
            e.headers,
            vec![("a".into(), "1".into()), ("b".into(), "x=y".into())]
        );
        let o = Otel {
            endpoint: "https://otel.example/".into(),
            headers: "signoz-ingestion-key=k".into(),
            ..Otel::default()
        };
        let e = o.resolve_with(env).unwrap();
        assert_eq!(e.url, "https://otel.example");
        assert_eq!(e.headers, vec![("signoz-ingestion-key".into(), "k".into())]);
    }

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

    /// The writer mirror round-trips through the reader for every catalogue id (T15.4).
    #[test]
    fn set_plugin_enabled_flips_every_catalogue_id() {
        let mut cfg = Config::default();
        for (id, on) in CATALOGUE {
            cfg.set_plugin_enabled(id, !on);
            assert_eq!(cfg.plugin_enabled(id, on), !on, "{id}");
        }
        // An id outside the catalogue is nothing to set; the reader's fallback stands.
        cfg.set_plugin_enabled("external", false);
        assert!(cfg.plugin_enabled("external", true));
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
        assert!(cfg.plugin_enabled("toon", true));
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
    fn tools_rewrite_defaults_overlay_and_unknown_key() {
        let d = ToolsRewrite::default();
        assert!(!d.enabled);
        assert_eq!(d.max_description_tokens, 60);
        assert!(d.allow.is_empty());
        assert!(d.deny.is_empty());
        let cfg: Config = parse(
            "[proxy.tools_rewrite]
enabled = true
max_description_tokens = 40
deny = [\"Bash\"]
",
        )
        .unwrap();
        assert!(cfg.proxy.tools_rewrite.enabled);
        assert_eq!(cfg.proxy.tools_rewrite.max_description_tokens, 40);
        assert_eq!(cfg.proxy.tools_rewrite.deny, ["Bash"]);
        let err = parse("[proxy.tools_rewrite]\nbogus = true\n").unwrap_err();
        assert!(err.to_string().contains("bogus"), "{err}");
    }

    #[test]
    fn semantic_cache_defaults_overlay_and_unknown_key() {
        let d = SemanticCache::default();
        assert!(!d.enabled);
        assert_eq!(d.threshold, 0.99);
        assert_eq!(d.ttl_s, 300);
        assert_eq!(d.max_messages, 1);
        assert!(d.require_empty_tools);
        assert_eq!(d.embed_backend, "hash");
        assert!(d.cache_by_model);
        assert!(d.cache_by_provider);

        let cfg: Config = parse(
            "[plugins.proxy.semantic_cache]
enabled = true
threshold = 0.95
",
        )
        .unwrap();
        assert!(cfg.plugins.proxy.semantic_cache.enabled);
        assert_eq!(cfg.plugins.proxy.semantic_cache.threshold, 0.95);

        let err = parse(
            "[plugins.proxy.semantic_cache]
bogus = true
",
        )
        .unwrap_err();
        assert!(err.to_string().contains("bogus"), "{err}");
    }

    #[test]
    fn read_languages_is_accepted_only_for_compatibility() {
        let cfg = parse("[plugins.read]\nlanguages = [\"rust\", \"ts\"]\n")
            .expect("legacy languages key should still parse");
        assert_eq!(cfg.plugins.read.languages, ["rust", "ts"]);

        let err = parse("[plugins.read]\nlangauges = [\"rust\"]\n").unwrap_err();
        assert!(err.to_string().contains("langauges"), "{err}");
    }

    #[test]
    fn unknown_key_is_an_error() {
        let err = parse("[proxy]\nprot = 1\n").unwrap_err();
        assert!(err.to_string().contains("prot"), "{err}");
        assert!(parse("[nope]\nx = 1\n").is_err());
        assert!(parse("[plugins.compress]\nunknown = true\n").is_err());
        let err = parse("[plugins.memory.embed]\nprot = 1\n").unwrap_err();
        assert!(err.to_string().contains("prot"), "{err}");
        assert!(parse("[plugins.archive]\nno_such = 1\n").is_err());
    }

    #[test]
    fn compress_defaults_on_and_overlays_turn_off() {
        use super::layers;

        assert!(Config::default().plugins.compress.enabled);

        let cfg: Config = parse("[plugins.compress]\nenabled = false\n").unwrap();
        assert!(!cfg.plugins.compress.enabled);

        let home = tmp("compress-env");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".env"), "RTOK_PLUGINS_COMPRESS_ENABLED=false\n").unwrap();
        let cfg = layers::load(&home, None, None).unwrap();
        assert!(!cfg.plugins.compress.enabled);
        let row = layers::entries(&layers::figment(&home, None, None))
            .into_iter()
            .find(|(k, _, _)| k == "plugins.compress.enabled")
            .expect("leaf key listed");
        assert_eq!(row.1, "false");
        assert_eq!(row.2, "dotenv");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn memory_embed_defaults_off() {
        let cfg = Config::default();
        assert!(!cfg.plugins.memory.embed.enabled);
        assert_eq!(cfg.plugins.memory.embed.provider, "local");
        assert_eq!(cfg.plugins.memory.embed.model, "all-MiniLM-L6-v2");
        assert_eq!(cfg.plugins.memory.embed.dimensions, 384);
        assert!(cfg.plugins.memory.embed.hybrid);
    }

    #[test]
    fn memory_embed_enabled_from_file() {
        let cfg: Config = parse("[plugins.memory.embed]\nenabled = true\n").unwrap();
        assert!(cfg.plugins.memory.embed.enabled);
    }

    #[test]
    fn graph_backend_defaults_to_tags() {
        assert_eq!(Config::default().plugins.graph.backend, "tags");
        assert_eq!(Config::default().plugins.graph.map_tokens, 0);
    }

    #[test]
    fn graph_backend_overlay_from_toml() {
        let cfg = parse("[plugins.graph]\nbackend = \"lsp\"\n").unwrap();
        assert_eq!(cfg.plugins.graph.backend, "lsp");
    }

    #[test]
    fn graph_backend_unknown_key_is_an_error() {
        assert!(parse("[plugins.graph]\nback_end = \"lsp\"\n").is_err());
    }

    /// T33.1: `plugins.archive.tiers` defaults off, overlays, and maps to `RTOK_PLUGINS_ARCHIVE_TIERS`.
    #[test]
    fn archive_tiers_defaults_and_overlays() {
        assert!(!Config::default().plugins.archive.tiers);
        let cfg: Config = parse("[plugins.archive]\ntiers = true\n").unwrap();
        assert!(cfg.plugins.archive.tiers);
        assert!(layers::leaf_keys().contains(&"plugins.archive.tiers".to_string()));
    }

    #[test]
    fn default_expands_every_pathbuf() {
        let home = Path::new("/tmp/rtok-tilde-default");
        let mut cfg = Config::default();
        cfg.finish(home);
        assert_paths_expanded(&cfg);
    }

    #[rstest]
    #[case::report_out("[report]\nout = \"~/rtok-report.md\"\n")]
    #[case::bench_tasks("[bench]\ntasks = \"~/bench/tasks.toml\"\n")]
    #[case::bench_configs("[bench.configs]\ncustom = \"~/bench/rtok.json\"\n")]
    #[case::allow_paths("[plugins.read]\nallow_paths = [\"~/src\"]\n")]
    #[case::wasm_dir("[plugins.wasm]\ndir = \"~/plugins\"\n")]
    #[case::doctor_mcp_json("[doctor]\nmcp_json = \"~/.mcp.json\"\n")]
    fn tilde_expands_for_every_path_key(#[case] toml: &str) {
        let home = Path::new("/tmp/rtok-tilde-keys");
        let mut cfg: Config = parse(toml).unwrap();
        cfg.finish(home);
        assert_paths_expanded(&cfg);
    }

    #[test]
    fn user_home_prefers_home_then_userprofile() {
        use std::ffi::OsString;
        assert_eq!(
            user_home_from(
                Some(OsString::from("/Users/me")),
                Some(OsString::from(r"C:\Users\me"))
            ),
            Some(PathBuf::from("/Users/me"))
        );
        assert_eq!(
            user_home_from(None, Some(OsString::from(r"C:\Users\Example"))),
            Some(PathBuf::from(r"C:\Users\Example"))
        );
        // Empty HOME must fall through to USERPROFILE (native Windows).
        assert_eq!(
            user_home_from(
                Some(OsString::from("")),
                Some(OsString::from(r"C:\Users\Example"))
            ),
            Some(PathBuf::from(r"C:\Users\Example"))
        );
        assert_eq!(user_home_from(None, None), None);
        assert_eq!(
            user_home_from(Some(OsString::from("")), Some(OsString::from(""))),
            None
        );
    }

    #[test]
    fn expand_covers_bare_tilde_and_rtok_home_dir() {
        let home = Path::new("/tmp/rtok-home");
        assert_eq!(expand(Path::new("~/.rtok"), home), home);
        assert_eq!(expand(Path::new("~/.rtok/"), home), home);
        assert_eq!(expand(Path::new("~/.rtok/db"), home), home.join("db"));
        if let Some(h) = env_user_home() {
            assert_eq!(expand(Path::new("~"), home), h);
            assert_eq!(
                expand(Path::new("~/.claude/settings.json"), home),
                h.join(".claude/settings.json")
            );
        }
    }

    #[test]
    fn expand_with_userprofile_resolves_cursor_and_claude_paths() {
        // Regression: without this, Windows `rtok agents install cursor|claude`
        // printed "not found, not installed" because `~/...` stayed literal.
        let rtok = Path::new("/tmp/rtok-home");
        let profile = Path::new(r"C:\Users\Example");
        assert_eq!(
            expand_with(Path::new("~/.cursor/hooks.json"), rtok, Some(profile)),
            profile.join(".cursor/hooks.json")
        );
        assert_eq!(
            expand_with(Path::new("~/.cursor/mcp.json"), rtok, Some(profile)),
            profile.join(".cursor/mcp.json")
        );
        assert_eq!(
            expand_with(Path::new("~/.claude/settings.json"), rtok, Some(profile)),
            profile.join(".claude/settings.json")
        );
        assert_eq!(
            expand_with(Path::new("~/.claude.json"), rtok, Some(profile)),
            profile.join(".claude.json")
        );
        // No user home → leave `~/...` literal (same as pre-fix Windows).
        assert_eq!(
            expand_with(Path::new("~/.cursor/hooks.json"), rtok, None),
            PathBuf::from("~/.cursor/hooks.json")
        );
        // PowerShell-style backslash tilde paths must expand too.
        assert_eq!(
            expand_with(Path::new("~\\.cursor\\hooks.json"), rtok, Some(profile)),
            profile.join(".cursor").join("hooks.json")
        );
        assert_eq!(
            expand_with(Path::new("~\\.rtok\\db"), rtok, Some(profile)),
            rtok.join("db")
        );
        assert_eq!(
            expand_with(Path::new("~/.rtok\\db"), rtok, Some(profile)),
            rtok.join("db")
        );
    }

    #[test]
    fn legacy_budget_key_migrates() {
        let mut cfg: Config = parse("[core]\ninject_budget_tokens = 250\n").unwrap();
        cfg.finish(Path::new("/tmp/rtok-legacy"));
        assert_eq!(cfg.plugins.inject.budget_tokens, 250);
        assert_eq!(cfg.core.inject_budget_tokens, None);
    }

    /// T24.5: an old `[core] log_*` file folds into `[log]` once and clears the legacy keys.
    #[test]
    fn legacy_core_log_keys_migrate_into_log() {
        let mut cfg: Config = parse(
            "[core]\nlog_file = \"/tmp/old.log\"\nlog_level = \"debug\"\nlog_to_db = false\n",
        )
        .unwrap();
        cfg.finish(Path::new("/tmp/rtok-legacy-log"));
        assert_eq!(cfg.log.path, PathBuf::from("/tmp/old.log"));
        assert_eq!(cfg.log.level, "debug");
        assert!(!cfg.log.to_db);
        assert_eq!(cfg.core.log_file, None);
        assert_eq!(cfg.core.log_level, None);
        assert_eq!(cfg.core.log_to_db, None);
    }

    /// T24.5: `config validate` rejects legacy keys — they are absent from the reference schema.
    #[test]
    fn validate_rejects_legacy_core_log_keys() {
        let dir = std::env::temp_dir().join(format!("rtok-val-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.toml");
        std::fs::write(
            &path,
            "[core]\nlog_file = \"/tmp/x.log\"\nlog_level = \"debug\"\nlog_to_db = false\n",
        )
        .unwrap();
        let errs = validate::issues(&path).unwrap();
        let joined = errs.join("\n");
        assert!(
            joined.contains("log_file")
                || joined.contains("log_level")
                || joined.contains("log_to_db"),
            "expected unknown-key errors, got {errs:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn wasm_disabled_by_default() {
        assert!(!Config::default().plugins.wasm.enabled);
    }

    #[test]
    fn wasm_overlay_enables() {
        let cfg: Config = parse("[plugins.wasm]\nenabled = true\n").unwrap();
        assert!(cfg.plugins.wasm.enabled);
    }

    #[test]
    fn wasm_unknown_key_is_denied() {
        let err = parse("[plugins.wasm]\nfuel_per_call = 1\n").unwrap_err();
        assert!(err.to_string().contains("fuel_per_call"), "{err}");
    }
}
