//! `rtok doctor` (plan T1.4): hooks, MCP servers, proxy chain.
//!
//! Since T15.11 the probes live in [`page`] and the text in [`Report::to_text`]: the page is
//! what the operator model serves (D27) and what a `rtok web` / `rtok tui` Doctor page will
//! render; the command is one renderer of it.

use crate::config::Config;
use crate::tokens::{self, Class};
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// What `rtok doctor` found, as data.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub hooks_total: usize,
    pub hooks_by_event: BTreeMap<String, usize>,
    /// One probed MCP server: name, command, tool count, description tokens.
    pub mcp: Vec<ServerInfo>,
    /// The proxy chain behind `ANTHROPIC_BASE_URL`, hops joined with `→`.
    pub proxy: String,
    /// The proxy chain behind the OpenAI seed (`OPENAI_BASE_URL` or a host config).
    pub proxy_openai: String,
    /// `ANTHROPIC_BASE_URL` is set, so MCP tool search is likely disabled.
    pub mcp_tool_search_disabled: bool,
    pub bash_max_output_length: Option<String>,
    pub auto_compact_window: Option<String>,
    /// Read-class token share from the transcripts (`None` = no data, fail open).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_share: Option<ReadShare>,
    /// The instruction audit (T7.2): `Some` only when `[doctor] instructions` ran — an audit
    /// that found nothing still prints its section header, as it always did.
    pub instructions: Option<Instructions>,
    /// The skills audit (T61.3): `Some` when the probe ran; an empty tree prints no section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillsAudit>,
    /// Host-native features that duplicate a running rtok surface (T59.7), each
    /// naming the rtok config key that turns the duplicate side off. Advice: the
    /// lines say "duplicate", never "saves N".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlaps: Vec<String>,
    /// Advice for enabling `[proxy.tools_rewrite]` when applicable (T59.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools_rewrite_advice: Option<String>,
    /// Every host variant and the state of each rtok module in it, as `agent setup` prints.
    pub agents: Vec<AgentModules>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentModules {
    pub host: &'static str,
    pub kind: &'static str,
    pub modules: Vec<crate::agents::ModuleRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub cmd: String,
    pub tools: usize,
    pub desc_tokens: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Instructions {
    pub rows: Vec<InstructionRow>,
    pub duplicates: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstructionRow {
    pub name: String,
    pub tokens: u32,
    pub path: String,
    pub warn: bool,
}

/// Share of Read-class transcript tokens spent in native Grep/Glob (T50.4):
/// `(grep + glob) / (read + grep + glob)` by estimated tokens. `None` when the
/// transcripts hold no Read-class results — the default stays off on no data.
#[derive(Debug, Clone, Serialize)]
pub struct ReadShare {
    pub read_tokens: u64,
    pub grep_tokens: u64,
    pub glob_tokens: u64,
    pub share: f64,
}

/// The skills audit (T61.3): what the host lists and what it costs the system
/// prompt. Advice only — nothing here edits a file.
#[derive(Debug, Clone, Serialize)]
pub struct SkillsAudit {
    pub rows: Vec<SkillRow>,
    /// Description bytes the listing rides with every request (≈ tokens/4).
    pub desc_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillRow {
    pub name: String,
    /// `user`, `project`, or `plugin:<id>@<marketplace>`.
    pub source: String,
    pub desc_chars: usize,
    pub body_bytes: u64,
    /// Invocations in the last 30 d from the T61.1 fold; `None` = no data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocations: Option<u64>,
    /// `description:` over the measured 200-char median.
    pub warn_desc: bool,
    /// Body over 8 KB — almost always a `references/` candidate.
    pub warn_body: bool,
    /// Listed but never invoked in the window (only when data exists).
    pub warn_never: bool,
}

impl Report {
    /// The doctor text the reports and the TUI embed: plain, no marks or colour.
    pub fn to_text(&self) -> String {
        self.render(false)
    }

    /// `rtok doctor` on a console: [`Self::to_text`] with the module marks and colours.
    pub fn to_console(&self) -> String {
        self.render(true)
    }

    fn render(&self, console: bool) -> String {
        let mut out = format!("hooks {}\n", self.hooks_total);
        for (ev, n) in &self.hooks_by_event {
            out.push_str(&format!("  {ev} {n}\n"));
        }
        out.push_str("mcp\n");
        for s in &self.mcp {
            out.push_str(&format!(
                "  {} ({} tools, ~{} desc tokens) {}\n",
                s.name, s.tools, s.desc_tokens, s.cmd
            ));
        }
        out.push_str(&format!("proxy {}\n", self.proxy));
        out.push_str(&format!("proxy openai {}\n", self.proxy_openai));
        if self.mcp_tool_search_disabled {
            out.push_str("mcp_tool_search likely disabled (ANTHROPIC_BASE_URL is set)\n");
        }
        out.push_str(&format!(
            "BASH_MAX_OUTPUT_LENGTH {}\n",
            self.bash_max_output_length.as_deref().unwrap_or("(unset)")
        ));
        out.push_str(&format!(
            "autoCompactWindow {}\n",
            self.auto_compact_window.as_deref().unwrap_or("(unset)")
        ));
        match &self.read_share {
            Some(s) => out.push_str(&format!(
                "read-share grep+glob {:.1}% of read-class tokens (read {}, grep {}, glob {})\n",
                s.share * 100.0,
                s.read_tokens,
                s.grep_tokens,
                s.glob_tokens
            )),
            None => out.push_str("read-share no data\n"),
        };
        out.push_str("agents\n");
        for a in &self.agents {
            out.push_str(&format!("  {} ({})\n", a.host, a.kind));
            out.push_str(&crate::agents::module_lines(&a.modules, "    ", console));
        }
        if let Some(audit) = &self.instructions {
            out.push_str("instructions\n");
            for r in &audit.rows {
                out.push_str(&format!(
                    "  {} {} tokens {}{}\n",
                    r.name,
                    r.tokens,
                    r.path,
                    if r.warn { " WARN" } else { "" }
                ));
            }
            for (sent, names) in &audit.duplicates {
                out.push_str(&format!("  duplicate `{sent}` in {}\n", names.join(", ")));
            }
        }
        if !self.overlaps.is_empty() {
            out.push_str("overlaps\n");
            for l in &self.overlaps {
                out.push_str(&format!("  {l}\n"));
            }
        }
        if let Some(skills) = &self.skills
            && !skills.rows.is_empty()
        {
            out.push_str(&format!(
                "skills ({} listed, {} desc bytes ≈ {} tokens per request)\n",
                skills.rows.len(),
                skills.desc_bytes,
                skills.desc_bytes / 4
            ));
            for r in &skills.rows {
                let mut flags = String::new();
                if r.warn_desc {
                    flags.push_str(" WARN desc>200");
                }
                if r.warn_body {
                    flags.push_str(" WARN body>8K (references/)");
                }
                if r.warn_never {
                    flags.push_str(" WARN never invoked");
                }
                out.push_str(&format!(
                    "  {} {} desc {}c body {}B calls {}{}\n",
                    r.name,
                    r.source,
                    r.desc_chars,
                    r.body_bytes,
                    r.invocations.map_or_else(|| "-".into(), |n| n.to_string()),
                    flags
                ));
            }
        }
        out
    }
}

/// Advice for enabling `[proxy.tools_rewrite]` when all four conditions hold.
/// Returns the advice line, or None if conditions are not met.
fn tools_rewrite_advice(
    mcp_tool_search_disabled: bool,
    proxy: &str,
    rtok_port: u16,
    total_desc_tokens: u32,
    tools_rewrite_enabled: bool,
    threshold: u32,
) -> Option<String> {
    if mcp_tool_search_disabled
        && proxy
            .split('→')
            .any(|h| h == rtok_port.to_string() || h == format!("localhost:{rtok_port}"))
        && !tools_rewrite_enabled
        && total_desc_tokens >= threshold
    {
        Some(format!(
            "mcp descriptions ~{} tokens every turn; [proxy.tools_rewrite] enabled = true shortens them (T59.5)",
            total_desc_tokens
        ))
    } else {
        None
    }
}

/// Every probe runs here: settings and host files are read, MCP servers are spawned and
/// asked for their tools, proxy hops answer `/health` or do not.
pub fn page(cfg: &Config) -> Result<Report> {
    let settings = read_json(&cfg.doctor.settings_path);
    let mut hooks = count_hooks(settings.as_ref());
    // A plugin install carries its hooks outside `settings.json` (D21 strips the
    // file entries while the plugin is installed): count them too, so the header
    // agrees with the agents block below (T173).
    let carried = plugin_hooks(cfg);
    hooks.total += carried.total;
    for (event, n) in carried.by_event {
        *hooks.by_event.entry(event).or_insert(0) += n;
    }
    let claude = read_json(&cfg.doctor.claude_json);
    let mut servers = mcp_servers(claude.as_ref(), Path::new(&cfg.doctor.mcp_json));
    // rtok's own MCP surface, so the P6/P8 gates compare like with like.
    // Not under test: current_exe would be the test binary.
    if !cfg!(test)
        && let Ok(exe) = std::env::current_exe()
    {
        servers.push(Server {
            name: "rtok".into(),
            cmd: exe.display().to_string(),
            args: vec!["mcp".into()],
            env: BTreeMap::new(),
        });
    }
    let mcp_timeout = Duration::from_millis(cfg.doctor.mcp_timeout_ms.max(500));
    let mcp: Vec<ServerInfo> = servers
        .iter()
        .map(|s| {
            let (tools, desc_tokens) = list_tools(s, mcp_timeout, &cfg.estimator);
            ServerInfo {
                name: s.name.clone(),
                cmd: s.cmd.clone(),
                tools,
                desc_tokens,
            }
        })
        .collect();
    let timeout = Duration::from_millis(cfg.doctor.probe_timeout_ms.max(300));
    let anthropic = anthropic_base(settings.as_ref(), std::env::var("ANTHROPIC_BASE_URL").ok());
    let total_desc_tokens: u32 = mcp.iter().map(|s| s.desc_tokens).sum();
    let proxy_str = proxy_chain(anthropic.clone(), timeout);
    let tools_rewrite_adv = tools_rewrite_advice(
        anthropic.is_some(),
        &proxy_str,
        cfg.proxy.port,
        total_desc_tokens,
        cfg.proxy.tools_rewrite.enabled,
        cfg.doctor.tools_rewrite_min_desc_tokens,
    );
    Ok(Report {
        hooks_total: hooks.total,
        hooks_by_event: hooks.by_event,
        mcp,
        mcp_tool_search_disabled: anthropic.is_some(),
        proxy: proxy_chain(anthropic, timeout),
        proxy_openai: proxy_chain(openai_seed(cfg, settings.as_ref()), timeout),
        bash_max_output_length: std::env::var("BASH_MAX_OUTPUT_LENGTH").ok(),
        auto_compact_window: settings
            .as_ref()
            .and_then(|s| s.get("autoCompactWindow"))
            .map(|v| v.to_string()),
        read_share: read_share(cfg),
        instructions: cfg
            .doctor
            .instructions
            .then(|| instruction_audit(cfg, settings.as_ref(), claude.as_ref())),
        skills: skills_audit(cfg),
        overlaps: {
            let mut lines = overlap_lines(
                cfg.plugin_enabled("archive", true),
                cfg.plugin_enabled("memory", true) && cfg.plugins.memory.recall_tokens > 0,
                sync_block_present(),
                &detected_hosts(settings.as_ref()),
            );
            let desktop = crate::agents::claude::desktop_path();
            lines.extend(mcp_duplicate_lines(
                crate::agents::claude::plugin_installed(cfg),
                &[
                    (cfg.doctor.claude_json.as_path(), claude.as_ref()),
                    (desktop.as_path(), read_json(&desktop).as_ref()),
                ],
            ));
            lines
        },
        tools_rewrite_advice: tools_rewrite_adv,
        // File reads only: no `--version` probe, so the 2 s dashboard tick stays cheap.
        agents: crate::agents::HOSTS
            .iter()
            .filter_map(|id| crate::agents::host(id))
            .flat_map(|agent| {
                agent.variants().iter().map(move |v| AgentModules {
                    host: agent.id(),
                    kind: v.kind.as_str(),
                    modules: crate::agents::module_rows(agent, v.kind, cfg),
                })
            })
            .collect(),
    })
}

/// Which hosts this machine runs (T59.7): Claude Code when the doctor's settings
/// probe found the file, OpenCode and Cursor by their config dirs under $HOME.
fn detected_hosts(settings: Option<&Value>) -> Vec<&'static str> {
    let mut v = Vec::new();
    if settings.is_some() {
        v.push("claude");
    }
    let home = crate::config::env_user_home();
    let has = |p: &str| home.as_ref().is_some_and(|h| h.join(p).exists());
    if has(".config/opencode") {
        v.push("opencode");
    }
    if has(".cursor") {
        v.push("cursor");
    }
    v
}

fn sync_block_present() -> bool {
    #[cfg(feature = "memory")]
    {
        let cwd = std::env::current_dir().unwrap_or_default();
        ["CLAUDE.md", "AGENTS.md"].iter().any(|f| {
            std::fs::read_to_string(cwd.join(f))
                .ok()
                .is_some_and(|t| crate::plugins::memory::sync::has_block(&t))
        })
    }
    #[cfg(not(feature = "memory"))]
    {
        false
    }
}

/// T59.7: the duplicate checks. Each names the rtok config key that turns the
/// rtok side off; none claims a saving.
fn overlap_lines(
    archive_on: bool,
    memory_recall_on: bool,
    sync_block: bool,
    installed: &[&str],
) -> Vec<String> {
    let mut out = Vec::new();
    if installed.contains(&"claude") && memory_recall_on {
        out.push(
            "duplicate: Claude Code auto-memory (on by default) and memory recall both carry              facts — rtok side: [plugins.memory] enabled = false"
                .into(),
        );
    }
    if sync_block && memory_recall_on {
        #[cfg(feature = "memory")]
        out.push(crate::plugins::memory::sync::overlap_line().into());
        #[cfg(not(feature = "memory"))]
        let _ = sync_block;
    }
    if archive_on && installed.contains(&"opencode") {
        out.push(
            "duplicate: OpenCode marks old tool outputs natively — rtok side:              [plugins.archive] enabled = false"
                .into(),
        );
    }
    if archive_on && installed.contains(&"cursor") {
        out.push(
            "duplicate: Cursor Dynamic Context prunes old context too — rtok side:              [plugins.archive] enabled = false"
                .into(),
        );
    }
    out
}

/// T171: every file that still registers `mcpServers.rtok` while the Claude plugin is
/// installed — Claude Code or the desktop app's Code tab then runs a second rtok server
/// (D21). `rtok agents install claude` strips the file entry under the plugin (T243).
fn mcp_duplicate_lines(plugin: bool, files: &[(&Path, Option<&Value>)]) -> Vec<String> {
    if !plugin {
        return Vec::new();
    }
    files
        .iter()
        .filter(|(_, v)| v.and_then(|v| v.pointer("/mcpServers/rtok")).is_some())
        .map(|(path, _)| {
            format!(
                "duplicate: the Claude plugin and mcpServers.rtok in {} both serve rtok's MCP \
                 — rtok side: rtok agents install claude",
                path.display()
            )
        })
        .collect()
}

/// The skills audit probe (T61.3): the documented roots of every host on this
/// machine (`research.md` §10.1), the enabled plugin skill dirs, and the T61.1
/// invocation counts from the transcripts. Fail open: unreadable roots are skipped.
fn skills_audit(cfg: &Config) -> Option<SkillsAudit> {
    // The user roots hang off $HOME, not rtok's own home: `~/.claude/skills` is the
    // host's dir, `~/.rtok` never holds skills.
    let home = crate::config::env_user_home().unwrap_or_else(|| cfg.home.clone());
    let cwd = std::env::current_dir().ok()?;
    let user = [
        ".claude/skills",
        ".codex/skills",
        ".cursor/skills",
        ".gemini/skills",
        ".copilot/skills",
    ]
    .iter()
    .map(|p| home.join(p).display().to_string())
    .collect();
    let project = [".claude/skills", ".agents/skills"]
        .iter()
        .map(|p| cwd.join(p).display().to_string())
        .collect();
    let roots = vec![
        ("user".to_string(), user),
        ("project".to_string(), project),
        ("plugin".to_string(), plugin_skill_dirs(&home)),
    ];
    let invocations = skill_invocations(cfg);
    Some(audit_from(
        &roots,
        &|p| std::fs::read_to_string(p).ok(),
        &|d| {
            let mut out: Vec<String> = std::fs::read_dir(d)
                .map(|rd| {
                    rd.flatten()
                        .filter(|e| e.path().is_dir())
                        .map(|e| e.path().display().to_string())
                        .collect()
                })
                .unwrap_or_default();
            out.sort();
            out
        },
        &invocations,
    ))
}

/// `<installPath>/skills` of every enabled plugin entry (`installed_plugins.json`).
fn plugin_skill_dirs(home: &Path) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(home.join(".claude/plugins/installed_plugins.json"))
            .unwrap_or_default(),
    ) else {
        return Vec::new();
    };
    let Some(plugins) = v.get("plugins").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in plugins.keys() {
        for p in plugin_install_paths(&v, id) {
            out.push(format!("{p}/skills [{id}]"));
        }
    }
    out
}

/// Walk `roots` — `(source label, dirs whose one-level subdirs may hold a
/// `SKILL.md`)` — through `read`/`subdirs` so tests drive a [`crate::testutil::Vfs`]
/// the way production drives std::fs (D29). Sorted by body bytes, name, byte-stable.
fn audit_from(
    roots: &[(String, Vec<String>)],
    read: &dyn Fn(&str) -> Option<String>,
    subdirs: &dyn Fn(&str) -> Vec<String>,
    invocations: &Option<std::collections::BTreeMap<String, u64>>,
) -> SkillsAudit {
    let mut rows: Vec<SkillRow> = Vec::new();
    for (source, dirs) in roots {
        for dir in dirs {
            // A `[id]` suffix carries the plugin id into the source label.
            let (dir, plugin) = match dir.split_once(" [") {
                Some((d, id)) => (d, Some(format!("plugin:{}", id.trim_end_matches(']')))),
                None => (dir.as_str(), None),
            };
            let source = plugin.as_deref().unwrap_or(source);
            for sub in subdirs(dir) {
                // `file_name`, not `rsplit('/')`: Windows paths end in `\<name>` (T83.7).
                let Some(name) = Path::new(&sub).file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some(md) = read(&format!("{sub}/SKILL.md")) else {
                    continue;
                };
                rows.push(skill_row(name, source, &md, invocations));
            }
        }
    }
    rows.sort_by(|a, b| b.body_bytes.cmp(&a.body_bytes).then(a.name.cmp(&b.name)));
    // The same skill reachable from two roots (`.claude/skills` and
    // `.agents/skills` mirror each other) is one listing, not two.
    let mut seen = std::collections::HashSet::new();
    rows.retain(|r| seen.insert((r.source.clone(), r.name.clone())));
    let desc_bytes = rows.iter().map(|r| r.desc_chars as u64).sum();
    SkillsAudit { rows, desc_bytes }
}

/// One row: frontmatter `description:` length, body bytes, and the flags the
/// measured §10.2 numbers justify.
fn skill_row(
    name: &str,
    source: &str,
    md: &str,
    invocations: &Option<std::collections::BTreeMap<String, u64>>,
) -> SkillRow {
    let desc_chars = frontmatter_desc(md);
    let body = frontmatter_body(md);
    let body_bytes = body.len() as u64;
    let calls = invocations.as_ref().and_then(|m| m.get(name).copied());
    SkillRow {
        name: name.to_string(),
        source: source.to_string(),
        desc_chars,
        body_bytes,
        invocations: calls,
        warn_desc: desc_chars > 200,
        warn_body: body_bytes > 8192,
        warn_never: invocations.is_some() && calls.is_none(),
    }
}

/// The `description:` value's char count from the frontmatter block, if any.
fn frontmatter_desc(md: &str) -> usize {
    let mut lines = md.lines();
    if lines.next().is_none_or(|l| l.trim() != "---") {
        return 0;
    }
    for l in lines {
        let l = l.trim();
        if l == "---" {
            break;
        }
        if let Some(v) = l.strip_prefix("description:") {
            return v.trim().trim_matches('"').chars().count();
        }
    }
    0
}

/// The bytes after the closing `---` fence — the body the T61.3 flags measure.
/// A missing or unclosed fence treats the whole file as the body (fail open).
fn frontmatter_body(md: &str) -> &str {
    let Some(rest) = md.strip_prefix("---\n") else {
        return md;
    };
    match rest.find("\n---\n") {
        Some(i) => &rest[i + "\n---\n".len()..],
        None => md,
    }
}

/// Invocations per skill over the last 30 d, counted the T61.1 way (the `Skill`
/// tool_use's `input.skill`). `None` = no scan ran (tests, unreadable dir) — the
/// `never invoked` flag then stays off (fail open), and never on the hook path.
fn skill_invocations(cfg: &Config) -> Option<std::collections::BTreeMap<String, u64>> {
    if cfg!(test) {
        return None;
    }
    let cutoff =
        std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(30 * 86400))?;
    let mut out = std::collections::BTreeMap::new();
    for (_, agg) in transcripts(cfg, cutoff) {
        for (skill, n) in agg.skills {
            *out.entry(skill).or_insert(0) += n;
        }
    }
    Some(out)
}

/// T135: transcript aggregates through the (path, size, mtime) cache kept beside the store.
fn transcripts(
    cfg: &Config,
    cutoff: std::time::SystemTime,
) -> Vec<(
    std::path::PathBuf,
    crate::measure::transcript_cache::FileAgg,
)> {
    // Tests keep the in-process cache only: a default `Config` points at the real `~/.rtok`.
    let file = cfg.core.db_path.with_file_name("transcripts-cache.json");
    let file = (!cfg!(test)).then_some(file.as_path());
    crate::measure::transcript_cache::scan(&cfg.stats.transcripts_dir, cutoff, file)
}

const INJECTORS: &[&str] = &[
    "lean-ctx",
    "engram",
    "ponytail",
    "claude-mem",
    "token-optimizer",
    "caveman",
    "headroom",
];

struct Source {
    name: String,
    path: String,
    text: String,
}

fn instruction_audit(
    cfg: &Config,
    settings: Option<&Value>,
    claude: Option<&Value>,
) -> Instructions {
    let mut srcs = Vec::new();
    if let Some(dir) = cfg.doctor.settings_path.parent() {
        push_file(&mut srcs, "claude-user", &dir.join("CLAUDE.md"));
    }
    if let Some(root) = git_root() {
        push_file(&mut srcs, "claude-project", &root.join("CLAUDE.md"));
        push_file(&mut srcs, "agents-project", &root.join("AGENTS.md"));
    }
    let blob = format!(
        "{}{}",
        settings.map(Value::to_string).unwrap_or_default(),
        claude.map(Value::to_string).unwrap_or_default()
    );
    for name in INJECTORS {
        if blob.contains(name) {
            let path = find_skill(name).unwrap_or_else(|| format!("mcp:{name}"));
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            srcs.push(Source {
                name: (*name).into(),
                path,
                text,
            });
        }
    }
    let warn_at = cfg.doctor.instruction_warn_tokens;
    Instructions {
        rows: srcs
            .iter()
            .map(|s| {
                let tokens = tokens::estimate(&s.text, Class::Prose, &cfg.estimator);
                InstructionRow {
                    name: s.name.clone(),
                    warn: tokens > warn_at,
                    tokens,
                    path: s.path.clone(),
                }
            })
            .collect(),
        duplicates: duplicates(&srcs),
    }
}

fn push_file(srcs: &mut Vec<Source>, name: &str, path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    // By the file itself, not its name: with `CLAUDE.md -> AGENTS.md` one file was read twice,
    // every line came back as a duplicate of itself, and its tokens were counted twice.
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let me = real(path);
    if srcs.iter().any(|s| real(Path::new(&s.path)) == me) {
        return;
    }
    let disp = path.display().to_string();
    srcs.push(Source {
        name: name.into(),
        path: disp,
        text,
    });
}

fn git_root() -> Option<std::path::PathBuf> {
    let mut p = std::env::current_dir().ok()?;
    loop {
        if p.join(".git").exists() {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

fn find_skill(name: &str) -> Option<String> {
    // Same empty-HOME → USERPROFILE rule as config expand / agent setup.
    let home = crate::config::env_user_home()?;
    let p = skill_md_path(&home, name);
    p.is_file().then(|| p.display().to_string())
}

/// `~/.claude/skills/<name>/SKILL.md`, joined by components so Windows never
/// sees a single path segment with embedded slashes.
pub(crate) fn skill_md_path(home: &Path, name: &str) -> std::path::PathBuf {
    home.join(".claude")
        .join("skills")
        .join(name)
        .join("SKILL.md")
}

fn duplicates(srcs: &[Source]) -> Vec<(String, Vec<String>)> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in srcs {
        for line in s.text.lines() {
            let t = line.trim();
            if t.len() < 40 {
                continue;
            }
            map.entry(t.to_string()).or_default().push(s.name.clone());
        }
    }
    map.into_iter()
        .filter_map(|(sent, mut names)| {
            names.sort();
            names.dedup();
            (names.len() > 1).then_some((sent.chars().take(60).collect(), names))
        })
        .collect()
}

struct HookCount {
    total: usize,
    by_event: BTreeMap<String, usize>,
}

fn count_hooks(settings: Option<&Value>) -> HookCount {
    let mut c = HookCount {
        total: 0,
        by_event: BTreeMap::new(),
    };
    if let Some(hooks) = settings
        .and_then(|s| s.get("hooks"))
        .and_then(Value::as_object)
    {
        add_hooks_object(&mut c, hooks);
    }
    c
}

/// Add every command under a `hooks` object — `settings.json` or an installed
/// plugin's `hooks/hooks.json`, same shape — into the running count.
fn add_hooks_object(c: &mut HookCount, hooks: &serde_json::Map<String, Value>) {
    for (event, entries) in hooks {
        let n = match entries {
            Value::Array(a) => a.iter().map(inner_hook_count).sum(),
            _ => 0,
        };
        c.total += n;
        *c.by_event.entry(event.clone()).or_insert(0) += n;
    }
}

/// Hooks carried by the installed Claude plugin (T173). While the plugin is
/// installed, setup strips the file entries (D21), so `settings.json` alone
/// reads 0 while the agents block reports hooks installed. Same install check
/// the agents block uses through `Claude::installed` (`agents::claude::
/// plugin_installed`) and the same `installed_plugins.json` location
/// (`agents::claude::config_dir`) — so the two never drift. The install paths
/// come from `plugin_install_paths`, the parser `plugin_skill_dirs` already
/// runs for skills. Fail open: an unreadable plugin tree counts 0.
fn plugin_hooks(cfg: &Config) -> HookCount {
    let mut c = HookCount {
        total: 0,
        by_event: BTreeMap::new(),
    };
    if !crate::agents::claude::plugin_installed(cfg) {
        return c;
    }
    let path = crate::agents::claude::config_dir(cfg).join("plugins/installed_plugins.json");
    let Ok(root) =
        serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap_or_default())
    else {
        return c;
    };
    for p in plugin_install_paths(&root, crate::agents::claude::PLUGIN_ID) {
        let file = read_json(&Path::new(&p).join("hooks/hooks.json"));
        if let Some(obj) = file
            .as_ref()
            .and_then(|v| v.get("hooks"))
            .and_then(Value::as_object)
        {
            add_hooks_object(&mut c, obj);
        }
    }
    c
}

/// Every `installPath` an `installed_plugins.json`'s `plugins` map lists for `id`
/// (`{"plugins": {"<id>": [{"installPath": …}, …]}}`, the V2 shape both `plugin_hooks`
/// and `plugin_skill_dirs` need — one parser, not two).
fn plugin_install_paths(root: &Value, id: &str) -> Vec<String> {
    root.get("plugins")
        .and_then(|p| p.get(id))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("installPath").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn inner_hook_count(entry: &Value) -> usize {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(1)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// One `mcpServers` entry. `args`/`env` matter: spawning the bare `command` made every
/// `uvx`/`npx`-launched server (serena, mobile, engram) report 0 tools.
struct Server {
    name: String,
    cmd: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

fn mcp_servers(claude: Option<&Value>, mcp_json: &Path) -> Vec<Server> {
    let mut out: Vec<Server> = Vec::new();
    for src in [claude, read_json(mcp_json).as_ref()] {
        let Some(map) = src
            .and_then(|v| v.get("mcpServers"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (name, spec) in map {
            if out.iter().any(|s| &s.name == name) {
                continue;
            }
            let args = spec
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            let env = spec
                .get("env")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect();
            out.push(Server {
                name: name.clone(),
                cmd: spec
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                args,
                env,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Start one MCP server for probing.
///
/// On Windows, bare names like `npx` / `uvx` (and explicit `.cmd` / `.bat`
/// paths) must go through `cmd.exe /D /C`: CreateProcess will not run those
/// shims, so doctor used to report 0 tools for every npx-launched server.
fn spawn_mcp(s: &Server) -> std::io::Result<std::process::Child> {
    let mut cmd = mcp_command(&s.cmd, &s.args);
    cmd.envs(&s.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
}

fn mcp_command(command: &str, args: &[String]) -> Command {
    if cfg!(windows) && windows_needs_cmd_host(command) {
        let mut c = Command::new("cmd.exe");
        c.arg("/D").arg("/C").arg(command).args(args);
        c
    } else {
        let mut c = Command::new(command);
        c.args(args);
        c
    }
}

/// True when `command` is a Windows batch/PATHEXT shim CreateProcess cannot run.
fn windows_needs_cmd_host(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    if lower.ends_with(".exe") || lower.ends_with(".com") {
        return false;
    }
    if lower.ends_with(".cmd") || lower.ends_with(".bat") {
        return true;
    }
    // Bare name with no path: `npx`, `uvx`, `rtok` — PATHEXT may resolve a .cmd.
    !command.contains('/') && !command.contains('\\') && !command.contains(':')
}

fn list_tools(s: &Server, timeout: Duration, est: &crate::config::Estimator) -> (usize, u32) {
    if s.cmd.is_empty() {
        return (0, 0);
    }
    let payload = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"rtok","version":"0.1.0"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        "\n"
    );
    let mut child = match spawn_mcp(s) {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    // Keep stdin open until we have the answer: some servers exit on EOF before replying,
    // and slow starters (uvx, npx) need the full timeout rather than a wait-for-exit.
    let mut stdin = child.stdin.take();
    if let Some(si) = stdin.as_mut() {
        let _ = si.write_all(payload.as_bytes());
    }
    let (tx, rx) = mpsc::channel::<Value>();
    let stdout = child.stdout.take();
    std::thread::spawn(move || {
        let Some(so) = stdout else { return };
        for line in BufReader::new(so).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && let Some(tools) = v.pointer("/result/tools")
            {
                let _ = tx.send(tools.clone());
                return;
            }
        }
    });
    let tools = rx.recv_timeout(timeout).ok();
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let Some(tools) = tools.as_ref().and_then(Value::as_array) else {
        return (0, 0);
    };
    let tokens: u32 = tools
        .iter()
        .map(|t| {
            let d = t.get("description").and_then(Value::as_str).unwrap_or("");
            tokens::estimate(d, Class::Prose, est)
        })
        .sum();
    (tools.len(), tokens)
}

fn nonempty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.is_empty())
}

/// T50.4: `(grep + glob) / (read + grep + glob)` over transcript result bytes
/// (`stats::collect`, same 4 chars/token). `None` on any error, on files older
/// than `[stats] since`, or when no Read-class tool ran — doctor stays fail-open
/// and the deny stays off on no data.
fn read_share(cfg: &Config) -> Option<ReadShare> {
    let since = crate::measure::stats::parse_since(&cfg.stats.since).ok()?;
    let cutoff = std::time::SystemTime::now()
        .checked_sub(since)
        .unwrap_or(std::time::UNIX_EPOCH);
    // `stats::collect` totals, minus its sub-agent transcripts (T128), cached per file.
    let files: Vec<_> = transcripts(cfg, cutoff)
        .into_iter()
        .filter(|(p, _)| !crate::measure::subagents::is_subagent(p))
        .collect();
    let tok = |name: &str| -> u64 {
        files
            .iter()
            .filter_map(|(_, a)| a.tool_tokens.get(name))
            .sum()
    };
    let (read, grep, glob) = (tok("Read"), tok("Grep"), tok("Glob"));
    let denom = read + grep + glob;
    if denom == 0 {
        return None;
    }
    Some(ReadShare {
        read_tokens: read,
        grep_tokens: grep,
        glob_tokens: glob,
        share: (grep + glob) as f64 / denom as f64,
    })
}

/// The `ANTHROPIC_BASE_URL` a Claude Code session sees: `settings.json` `env` wins over the
/// shell, and empty means unset. The proxy chain and the tool-search warning read it in
/// opposite orders, so `env` `""` plus a settings URL showed a chain and no warning.
/// A URL equal to the default Anthropic endpoint (trailing slash tolerated) is not
/// custom — Claude Desktop sets it explicitly — so it reads as unset (T173).
fn anthropic_base(settings: Option<&Value>, env: Option<String>) -> Option<String> {
    let base = nonempty(
        settings
            .and_then(|s| s.pointer("/env/ANTHROPIC_BASE_URL"))
            .and_then(Value::as_str)
            .map(str::to_string),
    )
    .or_else(|| nonempty(env))?;
    (!is_default_anthropic_base(&base)).then_some(base)
}

/// True for `https://api.anthropic.com` with any trailing slashes (T173).
fn is_default_anthropic_base(url: &str) -> bool {
    url.trim().trim_end_matches('/') == "https://api.anthropic.com"
}

fn openai_seed(cfg: &Config, settings: Option<&Value>) -> Option<String> {
    nonempty(std::env::var("OPENAI_BASE_URL").ok())
        .or_else(|| {
            settings
                .and_then(|s| s.pointer("/env/OPENAI_BASE_URL"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            read_json(&cfg.setup.opencode.config_path)
                .as_ref()
                .and_then(|s| s.pointer("/env/OPENAI_BASE_URL"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            let doc: toml_edit::DocumentMut = std::fs::read_to_string(&cfg.setup.codex.config_path)
                .ok()?
                .parse()
                .ok()?;
            doc.get("model_providers")?
                .get("rtok")?
                .get("base_url")?
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

fn proxy_chain(seed: Option<String>, timeout: Duration) -> String {
    let mut hops = Vec::new();
    let mut url = nonempty(seed);
    let mut seen = 0;
    while let Some(u) = url.take() {
        if seen > 4 {
            break;
        }
        seen += 1;
        hops.push(hostport(&u));
        url = next_upstream(&u, timeout).filter(|n| !is_loop(&hops, n));
    }
    hops.join("→")
}

/// Whether `next` is a hop already walked. By `hostport`, not substring: hop `8788` is inside
/// `http://127.0.0.1:18788`, and `contains` cut a real chain short as a loop.
fn is_loop(hops: &[String], next: &str) -> bool {
    hops.contains(&hostport(next))
}

fn hostport(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split('/').next().unwrap_or(rest);
    host.trim_start_matches("127.0.0.1:").to_string()
}

fn next_upstream(url: &str, timeout: Duration) -> Option<String> {
    let body = http_get(url, "/health", timeout)?;
    let v: Value = serde_json::from_str(&body).ok()?;
    v.pointer("/checks/upstream/url")
        .or(v.pointer("/config/anthropic_api_url"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Body of a plain HTTP/1.1 GET to a local rtok endpoint (`/health`, `/live`); `None` on any
/// failure. Shared with the operator model's `/live` fetch, so there is one hand-rolled client.
pub(crate) fn http_get(base: &str, path: &str, timeout: Duration) -> Option<String> {
    let rest = base.split("://").nth(1).unwrap_or(base);
    let hostport = rest.split('/').next().unwrap_or(rest);
    let addr = hostport.to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {hostport}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    // `split_once`: a body with a blank line of its own is still the whole body.
    text.split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
}

/// A bare [`Report`] for the render tests — the `report` renderer fixtures (T105) build
/// their Doctor section on it, so a fixed model's doctor text is deterministic.
#[cfg(test)]
pub(crate) fn report_fixture() -> Report {
    Report {
        hooks_total: 0,
        hooks_by_event: BTreeMap::new(),
        mcp: Vec::new(),
        proxy: String::new(),
        proxy_openai: String::new(),
        mcp_tool_search_disabled: false,
        bash_max_output_length: None,
        auto_compact_window: None,
        read_share: None,
        instructions: None,
        skills: None,
        overlaps: Vec::new(),
        tools_rewrite_advice: None,
        agents: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// T61.3: the audit walks a Vfs tree of three skills — one over 200 desc chars,
    /// one over an 8 KB body, one never invoked — flags each, sorts by body bytes
    /// and totals the description bytes (D29: no host disk for path/content tests).
    #[test]
    fn skills_audit_flags_the_measured_warns_on_a_vfs_tree() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write(
            "home/.claude/skills/tiny/SKILL.md",
            "---\ndescription: small\n---\nbody\n",
        );
        vfs.write(
            "home/.claude/skills/wordy/SKILL.md",
            format!("---\ndescription: {}\n---\nbody\n", "w".repeat(201)),
        );
        vfs.write(
            "home/.claude/skills/big/SKILL.md",
            format!("---\ndescription: fine\n---\n{}\n", "b".repeat(9000)),
        );
        vfs.write(
            "home/.claude/plugins/cache/x/y/1.0/skills/plug/SKILL.md",
            "---\ndescription: from a plugin\n---\nbody\n",
        );
        let read = |p: &str| vfs.read_str(p).map(str::to_string);
        let subdirs = |d: &str| {
            vfs.paths_under(d)
                .iter()
                .filter_map(|p| p.strip_suffix("/SKILL.md").map(|s| s.to_string()))
                .collect::<Vec<_>>()
        };
        let invocations = Some(std::collections::BTreeMap::from([(
            "tiny".to_string(),
            8u64,
        )]));
        let roots = vec![
            ("user".to_string(), vec!["home/.claude/skills".to_string()]),
            (
                "plugin".to_string(),
                vec!["home/.claude/plugins/cache/x/y/1.0/skills [x@market]".to_string()],
            ),
        ];
        let audit = audit_from(&roots, &read, &subdirs, &invocations);
        assert_eq!(
            audit
                .rows
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["big", "plug", "tiny", "wordy"],
            "body bytes desc, name asc"
        );
        let wordy = audit.rows.iter().find(|r| r.name == "wordy").unwrap();
        assert!(wordy.warn_desc, "{wordy:?}");
        assert!(wordy.warn_never, "invocation data exists, count does not");
        let big = audit.rows.iter().find(|r| r.name == "big").unwrap();
        assert!(big.warn_body, "{big:?}");
        let tiny = audit.rows.iter().find(|r| r.name == "tiny").unwrap();
        assert_eq!(tiny.invocations, Some(8));
        assert!(
            !tiny.warn_never && !tiny.warn_body && !tiny.warn_desc,
            "{tiny:?}"
        );
        let plug = audit.rows.iter().find(|r| r.name == "plug").unwrap();
        assert_eq!(plug.source, "plugin:x@market", "{plug:?}");
        assert_eq!(
            audit.desc_bytes, 223,
            "small=5, wordy=201, fine=4, `from a plugin`=13"
        );
        // No invocation data → the never-invoked flag stays off (fail open).
        let none = audit_from(&roots, &read, &subdirs, &None);
        assert!(none.rows.iter().all(|r| !r.warn_never));
    }

    /// T71.3: the hub skill is a user skill like any other — same roots, same row shape.
    #[test]
    fn skills_audit_lists_the_rtok_hub_skill_like_any_other() {
        let hub = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/rtok/SKILL.md"));
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("home/.claude/skills/rtok/SKILL.md", hub);
        vfs.write(
            "home/.claude/skills/other/SKILL.md",
            "---\ndescription: other\n---\n# o\n",
        );
        let read = |p: &str| vfs.read_str(p).map(str::to_string);
        let subdirs = |d: &str| {
            vfs.paths_under(d)
                .iter()
                .filter_map(|p| p.strip_suffix("/SKILL.md").map(|s| s.to_string()))
                .collect::<Vec<_>>()
        };
        let roots = vec![("user".to_string(), vec!["home/.claude/skills".to_string()])];
        let audit = audit_from(&roots, &read, &subdirs, &None);
        let rtok = audit.rows.iter().find(|r| r.name == "rtok").unwrap();
        assert!(
            audit.rows.iter().any(|r| r.name == "other"),
            "foreign skills stay in the listing"
        );
        assert_eq!(rtok.source, "user");
        assert_eq!(rtok.desc_chars, 108);
        assert!(!rtok.warn_desc && !rtok.warn_body, "{rtok:?}");
    }

    /// T59.7: the three duplicate checks fire only when both sides are on, name
    /// the rtok config key, and never claim a saving.
    #[test]
    fn overlap_checks_name_the_rtok_off_key_and_stay_off_when_quiet() {
        let lines = overlap_lines(true, true, false, &["claude", "opencode", "cursor"]);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(
            lines[0].contains("[plugins.memory] enabled = false"),
            "{lines:?}"
        );
        assert!(
            lines[1].contains("[plugins.archive] enabled = false"),
            "{lines:?}"
        );
        assert!(
            lines[2].contains("[plugins.archive] enabled = false"),
            "{lines:?}"
        );
        for l in &lines {
            assert!(l.starts_with("duplicate:"), "{l}");
            assert!(!l.contains("saves"), "no measurement claim: {l}");
        }
        // Either side off, or the host absent, is no line.
        assert!(overlap_lines(false, true, false, &["opencode"]).is_empty());
        assert!(overlap_lines(true, false, true, &["claude"]).is_empty());
        assert!(overlap_lines(true, true, false, &[]).is_empty());
        // The report renders them under `overlaps`.
        let mut r = report_fixture();
        r.overlaps = overlap_lines(true, true, false, &["opencode"]);
        let text = r.to_text();
        assert!(text.contains("overlaps\n  duplicate: OpenCode"), "{text}");
        let sync = overlap_lines(false, true, true, &[]);
        assert_eq!(sync.len(), 1, "{sync:?}");
        assert!(
            sync[0].contains("[plugins.memory] enabled = false"),
            "{sync:?}"
        );
        assert!(sync[0].starts_with("duplicate:"), "{sync:?}");
        assert!(!sync[0].contains("saves"), "{sync:?}");
    }

    /// T171: an `mcpServers.rtok` next to the installed Claude plugin is one `duplicate:`
    /// line per file; without the plugin, or without the entry, there is none.
    #[test]
    fn mcp_entry_next_to_the_claude_plugin_is_a_duplicate() {
        let (code, desktop) = (Path::new("/h/.claude.json"), Path::new("/h/desktop.json"));
        let ours = json!({"mcpServers": {"rtok": {"command": "rtok", "args": ["mcp"]}}});
        let other = json!({"mcpServers": {"serena": {"command": "uvx"}}});
        let lines = mcp_duplicate_lines(true, &[(code, Some(&other)), (desktop, Some(&ours))]);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("duplicate:"), "{lines:?}");
        assert!(lines[0].contains("/h/desktop.json"), "{lines:?}");
        assert!(lines[0].contains("rtok agents install claude"), "{lines:?}");
        assert!(!lines[0].contains("saves"), "{lines:?}");
        let both = mcp_duplicate_lines(true, &[(code, Some(&ours)), (desktop, Some(&ours))]);
        assert_eq!(both.len(), 2, "{both:?}");
        assert!(mcp_duplicate_lines(false, &[(code, Some(&ours))]).is_empty());
        assert!(mcp_duplicate_lines(true, &[(code, None), (desktop, Some(&other))]).is_empty());
    }

    /// The doctor text carries the section with the header total and per-row flags.
    #[test]
    fn skills_audit_renders_a_section_with_flags() {
        let audit = SkillsAudit {
            desc_bytes: 205,
            rows: vec![SkillRow {
                name: "update-config".into(),
                source: "user".into(),
                desc_chars: 205,
                body_bytes: 248_175,
                invocations: Some(4),
                warn_desc: true,
                warn_body: true,
                warn_never: false,
            }],
        };
        let mut r = report_fixture();
        r.skills = Some(audit);
        let text = r.to_text();
        assert!(
            text.contains("skills (1 listed, 205 desc bytes ≈ 51 tokens per request)"),
            "{text}"
        );
        assert!(
            text.contains("update-config user desc 205c body 248175B calls 4"),
            "{text}"
        );
        assert!(text.contains("WARN desc>200"), "{text}");
        assert!(text.contains("WARN body>8K (references/)"), "{text}");
    }

    #[test]
    fn counts_nested_hook_commands() {
        let s = json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command"},{"type":"command"}]},{"hooks":[{"type":"command"}]}]}});
        let c = count_hooks(Some(&s));
        assert_eq!(c.total, 3);
        assert_eq!(c.by_event["PreToolUse"], 3);
    }

    #[test]
    fn mcp_servers_keep_args_and_env() {
        let c = json!({"mcpServers":{
            "serena":{"command":"/usr/bin/uvx","args":["--from","serena-agent","serena"],"env":{"A":"1"}},
            "bare":{"command":"x"}}});
        let s = mcp_servers(Some(&c), Path::new("/nonexistent/.mcp.json"));
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "bare");
        assert!(s[0].args.is_empty());
        assert_eq!(s[1].args, ["--from", "serena-agent", "serena"]);
        assert_eq!(s[1].env["A"], "1");
    }

    #[test]
    fn skill_md_path_joins_components() {
        let p = skill_md_path(Path::new(r"C:\Users\Example"), "lean-ctx");
        assert_eq!(
            p,
            Path::new(r"C:\Users\Example")
                .join(".claude")
                .join("skills")
                .join("lean-ctx")
                .join("SKILL.md")
        );
    }

    #[test]
    fn windows_cmd_host_wraps_npx_shims_not_exes() {
        assert!(windows_needs_cmd_host("npx"));
        assert!(windows_needs_cmd_host("uvx"));
        assert!(windows_needs_cmd_host(r"C:\Program Files\nodejs\npx.cmd"));
        assert!(windows_needs_cmd_host("tool.bat"));
        assert!(!windows_needs_cmd_host("rtok.exe"));
        assert!(!windows_needs_cmd_host(r"C:\Users\u\.ketch\bin\rtok.exe"));
        assert!(!windows_needs_cmd_host("/usr/bin/uvx"));
    }

    #[test]
    fn hostport_strips_loopback() {
        assert_eq!(hostport("http://127.0.0.1:8788"), "8788");
        assert_eq!(hostport("http://127.0.0.1:8787/w/claude"), "8787");
    }

    /// One resolution for both readers: settings over shell, empty is unset.
    #[test]
    fn anthropic_base_prefers_settings_and_ignores_empty() {
        let s = serde_json::json!({"env": {"ANTHROPIC_BASE_URL": "http://a"}});
        let empty = serde_json::json!({"env": {"ANTHROPIC_BASE_URL": ""}});
        let b = |v: Option<&Value>, e: &str| anthropic_base(v, Some(e.to_string()));
        assert_eq!(b(Some(&s), "").as_deref(), Some("http://a"));
        assert_eq!(b(Some(&s), "http://b").as_deref(), Some("http://a"));
        assert_eq!(b(Some(&empty), "http://b").as_deref(), Some("http://b"));
        assert_eq!(b(None, ""), None);
    }

    /// T173: Claude Desktop sets `ANTHROPIC_BASE_URL` to the default endpoint
    /// explicitly — that is not a custom base, so no tool-search warning.
    #[test]
    fn anthropic_base_ignores_the_default_endpoint() {
        let plain = serde_json::json!({"env": {"ANTHROPIC_BASE_URL": "https://api.anthropic.com"}});
        let slash =
            serde_json::json!({"env": {"ANTHROPIC_BASE_URL": "https://api.anthropic.com/"}});
        let custom = serde_json::json!({"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:8790"}});
        assert_eq!(anthropic_base(Some(&plain), None), None);
        assert_eq!(anthropic_base(Some(&slash), None), None);
        assert_eq!(
            anthropic_base(None, Some("https://api.anthropic.com".into())),
            None
        );
        assert_eq!(
            anthropic_base(Some(&custom), None).as_deref(),
            Some("http://127.0.0.1:8790")
        );
        // A custom env URL still loses to an explicit default in settings.
        assert_eq!(anthropic_base(Some(&plain), Some("http://a".into())), None);
    }

    /// T173: a plugin-only home — empty `settings.json` hooks, plugin installed —
    /// counts the plugin-carried hooks, agreeing with the agents block.
    #[test]
    fn page_counts_plugin_carried_hooks_on_a_plugin_only_home() {
        let dir = std::env::temp_dir().join(format!("rtok-t173-plugin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("cache/rtok/1.0");
        std::fs::create_dir_all(install.join("hooks")).unwrap();
        std::fs::create_dir_all(dir.join("plugins")).unwrap();
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        // `serde_json::json!` (not a hand-formatted string) so a Windows install path's
        // backslashes are JSON-escaped rather than landing raw in the file and failing
        // to parse.
        std::fs::write(
            dir.join("plugins/installed_plugins.json"),
            serde_json::json!({
                "version": 2,
                "plugins": {"rtok@rtok": [{"installPath": install.to_string_lossy()}]}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            install.join("hooks/hooks.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command"}]}]}}"#,
        )
        .unwrap();
        let mut cfg = crate::testutil::config_in(&dir);
        cfg.doctor.settings_path = dir.join("settings.json");
        cfg.setup.claude.settings_path = dir.join("settings.json");
        let report = page(&cfg).unwrap();
        assert_eq!(report.hooks_total, 1, "{:?}", report.hooks_by_event);
        assert_eq!(report.hooks_by_event["PreToolUse"], 1);
        let text = report.to_text();
        assert!(text.starts_with("hooks 1\n"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T173: the default endpoint in `settings.json` raises no tool-search warning.
    #[test]
    fn page_warns_no_tool_search_on_the_default_endpoint() {
        let dir = std::env::temp_dir().join(format!("rtok-t173-base-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.anthropic.com"}}"#,
        )
        .unwrap();
        let mut cfg = crate::testutil::config_in(&dir);
        cfg.doctor.settings_path = dir.join("settings.json");
        cfg.setup.claude.settings_path = dir.join("settings.json");
        let report = page(&cfg).unwrap();
        assert!(!report.mcp_tool_search_disabled);
        assert!(!report.to_text().contains("mcp_tool_search"), "{report:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_port_that_contains_a_seen_port_is_not_a_loop() {
        let hops = vec!["8788".to_string()];
        assert!(!is_loop(&hops, "http://127.0.0.1:18788"));
        assert!(is_loop(&hops, "http://127.0.0.1:8788/v1"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_instruction_file_is_one_source() {
        let dir = std::env::temp_dir().join(format!("rtok-doctor-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "one rule\n").unwrap();
        std::os::unix::fs::symlink("AGENTS.md", dir.join("CLAUDE.md")).unwrap();
        let mut srcs = Vec::new();
        push_file(&mut srcs, "claude-project", &dir.join("CLAUDE.md"));
        push_file(&mut srcs, "agents-project", &dir.join("AGENTS.md"));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(srcs.len(), 1);
        assert!(duplicates(&srcs).is_empty());
    }

    #[test]
    fn instructions_lists_four_injectors() {
        let dir = std::env::temp_dir().join("rtok-t72-instructions");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("CLAUDE.md"),
            "user claude md padding for a long enough line xx\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("claude.json"),
            r#"{"mcpServers":{"lean-ctx":{"command":"/bin/true"},"engram":{"command":"/bin/true"},"ponytail":{"command":"/bin/true"},"claude-mem":{"command":"/bin/true"}}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let mut cfg = crate::testutil::config_in(&dir);
        cfg.doctor.settings_path = dir.join("settings.json");
        cfg.doctor.claude_json = dir.join("claude.json");
        cfg.doctor.instructions = true;
        let s = page(&cfg).unwrap().to_text();
        assert!(s.contains("instructions"), "{s}");
        let n = s
            .lines()
            .filter(|l| l.starts_with("  ") && l.contains("tokens"))
            .count();
        assert!(n >= 4, "{s}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T50.4: one session with Read (40 B), Grep (20 B) and Glob (12 B) results
    /// reports grep+glob 8/18 = 44.4% of read-class tokens; an empty transcripts
    /// dir reports `no data` instead of 0%.
    #[test]
    fn read_share_reports_grep_glob_fraction() {
        let dir = std::env::temp_dir().join(format!("rtok-t504-doc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pair = |mid: &str, uid: &str, name: &str, rid: &str, body: &str| {
            format!(
                "{{\"type\":\"assistant\",\"message\":{{\"id\":\"{mid}\",\"content\":[{{\"type\":\"tool_use\",\"id\":\"{uid}\",\"name\":\"{name}\",\"input\":{{}}}}]}}}}\n\
                 {{\"type\":\"user\",\"message\":{{\"id\":\"{rid}\",\"content\":[{{\"type\":\"tool_result\",\"tool_use_id\":\"{uid}\",\"content\":\"{body}\"}}]}}}}\n"
            )
        };
        std::fs::write(
            dir.join("s.jsonl"),
            pair("a1", "u-read", "Read", "r1", &"R".repeat(40))
                + &pair("a2", "u-grep", "Grep", "r2", &"G".repeat(20))
                + &pair("a3", "u-glob", "Glob", "r3", &"g".repeat(12)),
        )
        .unwrap();
        let mut cfg = crate::testutil::config_in(&dir);
        cfg.stats.transcripts_dir = dir.clone();
        let s = page(&cfg).unwrap().to_text();
        assert!(
            s.contains("read-share grep+glob 44.4% of read-class tokens (read 10, grep 5, glob 3)"),
            "{s}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_share_without_transcripts_is_no_data() {
        let dir = std::env::temp_dir().join(format!("rtok-t504-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = crate::testutil::config_in(&dir);
        cfg.stats.transcripts_dir = dir.clone();
        let s = page(&cfg).unwrap().to_text();
        assert!(s.contains("read-share no data"), "{s}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lists_anthropic_and_openai_proxy_chains() {
        let dir = std::env::temp_dir().join(format!("rtok-t115-doctor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:8790"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("opencode.json"),
            r#"{"env":{"OPENAI_BASE_URL":"http://127.0.0.1:8790/v1"}}"#,
        )
        .unwrap();
        let mut cfg = crate::testutil::config_in(&dir);
        cfg.doctor.settings_path = dir.join("settings.json");
        cfg.setup.opencode.config_path = dir.join("opencode.json");
        let s = page(&cfg).unwrap().to_text();
        assert!(
            s.lines().any(|l| l.starts_with("proxy ")
                && !l.starts_with("proxy openai")
                && l.contains("8790")),
            "{s}"
        );
        assert!(
            s.lines()
                .any(|l| l.starts_with("proxy openai ") && l.contains("8790")),
            "{s}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tools_rewrite_advice_all_conditions_met() {
        // All four conditions: tool search disabled, rtok in proxy, tools_rewrite disabled, high desc_tokens
        let advice = tools_rewrite_advice(true, "8790→api.anthropic.com", 8790, 2500, false, 2000);
        let advice = advice.expect("all four hold");
        assert!(
            advice.contains("2500") && advice.contains("T59.5"),
            "{advice}"
        );
        let local = tools_rewrite_advice(true, "localhost:8790→x:443", 8790, 2500, false, 2000);
        assert!(local.is_some());
    }

    #[test]
    fn tools_rewrite_advice_no_tool_search_disabled() {
        // No tool search disabled — advice should not appear
        let advice = tools_rewrite_advice(false, "8790→api.anthropic.com", 8790, 2500, false, 2000);
        assert!(advice.is_none());
    }

    #[test]
    fn tools_rewrite_advice_rtok_not_in_proxy() {
        // rtok not in proxy chain — advice should not appear
        let advice = tools_rewrite_advice(true, "8788→api.anthropic.com", 8790, 2500, false, 2000);
        assert!(advice.is_none());
    }

    #[test]
    fn tools_rewrite_advice_already_enabled() {
        // tools_rewrite already enabled — advice should not appear
        let advice = tools_rewrite_advice(true, "8790→api.anthropic.com", 8790, 2500, true, 2000);
        assert!(advice.is_none());
    }

    #[test]
    fn tools_rewrite_advice_below_threshold() {
        // desc_tokens below threshold — advice should not appear
        let advice = tools_rewrite_advice(true, "8790→api.anthropic.com", 8790, 1500, false, 2000);
        assert!(advice.is_none());
    }
}
