//! Agent hosts (`rtok agents setup|remove|list`).
//!
//! Everything the hosts share — backup, the dry-run/idempotence write gate, `mcpServers`
//! registration, the plugin-link offer — lives in `rtok-agent-sdk` (D28). Each host is one
//! folder here: `<host>/mod.rs` implements [`Agent`] (variants, files, installed modules,
//! apply) and `<host>/README.md` says which rtok modules the host takes, which it could take,
//! and why the rest cannot be taken; a unit test keeps the README and `support()` in step.
//! `rtok agents list` and `rtok doctor` read the same files back through the same contract.

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod opencode;
pub mod pi;

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use rtok_agent_sdk::NO_CHANGES;
use rtok_plugin_sdk::Surface;

use crate::config::Config;

/// Every host rtok installs into, in `agents list` order.
pub const HOSTS: &[&str] = &["claude", "cursor", "codex", "opencode", "pi"];

/// Every module an rtok install can carry, in print order.
pub const MODULES: &[&str] = &["hooks", "mcp", "proxy", "plugin"];

/// The host behind an id. `None` is refused by the CLI before any backup is taken.
pub fn host(id: &str) -> Option<&'static dyn Agent> {
    match id {
        "claude" => Some(&claude::Claude),
        "cursor" => Some(&cursor::Cursor),
        "codex" => Some(&codex::Codex),
        "opencode" => Some(&opencode::OpenCode),
        "pi" => Some(&pi::Pi),
        _ => None,
    }
}

/// How the app runs: a terminal binary or a desktop application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cli,
    Desktop,
}

impl Kind {
    /// The block header word: `CLI: Codex`, `Desktop: Claude Desktop`.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Cli => "CLI",
            Kind::Desktop => "Desktop",
        }
    }

    /// The flag spelling (`--cli`, `--desktop`) and the word `rtok doctor` prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Cli => "cli",
            Kind::Desktop => "desktop",
        }
    }
}

/// One app of a host. `bins` are looked up on PATH and asked `--version`; `apps` are where a
/// desktop build installs (`~/…`, `$VAR/…` or absolute), probed with `exists()`. Entries for
/// other platforms simply never exist.
pub struct Variant {
    pub kind: Kind,
    pub name: &'static str,
    pub bins: &'static [&'static str],
    pub apps: &'static [&'static str],
}

/// Whether `setup` can write a module into a host variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Written by a plain `setup`.
    Yes,
    /// Written only when this flag is given (`--proxy`, `--yes`).
    Flag(&'static str),
    /// Cannot be written today; the reason is the README's, in one line.
    No(&'static str),
}

/// What `rtok agents setup|remove` does to a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Install,
    Remove,
    /// `setup claude --replace`: drop legacy token hooks and retarget the proxy.
    Replace,
}

/// The contract every host folder implements. The generic [`run`] loop, [`list`] and
/// `rtok doctor` know nothing else about a host.
pub trait Agent: Sync {
    fn id(&self) -> &'static str;
    fn variants(&self) -> &'static [Variant];
    /// The folder's README, for the parity test.
    fn readme(&self) -> &'static str;
    /// True when every variant reads the same files (Cursor): one apply covers them all.
    fn shared(&self) -> bool {
        false
    }
    fn support(&self, kind: Kind, module: &str) -> Support;
    /// Config files an install writes; copied before any write. Empty for a host that owns a
    /// linked directory instead of a file (pi).
    fn files(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf>;
    /// Paths whose presence (or whose parent's) means the app is installed. Defaults to
    /// [`Agent::files`]; a host adds its plugin directory.
    fn markers(&self, cfg: &Config, kind: Kind) -> Vec<PathBuf> {
        self.files(cfg, kind)
    }
    /// Modules found in the host's files: the same markers the installer writes.
    fn installed(&self, cfg: &Config, kind: Kind) -> Vec<&'static str>;
    /// The plugin surfaces the linked `plugin` module serves on this host (Cursor: hook and
    /// MCP as one unit; pi: the bash call path). Empty where there is no plugin module.
    fn plugin_surfaces(&self) -> &'static [Surface] {
        &[]
    }
    /// Run the installer. One report per step; a step that touched nothing reports
    /// [`NO_CHANGES`].
    fn apply(&self, cfg: &Config, kind: Kind, mode: Mode) -> Result<Vec<String>>;
}

/// Whether `kind` is wanted given `--cli/--desktop/--all`. No flag (or `--all`) means every
/// variant.
pub fn wants(kind: Kind, cli: bool, desktop: bool, all: bool) -> bool {
    if all || (!cli && !desktop) {
        return true;
    }
    (kind == Kind::Cli && cli) || (kind == Kind::Desktop && desktop)
}

pub(crate) fn home_dir() -> PathBuf {
    crate::config::env_user_home().unwrap_or_default()
}

/// `~/x` and `$VAR/x` as a path on this machine; anything else unchanged.
fn expand_app(spec: &str) -> PathBuf {
    if let Some(rest) = spec.strip_prefix("~/") {
        return join_rel(&home_dir(), rest);
    }
    if let Some(rest) = spec.strip_prefix('$') {
        let (var, tail) = rest.split_once('/').unwrap_or((rest, ""));
        let Some(root) = std::env::var_os(var) else {
            return PathBuf::from(spec);
        };
        return join_rel(Path::new(&root), tail);
    }
    PathBuf::from(spec)
}

/// The first `bin` on PATH (`.exe`/`.cmd` on Windows).
fn find_on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let names: &[String] = if cfg!(windows) {
        &[bin.to_string(), format!("{bin}.exe"), format!("{bin}.cmd")]
    } else {
        &[bin.to_string()]
    };
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

/// Where the app is: a desktop bundle first, else the binary on PATH.
pub fn app_path(v: &Variant) -> Option<PathBuf> {
    v.apps
        .iter()
        .map(|a| expand_app(a))
        .find(|p| p.exists())
        .or_else(|| v.bins.iter().find_map(|b| find_on_path(b)))
}

/// Version of the installed app, or `"-"` when unknown.
/// Probes `<bin> --version` and keeps the first non-empty line (32 chars max).
pub fn app_version(v: &Variant) -> String {
    for bin in v.bins {
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

pub(crate) fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// True when the app itself is found: its bundle or binary exists, or one of its marker
/// paths (or the directory that would hold it) does. Setup skips a missing app instead of
/// creating its files; removal runs regardless so a half-installed host is cleaned up.
pub fn present(agent: &dyn Agent, v: &Variant, cfg: &Config) -> bool {
    app_path(v).is_some()
        || agent.markers(cfg, v.kind).iter().any(|p| {
            p.exists()
                || p.parent()
                    .is_some_and(|d| !d.as_os_str().is_empty() && d.exists())
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleState {
    Installed,
    NotInstalled,
    NotSupported,
}

/// One [`MODULES`] row of a host variant: its state and the note printed after it — the flag
/// that would install it, or the reason it cannot be installed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModuleRow {
    pub name: &'static str,
    pub state: ModuleState,
    pub note: String,
}

/// A module found in the host's files counts as installed even where setup cannot write it
/// (a hand-added MCP entry).
pub fn module_rows(agent: &dyn Agent, kind: Kind, cfg: &Config) -> Vec<ModuleRow> {
    let found = agent.installed(cfg, kind);
    MODULES
        .iter()
        .map(|&name| {
            let support = agent.support(kind, name);
            let (state, note) = if found.contains(&name) {
                (ModuleState::Installed, String::new())
            } else {
                match support {
                    Support::Yes => (ModuleState::NotInstalled, String::new()),
                    Support::Flag(flag) => (ModuleState::NotInstalled, format!(" ({flag})")),
                    Support::No(why) => (ModuleState::NotSupported, format!(": {why}")),
                }
            };
            ModuleRow { name, state, note }
        })
        .collect()
}

impl ModuleState {
    fn mark(self) -> &'static str {
        match self {
            ModuleState::Installed => "✓ ",
            ModuleState::NotInstalled => "✗ ",
            ModuleState::NotSupported => "− ",
        }
    }

    fn word(self) -> &'static str {
        match self {
            ModuleState::Installed => "installed",
            ModuleState::NotInstalled => "not installed",
            ModuleState::NotSupported => "not supported",
        }
    }

    /// `{indent}{mark}{text}` — green, red `✗` for not installed, grey `−` for not supported,
    /// coloured only where stdout takes colour (see `render`). `console = false` drops marks
    /// and colour: the doctor text also lands in the PDF report, whose built-in font has no
    /// `✓`.
    fn line(self, indent: &str, text: &str, console: bool) -> String {
        use owo_colors::{OwoColorize, Stream};
        if !console {
            return format!("{indent}{text}\n");
        }
        let line = format!("{indent}{}{text}", self.mark());
        let line = match self {
            ModuleState::Installed => line
                .if_supports_color(Stream::Stdout, |t| t.green())
                .to_string(),
            ModuleState::NotInstalled => line
                .if_supports_color(Stream::Stdout, |t| t.red())
                .to_string(),
            ModuleState::NotSupported => line
                .if_supports_color(Stream::Stdout, |t| t.bright_black())
                .to_string(),
        };
        format!("{line}\n")
    }
}

/// `✓ hooks   installed`, one line per module row.
pub fn module_lines(rows: &[ModuleRow], indent: &str, console: bool) -> String {
    rows.iter()
        .map(|row| {
            let text = format!("{:<7} {}{}", row.name, row.state.word(), row.note);
            row.state.line(indent, &text, console)
        })
        .collect()
}

/// The modules an install should leave behind: every `Yes` module (`mcp` only with
/// `[setup] mcp`), and a flag module only when its flag was given.
pub fn expected(agent: &dyn Agent, kind: Kind, cfg: &Config) -> Vec<&'static str> {
    MODULES
        .iter()
        .copied()
        .filter(|m| match agent.support(kind, m) {
            Support::Yes => *m != "mcp" || cfg.setup.mcp,
            Support::Flag("--proxy") => cfg.setup.proxy,
            Support::Flag("--yes") => cfg.setup.yes,
            Support::Flag(_) | Support::No(_) => false,
        })
        .collect()
}

/// Expected modules that do not read back from the host's files: the installer checking its
/// own write (a host that rewrote the file, a marker the reader does not recognise).
pub fn missing(agent: &dyn Agent, kind: Kind, cfg: &Config) -> Vec<&'static str> {
    let have = agent.installed(cfg, kind);
    expected(agent, kind, cfg)
        .into_iter()
        .filter(|m| !have.contains(m))
        .collect()
}

/// True when the `rtok` the configs spawn resolves: bare on PATH, or written absolute.
fn rtok_spawns() -> bool {
    bare_rtok_on_path(std::env::var_os("PATH").as_deref()) || rtok_command() != "rtok"
}

/// One of rtok's own plugins as a host variant reaches it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PluginRow {
    pub id: &'static str,
    /// `[plugins.<id>] enabled`; printed as `(off)` when false.
    pub on: bool,
    pub state: ModuleState,
}

/// The modules that carry a plugin surface into a host: hooks carry `hook` and the bash
/// call path (`cli`), MCP carries `mcp`, the proxy carries `proxy`, and the linked plugin
/// carries whatever [`Agent::plugin_surfaces`] says it serves.
fn modules_for(agent: &dyn Agent, surface: Surface) -> Vec<&'static str> {
    let mut out = match surface {
        Surface::Hook | Surface::Cli => vec!["hooks"],
        Surface::Mcp => vec!["mcp"],
        Surface::Proxy => vec!["proxy"],
    };
    if agent.plugin_surfaces().contains(&surface) {
        out.push("plugin");
    }
    out
}

/// True when at least one of the plugin's surfaces has a module setup can write here.
pub fn reaches(agent: &dyn Agent, kind: Kind, surfaces: &[Surface]) -> bool {
    surfaces.iter().any(|&s| {
        modules_for(agent, s)
            .iter()
            .any(|m| !matches!(agent.support(kind, m), Support::No(_)))
    })
}

/// Every catalogue plugin grouped like the modules: installed when one of its surfaces rides
/// an installed module, not installed when one could, not supported otherwise.
pub fn plugin_rows(agent: &dyn Agent, kind: Kind, cfg: &Config) -> Vec<PluginRow> {
    let modules = module_rows(agent, kind, cfg);
    let state_of = |name: &str| modules.iter().find(|r| r.name == name).map(|r| r.state);
    crate::plugins::Registry::new(cfg)
        .manifests()
        .into_iter()
        .map(|(m, on)| {
            let states: Vec<ModuleState> = m
                .surfaces
                .iter()
                .flat_map(|&s| modules_for(agent, s))
                .filter_map(state_of)
                .collect();
            let state = if states.contains(&ModuleState::Installed) {
                ModuleState::Installed
            } else if states.contains(&ModuleState::NotInstalled) {
                ModuleState::NotInstalled
            } else {
                ModuleState::NotSupported
            };
            PluginRow {
                id: m.id,
                on,
                state,
            }
        })
        .collect()
}

/// ```text
///   plugins
///     ✓ installed      cmd, read, inject
///     ✗ not installed  proxy, compress (off)
///     − not supported  -
/// ```
pub fn plugin_lines(rows: &[PluginRow], indent: &str, console: bool) -> String {
    let mut out = format!("{indent}plugins\n");
    let inner = format!("{indent}  ");
    for state in [
        ModuleState::Installed,
        ModuleState::NotInstalled,
        ModuleState::NotSupported,
    ] {
        let ids: Vec<String> = rows
            .iter()
            .filter(|r| r.state == state)
            .map(|r| {
                if r.on {
                    r.id.to_string()
                } else {
                    format!("{} (off)", r.id)
                }
            })
            .collect();
        let ids = if ids.is_empty() {
            "-".to_string()
        } else {
            ids.join(", ")
        };
        out.push_str(&state.line(&inner, &format!("{:<14} {ids}", state.word()), console));
    }
    out
}

/// What a block reports about its variant.
pub enum Outcome<'a> {
    /// The app is not on this machine: header, app and config lines only.
    NotFound,
    /// `agents list`: state, no installer ran.
    Listed,
    /// The installer ran with these step reports.
    Applied { mode: Mode, reports: &'a [String] },
    /// A shared-config host already applied under this sibling variant.
    Shared(&'static str),
}

/// The block one host variant prints:
///
/// ```text
/// CLI: Codex — already installed
///   app     /opt/homebrew/bin/codex (codex-cli 0.40.0)
///   config  /Users/me/.codex/config.toml
///   ✓ mcp     installed
///   ✗ proxy   not installed (--proxy)
///   − hooks   not supported: Codex has no shell hooks
/// ```
pub fn block(agent: &dyn Agent, v: &Variant, cfg: &Config, outcome: Outcome) -> String {
    let note = match &outcome {
        Outcome::NotFound => " — not found",
        Outcome::Listed => "",
        Outcome::Applied { .. } if cfg.setup.dry_run => " — dry run, nothing written",
        Outcome::Applied { mode, reports } if reports.iter().all(|r| r == NO_CHANGES) => {
            if *mode == Mode::Remove {
                " — no changes"
            } else {
                " — already installed"
            }
        }
        Outcome::Applied { .. } => "",
        Outcome::Shared(_) => " — same files as above",
    };
    let mut out = format!("{}: {}{note}\n", v.kind.label(), v.name);
    match app_path(v) {
        Some(p) => out.push_str(&format!("  app     {} ({})\n", p.display(), app_version(v))),
        None => out.push_str("  app     -\n"),
    }
    let files = agent.files(cfg, v.kind);
    if !files.is_empty() {
        let files: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        out.push_str(&format!("  config  {}\n", files.join(", ")));
    }
    if matches!(outcome, Outcome::NotFound) {
        return out;
    }
    if let Outcome::Applied { reports, .. } = &outcome {
        // Steps that changed something print their `+`/`-` lines in diff colours (T12.6).
        let changed: Vec<&str> = reports
            .iter()
            .filter(|r| *r != NO_CHANGES)
            .map(String::as_str)
            .collect();
        if !changed.is_empty() {
            out.push_str(&crate::render::paint(&changed.join("\n")));
            out.push('\n');
        }
    }
    out.push_str(&module_lines(&module_rows(agent, v.kind, cfg), "  ", true));
    out.push_str(&plugin_lines(&plugin_rows(agent, v.kind, cfg), "  ", true));
    out
}

/// What `rtok agents setup|remove` was asked to do.
pub struct Request {
    pub hosts: Vec<String>,
    pub mode: Mode,
    pub cli: bool,
    pub desktop: bool,
    pub all: bool,
}

/// Every host named, or the first unknown name — checked before any backup is taken.
pub fn resolve(hosts: &[String]) -> Result<Vec<&'static dyn Agent>> {
    if hosts.is_empty() {
        bail!("no host given");
    }
    hosts
        .iter()
        .map(|h| host(h).ok_or_else(|| anyhow::anyhow!("unknown host: {h}")))
        .collect()
}

/// Back up, then install into (or remove from) every wanted variant of every host, one
/// [`block`] each. The copy is taken up front, before any installer runs, so one `.bak-<ts>`
/// per file holds the host exactly as it was — not as it was midway through a multi-file
/// edit; the installers therefore must not take one of their own (`cfg.setup.backup` is
/// cleared here). A run that then wrote nothing removes the copies it took: `already
/// installed` leaves the directory as it found it.
pub fn run(cfg: &mut Config, req: &Request) -> Result<String> {
    let agents = resolve(&req.hosts)?;
    let want = |kind: Kind| req.mode == Mode::Remove || wants(kind, req.cli, req.desktop, req.all);
    let mut out = String::new();
    if req.mode != Mode::Remove && !rtok_spawns() {
        out.push_str(
            "warning: rtok is not on PATH; the hooks and MCP entries spawn `rtok` by name and will fail until it is\n",
        );
    }
    let mut taken: Vec<PathBuf> = Vec::new();
    if !cfg.setup.dry_run && cfg.setup.backup {
        let mut seen: Vec<PathBuf> = Vec::new();
        for a in &agents {
            for v in a.variants().iter().filter(|v| want(v.kind)) {
                for path in a.files(cfg, v.kind) {
                    if seen.contains(&path) {
                        continue;
                    }
                    if let Some(bak) = rtok_agent_sdk::backup(&path)? {
                        taken.push(bak);
                    }
                    seen.push(path);
                }
            }
        }
        cfg.setup.backup = false;
    }
    let (blocks, changed) = apply_all(cfg, req, &agents, want)?;
    if changed {
        for bak in &taken {
            out.push_str(&format!("backup {}\n", bak.display()));
        }
    } else {
        for bak in &taken {
            let _ = std::fs::remove_file(bak);
        }
    }
    out.push_str(&blocks);
    Ok(out)
}

/// The blocks of every wanted variant, and whether any step changed a file.
fn apply_all(
    cfg: &Config,
    req: &Request,
    agents: &[&'static dyn Agent],
    want: impl Fn(Kind) -> bool,
) -> Result<(String, bool)> {
    let mut out = String::new();
    let mut changed = false;
    for &a in agents {
        let mut done: Option<&'static str> = None;
        let mut any = false;
        for v in a.variants().iter().filter(|v| want(v.kind)) {
            any = true;
            if req.mode != Mode::Remove && !present(a, v, cfg) {
                out.push_str(&block(a, v, cfg, Outcome::NotFound));
                continue;
            }
            if let (Some(first), true) = (done, a.shared()) {
                out.push_str(&block(a, v, cfg, Outcome::Shared(first)));
                continue;
            }
            let reports = a.apply(cfg, v.kind, req.mode)?;
            done = Some(v.name);
            changed |= reports.iter().any(|r| r != NO_CHANGES);
            out.push_str(&block(
                a,
                v,
                cfg,
                Outcome::Applied {
                    mode: req.mode,
                    reports: &reports,
                },
            ));
            if req.mode == Mode::Install && !cfg.setup.dry_run {
                for m in missing(a, v.kind, cfg) {
                    out.push_str(&format!("  warning: {m} did not read back as installed\n"));
                }
            }
        }
        if !any {
            out.push_str(&format!("skip {}: not selected\n", a.id()));
        }
    }
    Ok((out, changed))
}

/// `rtok agents list`: every known host × variant as a [`block`], nothing written.
pub fn list(cfg: &Config) -> String {
    let mut out = String::new();
    for id in HOSTS {
        let Some(a) = host(id) else { continue };
        for v in a.variants() {
            let outcome = if present(a, v, cfg) {
                Outcome::Listed
            } else {
                Outcome::NotFound
            };
            out.push_str(&block(a, v, cfg, outcome));
        }
    }
    out
}

/// The `[setup]` flags an installer acts on, as the SDK spells them.
pub(crate) fn apply(cfg: &crate::config::Config) -> rtok_agent_sdk::Apply {
    rtok_agent_sdk::Apply {
        dry_run: cfg.setup.dry_run,
        backup: cfg.setup.backup,
        yes: cfg.setup.yes,
    }
}

/// The `ANTHROPIC_BASE_URL` `agent setup claude --proxy` writes, and the one it reads back.
pub(crate) fn anthropic_proxy_url(cfg: &crate::config::Config) -> String {
    format!("http://{}:{}", cfg.proxy.bind, cfg.proxy.port)
}

pub(crate) fn openai_proxy_url(cfg: &crate::config::Config) -> String {
    format!("http://{}:{}/v1", cfg.proxy.bind, cfg.proxy.port)
}

/// Join `rel` onto `base` by path components so Windows never gets a single
/// component with embedded slashes (`plugins/cursor` → `plugins\cursor`).
fn join_rel(base: &std::path::Path, rel: &str) -> std::path::PathBuf {
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
        out.push(part);
    }
    out
}

/// Command string written into host configs for hooks and MCP.
///
/// Prefer bare `rtok` when it resolves on PATH. On Windows, a fresh install
/// often updates the user PATH while the host still has the old one — bare
/// `rtok` then fails to spawn. Fall back to the absolute `current_exe`
/// (typically `…\rtok.exe`) so Claude/Cursor/Codex can still start it.
pub(crate) fn rtok_command() -> String {
    resolve_rtok_command(
        std::env::current_exe().ok().as_deref(),
        std::env::var_os("PATH").as_deref(),
    )
}

/// Pure resolution used by [`rtok_command`] and unit tests.
pub(crate) fn resolve_rtok_command(
    exe: Option<&std::path::Path>,
    path_os: Option<&std::ffi::OsStr>,
) -> String {
    if bare_rtok_on_path(path_os) {
        return "rtok".to_string();
    }
    // Non-Windows hosts keep the bare name even when PATH lookup fails: shell
    // hooks expect `rtok` and absolute paths are a Windows spawn edge.
    if !cfg!(windows) {
        return "rtok".to_string();
    }
    if let Some(exe) = exe {
        return dunce::simplified(exe).to_string_lossy().into_owned();
    }
    "rtok".to_string()
}

fn bare_rtok_on_path(path: Option<&std::ffi::OsStr>) -> bool {
    let Some(path) = path else {
        return false;
    };
    for dir in std::env::split_paths(path) {
        if cfg!(windows) {
            if dir.join("rtok.exe").is_file() || dir.join("rtok").is_file() {
                return true;
            }
        } else if dir.join("rtok").is_file() {
            return true;
        }
    }
    false
}

/// True when `bin` names the rtok binary (bare, `.exe`, or an absolute path).
pub(crate) fn is_rtok_bin(bin: &str) -> bool {
    let base = bin.rsplit(['/', '\\']).next().unwrap_or(bin);
    // Windows executable suffix casing is arbitrary (`.Exe`, `.eXe`, …).
    let stem = if base.len() >= 4 && base[base.len() - 4..].eq_ignore_ascii_case(".exe") {
        &base[..base.len() - 4]
    } else {
        base
    };
    stem.eq_ignore_ascii_case("rtok")
}

/// Quote `bin` for a host shell hook command when it contains whitespace.
///
/// MCP JSON/TOML take an unquoted path string; Claude/Cursor hook `command`
/// values are shell lines, so an absolute `…\Ivan Tuhai\…\rtok.exe hook …`
/// would split on the space and fail to start.
pub(crate) fn shell_quote_bin(bin: &str) -> String {
    if !bin.chars().any(|c| c.is_whitespace() || c == '"') {
        return bin.to_string();
    }
    format!("\"{}\"", bin.replace('"', "\\\""))
}

/// Binary token for hook command lines (quoted when needed).
pub(crate) fn rtok_hook_bin() -> String {
    shell_quote_bin(&rtok_command())
}

/// Strip one layer of surrounding quotes from a hook binary token.
pub(crate) fn unquote_bin(bin: &str) -> &str {
    let b = bin.trim();
    if b.len() >= 2 {
        let bytes = b.as_bytes();
        if (bytes[0] == b'"' && *bytes.last().unwrap() == b'"')
            || (bytes[0] == b'\'' && *bytes.last().unwrap() == b'\'')
        {
            return &b[1..b.len() - 1];
        }
    }
    b
}

/// The tree this repo ships a host plugin from (D21 (6)).
///
/// Resolution order:
/// 1. `rel` next to the running binary (release archives ship `plugins/` beside `rtok`);
/// 2. ketch layout: when the exe lives in `<root>/bin/`, prefer
///    `<root>/store/rtok/v{version}/` matching `CARGO_PKG_VERSION`, else the
///    newest `store/rtok/*/` that contains `rel`;
/// 3. `CARGO_MANIFEST_DIR/rel` for `cargo test` / dev;
/// 4. beside-exe path for a clear error when nothing exists.
pub(crate) fn plugin_src(rel: &str) -> std::path::PathBuf {
    resolve_plugin_src(
        rel,
        std::env::current_exe().ok().as_deref(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        env!("CARGO_PKG_VERSION"),
    )
}

/// Pure resolution used by [`plugin_src`] and unit tests (fake exe / ketch layout).
pub(crate) fn resolve_plugin_src(
    rel: &str,
    exe: Option<&std::path::Path>,
    manifest_dir: &std::path::Path,
    pkg_version: &str,
) -> std::path::PathBuf {
    let cargo = join_rel(manifest_dir, rel);
    let beside = exe.and_then(|e| e.parent()).map(|dir| join_rel(dir, rel));

    if let Some(ref p) = beside
        && p.is_dir()
    {
        return p.clone();
    }

    if let Some(exe) = exe
        && let Some(bin_dir) = exe.parent()
        && bin_dir
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("bin"))
        && let Some(root) = bin_dir.parent()
        && let Some(found) = ketch_store_plugin(root, rel, pkg_version)
    {
        return found;
    }

    if cargo.is_dir() {
        return cargo;
    }

    beside.unwrap_or(cargo)
}

/// ketch installs the full package under `<root>/store/rtok/vX.Y.Z/` and only
/// copies the binary into `<root>/bin/`. Prefer the version that matches this
/// build; otherwise take the newest store folder that still has `rel`.
fn ketch_store_plugin(
    root: &std::path::Path,
    rel: &str,
    pkg_version: &str,
) -> Option<std::path::PathBuf> {
    let store = root.join("store").join("rtok");
    let versioned = join_rel(&store.join(format!("v{pkg_version}")), rel);
    if versioned.is_dir() {
        return Some(versioned);
    }
    let mut entries: Vec<_> = std::fs::read_dir(&store)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    // Lexicographic order is enough for `v0.1.5`-style tags; newest last.
    entries.sort_by(|a, b| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });
    for dir in entries.into_iter().rev() {
        let candidate = join_rel(&dir, rel);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::{Path, PathBuf};

    #[test]
    fn resolve_rtok_command_keeps_bare_name_when_on_path() {
        let dir = std::env::temp_dir().join(format!("rtok-setup-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join(if cfg!(windows) { "rtok.exe" } else { "rtok" });
        std::fs::write(&bin, b"x").unwrap();
        let path = std::env::join_paths([dir.as_os_str()]).unwrap();
        assert_eq!(
            resolve_rtok_command(
                Some(Path::new(r"C:\nowhere\rtok.exe")),
                Some(path.as_os_str())
            ),
            "rtok"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_rtok_command_uses_absolute_exe_when_path_misses_on_windows() {
        let exe = PathBuf::from(if cfg!(windows) {
            r"C:\Users\u\.ketch\bin\rtok.exe"
        } else {
            "/opt/rtok"
        });
        let empty = std::ffi::OsString::new();
        let got = resolve_rtok_command(Some(&exe), Some(empty.as_os_str()));
        if cfg!(windows) {
            assert!(got.ends_with("rtok.exe"), "{got}");
            assert!(got.contains("ketch") || got.contains("Users"), "{got}");
        } else {
            assert_eq!(got, "rtok");
        }
    }

    #[test]
    fn is_rtok_bin_accepts_absolute_windows_exe() {
        assert!(is_rtok_bin("rtok"));
        assert!(is_rtok_bin("rtok.exe"));
        assert!(is_rtok_bin("RTOK.Exe"));
        assert!(is_rtok_bin(r"C:\Users\u\.ketch\bin\rtok.eXe"));
        assert!(is_rtok_bin(r"C:\Users\u\.ketch\bin\rtok.exe"));
        assert!(!is_rtok_bin("rtok-extra"));
        assert!(!is_rtok_bin(r"C:\bin\other.exe"));
    }

    #[test]
    fn shell_quote_bin_quotes_paths_with_spaces() {
        assert_eq!(shell_quote_bin("rtok"), "rtok");
        let spaced = r"C:\Users\Ivan Tuhai\.ketch\bin\rtok.exe";
        let quoted = shell_quote_bin(spaced);
        assert!(quoted.starts_with('"') && quoted.ends_with('"'), "{quoted}");
        assert!(quoted.contains("Ivan Tuhai"), "{quoted}");
        assert_eq!(unquote_bin(&quoted), spaced);
        assert!(is_rtok_bin(unquote_bin(&quoted)));
    }

    #[test]
    fn apply_carries_the_setup_flags() {
        let mut cfg = Config::default();
        cfg.setup.dry_run = true;
        cfg.setup.yes = true;
        cfg.setup.backup = false;
        let a = apply(&cfg);
        assert!(a.dry_run && a.yes && !a.backup);
    }

    fn write_plugin(dir: &std::path::Path) {
        use std::fs;
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("plugin.json"), "{}").unwrap();
    }

    #[test]
    fn plugin_src_prefers_directory_beside_exe() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("rtok-plugin-src-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let bin_dir = root.join("bin");
        let plugins = bin_dir.join("plugins").join("cursor");
        write_plugin(&plugins);
        let fake_exe = bin_dir.join("rtok");
        fs::write(&fake_exe, b"").unwrap();
        let missing_manifest = root.join("no-such-manifest");
        let got = resolve_plugin_src(
            "plugins/cursor",
            Some(&fake_exe),
            &missing_manifest,
            "0.1.5",
        );
        assert_eq!(got, plugins);
        // When beside-exe is missing, fall back to an existing cargo tree.
        let cargo_root = root.join("cargo");
        let cargo_plugins = cargo_root.join("plugins").join("cursor");
        write_plugin(&cargo_plugins);
        let lonely_exe = root.join("lonely").join("rtok");
        fs::create_dir_all(lonely_exe.parent().unwrap()).unwrap();
        fs::write(&lonely_exe, b"").unwrap();
        let got = resolve_plugin_src("plugins/cursor", Some(&lonely_exe), &cargo_root, "0.1.5");
        assert_eq!(got, cargo_plugins);
        // Neither exists: still return the beside-exe path for a clear error.
        let empty = root.join("empty");
        let empty_exe = empty.join("rtok");
        fs::create_dir_all(&empty).unwrap();
        fs::write(&empty_exe, b"").unwrap();
        let got = resolve_plugin_src("plugins/cursor", Some(&empty_exe), &empty, "0.1.5");
        assert_eq!(got, empty.join("plugins").join("cursor"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plugin_src_finds_ketch_store_when_bin_has_no_plugins() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("rtok-ketch-src-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let fake_exe = bin_dir.join("rtok");
        fs::write(&fake_exe, b"").unwrap();
        let store_plugins = root
            .join("store")
            .join("rtok")
            .join("v0.1.5")
            .join("plugins")
            .join("cursor");
        write_plugin(&store_plugins);
        let missing_manifest = root.join("no-such-manifest");
        let got = resolve_plugin_src(
            "plugins/cursor",
            Some(&fake_exe),
            &missing_manifest,
            "0.1.5",
        );
        assert_eq!(got, store_plugins);
        // Still prefer plugins beside the bin exe when both exist.
        let beside = bin_dir.join("plugins").join("cursor");
        write_plugin(&beside);
        let got = resolve_plugin_src(
            "plugins/cursor",
            Some(&fake_exe),
            &missing_manifest,
            "0.1.5",
        );
        assert_eq!(got, beside);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plugin_src_finds_ketch_store_when_bin_dir_is_capitalised() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("rtok-ketch-Bin-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // Windows (and some shells) can surface the ketch bin dir as `Bin`.
        let bin_dir = root.join("Bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let fake_exe = bin_dir.join("rtok");
        fs::write(&fake_exe, b"").unwrap();
        let store_plugins = root
            .join("store")
            .join("rtok")
            .join("v0.1.6")
            .join("plugins")
            .join("cursor");
        write_plugin(&store_plugins);
        let missing_manifest = root.join("no-such-manifest");
        let got = resolve_plugin_src(
            "plugins/cursor",
            Some(&fake_exe),
            &missing_manifest,
            "0.1.6",
        );
        assert_eq!(got, store_plugins);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plugin_src_prefers_matching_ketch_store_version() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("rtok-ketch-ver-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let fake_exe = bin_dir.join("rtok");
        fs::write(&fake_exe, b"").unwrap();
        let store = root.join("store").join("rtok");
        let older = store.join("v0.1.4").join("plugins").join("cursor");
        let newer = store.join("v0.1.5").join("plugins").join("cursor");
        write_plugin(&older);
        write_plugin(&newer);
        let missing_manifest = root.join("no-such-manifest");
        // Matching CARGO_PKG_VERSION wins even when a newer folder exists.
        let got = resolve_plugin_src(
            "plugins/cursor",
            Some(&fake_exe),
            &missing_manifest,
            "0.1.4",
        );
        assert_eq!(got, older);
        // When the matching version has no plugins, take the newest that does.
        fs::remove_dir_all(store.join("v0.1.4")).unwrap();
        let got = resolve_plugin_src(
            "plugins/cursor",
            Some(&fake_exe),
            &missing_manifest,
            "0.1.4",
        );
        assert_eq!(got, newer);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn join_rel_splits_slash_and_backslash() {
        let base = std::path::Path::new("/tmp/root");
        assert_eq!(
            join_rel(base, "plugins/cursor"),
            base.join("plugins").join("cursor")
        );
        assert_eq!(
            join_rel(base, "plugins\\cursor"),
            base.join("plugins").join("cursor")
        );
    }

    #[test]
    fn variant_filter_defaults_to_all() {
        assert!(wants(Kind::Cli, false, false, false));
        assert!(wants(Kind::Desktop, false, false, false));
        assert!(wants(Kind::Cli, false, false, true));
        assert!(wants(Kind::Desktop, false, false, true));
        assert!(wants(Kind::Cli, true, false, false));
        assert!(!wants(Kind::Desktop, true, false, false));
        assert!(!wants(Kind::Cli, false, true, false));
        assert!(wants(Kind::Desktop, false, true, false));
        let kinds = |id: &str| -> Vec<Kind> {
            host(id)
                .unwrap()
                .variants()
                .iter()
                .map(|v| v.kind)
                .collect()
        };
        assert_eq!(kinds("cursor"), [Kind::Cli, Kind::Desktop]);
        assert_eq!(kinds("opencode"), [Kind::Cli, Kind::Desktop]);
        assert_eq!(kinds("claude"), [Kind::Cli, Kind::Desktop]);
        assert_eq!(kinds("codex"), [Kind::Cli]);
        assert!(host("windsurf").is_none());
        assert!(resolve(&["claude".into(), "nope".into()]).is_err());
    }

    #[test]
    fn list_prints_one_block_per_app_with_kind_name_app_and_config() {
        let out = list(&Config::default());
        for head in [
            "CLI: Claude Code",
            "Desktop: Claude Desktop",
            "CLI: Cursor CLI",
            "Desktop: Cursor",
            "CLI: Codex",
            "CLI: OpenCode",
            "Desktop: OpenCode Desktop",
            "CLI: pi",
        ] {
            assert!(
                out.contains(&format!("{head}\n")) || out.contains(&format!("{head} — ")),
                "{head} missing:\n{out}"
            );
        }
        assert!(out.contains("  app     "), "{out}");
        assert!(out.contains("  config  "), "{out}");
        assert!(out.contains("  plugins\n"), "{out}");
    }

    /// Codex with an MCP block and no provider: MCP plugins ride it, proxy-only plugins wait
    /// for `--proxy`, hook-only plugins have nowhere to go. A disabled plugin says so.
    #[test]
    fn plugins_group_by_the_modules_that_carry_their_surfaces() {
        let dir = std::env::temp_dir().join(format!("rtok-plugrows-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.setup.codex.config_path = dir.join("config.toml");
        std::fs::write(
            &cfg.setup.codex.config_path,
            "[mcp_servers.rtok]\ncommand = \"rtok\"\n",
        )
        .unwrap();
        let rows = plugin_rows(&codex::Codex, Kind::Cli, &cfg);
        let _ = std::fs::remove_dir_all(&dir);
        let state = |id: &str| rows.iter().find(|r| r.id == id).unwrap().state;
        assert_eq!(state("read"), ModuleState::Installed);
        assert_eq!(state("graph"), ModuleState::Installed);
        assert_eq!(state("proxy"), ModuleState::NotInstalled);
        assert_eq!(state("measure"), ModuleState::NotInstalled);
        assert_eq!(state("cmd"), ModuleState::NotSupported);
        assert_eq!(state("guard"), ModuleState::NotSupported);
        let text = plugin_lines(&rows, "", true);
        assert!(text.contains("✓ installed      "), "{text}");
        assert!(text.contains("compress (off)"), "{text}");
        assert!(
            text.contains("− not supported  cmd, inject, guard"),
            "{text}"
        );
        // pi reaches the bash call path through its extension, nothing else.
        assert!(reaches(&pi::Pi, Kind::Cli, &[Surface::Cli]));
        assert!(!reaches(
            &pi::Pi,
            Kind::Cli,
            &[Surface::Mcp, Surface::Proxy]
        ));
        assert!(reaches(&cursor::Cursor, Kind::Desktop, &[Surface::Mcp]));
    }

    /// What an install must leave behind follows `support()` and the flags given; against
    /// files that do not exist, every expected module is missing.
    #[test]
    fn expected_modules_follow_support_and_flags_and_missing_reads_them_back() {
        let mut cfg = Config::default();
        cfg.setup.codex.config_path = std::env::temp_dir()
            .join("rtok-no-such-dir")
            .join("config.toml");
        cfg.setup.claude.settings_path = std::env::temp_dir().join("rtok-no-such-dir/s.json");
        cfg.doctor.claude_json = std::env::temp_dir().join("rtok-no-such-dir/c.json");
        assert_eq!(expected(&codex::Codex, Kind::Cli, &cfg), ["mcp"]);
        assert_eq!(expected(&pi::Pi, Kind::Cli, &cfg), Vec::<&str>::new());
        assert_eq!(expected(&claude::Claude, Kind::Desktop, &cfg), ["mcp"]);
        cfg.setup.proxy = true;
        cfg.setup.yes = true;
        cfg.setup.mcp = false;
        assert_eq!(expected(&codex::Codex, Kind::Cli, &cfg), ["proxy"]);
        assert_eq!(
            expected(&claude::Claude, Kind::Cli, &cfg),
            ["hooks", "proxy"]
        );
        assert_eq!(expected(&pi::Pi, Kind::Cli, &cfg), ["plugin"]);
        assert_eq!(
            missing(&claude::Claude, Kind::Cli, &cfg),
            ["hooks", "proxy"]
        );
        assert_eq!(
            missing(&claude::Claude, Kind::Desktop, &cfg),
            Vec::<&str>::new()
        );
    }

    /// `~/x` follows the home dir, `$VAR/x` the variable, and an unset variable is left as
    /// written so the caller's `exists()` says no instead of probing a wrong root.
    #[test]
    fn expand_app_resolves_home_and_env_vars() {
        assert_eq!(expand_app("~/Apps/x"), join_rel(&home_dir(), "Apps/x"));
        let path = std::env::var_os("PATH").expect("PATH");
        assert_eq!(
            expand_app("$PATH/Claude/claude.exe"),
            join_rel(Path::new(&path), "Claude/claude.exe")
        );
        assert_eq!(
            expand_app("$RTOK_NO_SUCH_VAR/app"),
            PathBuf::from("$RTOK_NO_SUCH_VAR/app")
        );
        assert_eq!(
            expand_app("/Applications/Claude.app"),
            PathBuf::from("/Applications/Claude.app")
        );
    }

    /// A proxy on a non-default port read as not installed: the check looked for `8790`.
    #[test]
    fn claude_modules_read_back_hooks_and_a_proxy_on_any_port() {
        let dir = std::env::temp_dir().join(format!("rtok-mods-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.proxy.port = 9123;
        cfg.setup.claude.settings_path = dir.join("settings.json");
        cfg.doctor.claude_json = dir.join("claude.json");
        let settings = serde_json::json!({
            "hooks": {"SessionStart": [{"hooks": [{"command": "rtok hook SessionStart"}]}]},
            "env": {"ANTHROPIC_BASE_URL": anthropic_proxy_url(&cfg)},
        });
        std::fs::write(&cfg.setup.claude.settings_path, settings.to_string()).unwrap();
        let rows = module_rows(&claude::Claude, Kind::Cli, &cfg);
        let _ = std::fs::remove_dir_all(&dir);
        let states: Vec<(&str, ModuleState)> = rows.iter().map(|r| (r.name, r.state)).collect();
        assert_eq!(
            states,
            [
                ("hooks", ModuleState::Installed),
                ("mcp", ModuleState::NotInstalled),
                ("proxy", ModuleState::Installed),
                ("plugin", ModuleState::NotSupported),
            ]
        );
        let console = module_lines(&rows, "  ", true);
        assert!(console.contains("✓ hooks   installed"), "{console}");
        assert!(console.contains("✗ mcp     not installed"), "{console}");
        assert!(
            console.contains("− plugin  not supported: Claude Code"),
            "{console}"
        );
        let plain = module_lines(&rows, "  ", false);
        assert!(
            plain.contains("  proxy   installed") && !plain.contains('✓'),
            "{plain}"
        );
    }

    #[test]
    fn a_flag_module_names_its_flag_and_an_unknown_bin_has_no_path_or_version() {
        let mut cfg = Config::default();
        cfg.setup.codex.config_path = std::env::temp_dir()
            .join("rtok-no-such-dir")
            .join("config.toml");
        let rows = module_rows(&codex::Codex, Kind::Cli, &cfg);
        let row = |name: &str| rows.iter().find(|r| r.name == name).unwrap().clone();
        assert_eq!(row("proxy").state, ModuleState::NotInstalled);
        assert_eq!(row("proxy").note, " (--proxy)");
        assert_eq!(row("hooks").state, ModuleState::NotSupported);
        assert!(
            module_lines(&rows, "", true).contains("✗ proxy   not installed (--proxy)"),
            "{rows:?}"
        );
        let ghost = Variant {
            kind: Kind::Cli,
            name: "ghost",
            bins: &["rtok-no-such-binary"],
            apps: &["~/rtok-no-such-app", "$RTOK_NO_SUCH_VAR/app"],
        };
        assert_eq!(app_version(&ghost), "-");
        assert!(app_path(&ghost).is_none());
    }

    /// `| module | support | why |` rows of a host README, keyed by the first cell.
    fn readme_rows(readme: &str) -> std::collections::HashMap<String, (String, String)> {
        readme
            .lines()
            .filter_map(|l| {
                let cells: Vec<&str> = l
                    .trim()
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect();
                let is_row = cells.len() == 3 && MODULES.iter().any(|m| cells[0].starts_with(m));
                is_row.then(|| {
                    (
                        cells[0].to_string(),
                        (cells[1].to_string(), cells[2].to_string()),
                    )
                })
            })
            .collect()
    }

    /// `Reachable: a, b` / `Not reachable: c` lines of a host README, keyed by their label.
    fn readme_reach(readme: &str) -> std::collections::HashMap<String, Vec<String>> {
        readme
            .lines()
            .filter_map(|l| l.split_once(": "))
            .filter(|(label, _)| label.ends_with("eachable") || label.contains("eachable ("))
            .map(|(label, ids)| {
                let ids = ids
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "-")
                    .collect();
                (label.to_string(), ids)
            })
            .collect()
    }

    /// Each host README carries one row per module (`module` or `module (desktop)`) whose
    /// support cell is `yes`, the flag in backticks, or `no` — and a `no` row's reason is the
    /// very string `support()` prints. Its `Reachable:` / `Not reachable:` lines (per kind
    /// where they differ) name exactly the catalogue plugins the modules carry.
    #[test]
    fn readme_tables_match_support() {
        let manifests = crate::plugins::Registry::new(&Config::default()).manifests();
        for id in HOSTS {
            let agent = host(id).unwrap();
            let rows = readme_rows(agent.readme());
            let reach = readme_reach(agent.readme());
            for v in agent.variants() {
                let suffix = format!(" ({})", v.kind.as_str());
                // Only the plugins this build compiles in: a README names them all.
                let listed = |label: &str| -> Vec<String> {
                    reach
                        .get(&format!("{label}{suffix}"))
                        .or_else(|| reach.get(label))
                        .unwrap_or_else(|| panic!("{id} README has no `{label}:` line"))
                        .iter()
                        .filter(|p| manifests.iter().any(|(m, _)| m.id == p.as_str()))
                        .cloned()
                        .collect()
                };
                let (mut yes, mut no) = (Vec::new(), Vec::new());
                for (m, _) in &manifests {
                    if reaches(agent, v.kind, m.surfaces) {
                        yes.push(m.id.to_string());
                    } else {
                        no.push(m.id.to_string());
                    }
                }
                assert_eq!(listed("Reachable"), yes, "{id} ({})", v.name);
                assert_eq!(listed("Not reachable"), no, "{id} ({})", v.name);
                for module in MODULES {
                    let key = format!("{module} ({})", v.kind.as_str());
                    let (cell, why) = rows
                        .get(&key)
                        .or_else(|| rows.get(*module))
                        .unwrap_or_else(|| panic!("{id} README has no row for {module}"));
                    let want = match agent.support(v.kind, module) {
                        Support::Yes => "yes".to_string(),
                        Support::Flag(f) => format!("`{f}`"),
                        Support::No(reason) => {
                            assert_eq!(why, reason, "{id}: {key} reason");
                            "no".to_string()
                        }
                    };
                    assert_eq!(cell, &want, "{id}: {key}");
                }
            }
        }
    }

    fn cfg_with_cursor_hooks(hooks: PathBuf) -> Config {
        let mut cfg = Config::default();
        cfg.setup.cursor.hooks_path = hooks;
        cfg
    }

    fn cfg_with_claude(settings: PathBuf, claude_json: PathBuf) -> Config {
        let mut cfg = Config::default();
        cfg.setup.claude.settings_path = settings;
        cfg.doctor.claude_json = claude_json;
        cfg
    }

    #[test]
    fn present_when_cursor_dir_exists() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("rtok-present-cursor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cursor_dir = root.join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        let cfg = cfg_with_cursor_hooks(cursor_dir.join("hooks.json"));
        let a = &cursor::Cursor;
        for v in a.variants() {
            assert!(
                present(a, v, &cfg),
                "parent ~/.cursor must count as installed host ({})",
                v.name
            );
        }
    }

    #[test]
    fn present_when_claude_dir_exists() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("rtok-present-claude-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let claude_dir = root.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let cfg = cfg_with_claude(claude_dir.join("settings.json"), root.join(".claude.json"));
        let a = &claude::Claude;
        assert!(
            present(a, &a.variants()[0], &cfg),
            "parent ~/.claude must count as installed host"
        );
    }

    #[test]
    fn absent_when_config_paths_missing() {
        let root = std::env::temp_dir().join(format!("rtok-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let missing = root.join("no-such-dir");
        let a = &cursor::Cursor;
        let cfg = cfg_with_cursor_hooks(missing.join("hooks.json"));
        for v in a.variants().iter().filter(|v| app_path(v).is_none()) {
            assert!(!present(a, v, &cfg), "{}", v.name);
        }
        let a = &claude::Claude;
        let cfg = cfg_with_claude(missing.join("settings.json"), missing.join(".claude.json"));
        for v in a.variants().iter().filter(|v| app_path(v).is_none()) {
            assert!(!present(a, v, &cfg), "{}", v.name);
        }
    }
}
