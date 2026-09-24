//! Clap tree. `tests/config_coverage.rs` walks [`Cli::command`] (plan T12.4).

use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::config::layers;
use crate::config::validate;
use crate::demon::Service;
use crate::web::model;
use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

/// `0.1.0 (1a2b3c4d5)` — the sha comes from `build.rs` (T10.4).
pub(crate) const VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), " (", env!("RTOK_GIT_SHA"), ")");

/// Token-reduction CLI for AI coding agents. See plan.md for the task list.
#[derive(Parser)]
#[command(name = "rtok", version = VERSION, about)]
pub struct Cli {
    /// User config file (else `RTOK_CONFIG` or `<home>/config.toml`)
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Claude Code hook entry point: reads the event JSON on stdin, writes JSON to stdout
    Hook {
        #[arg(required_unless_present = "serve")]
        event: Option<String>,
        /// Overlay `[hook] host` (`claude` | `cursor` | `copilot` | `devin` | `cline`)
        #[arg(long)]
        host: Option<String>,
        /// Run the resident hook process `rtok-hook` talks to (T178, D32)
        #[arg(long, hide = true, conflicts_with_all = ["event", "host"])]
        serve: bool,
    },
    /// Serve MCP tools over stdio; `-- <server argv>` wraps a foreign server instead
    Mcp {
        /// Call one listed tool and print the text result (pi `registerTool` shim, T70.3)
        #[arg(long, value_name = "TOOL")]
        call: Option<String>,
        /// JSON arguments for `--call`
        #[arg(long, value_name = "ARGS")]
        json: Option<String>,
        /// Foreign stdio MCP server to wrap losslessly (`rtok mcp -- npx some-server`)
        #[arg(last = true)]
        wrap: Vec<String>,
    },
    /// Local API proxy for ANTHROPIC_BASE_URL
    Proxy {
        /// Override `[proxy] port`
        #[arg(long)]
        port: Option<u16>,
        /// Override `[proxy] upstream`
        #[arg(long)]
        upstream: Option<String>,
        /// Override `[proxy] mode` (`passthrough` | `compress`)
        #[arg(long)]
        mode: Option<String>,
        /// Print effective `[proxy]` settings and exit
        #[arg(long)]
        dry_run: bool,
    },
    /// Local web UI over the same data as `rtok tui` (WebSocket API + Slint/WASM)
    Web {
        /// Override `[web] host`
        #[arg(long)]
        host: Option<String>,
        /// Override `[web] port`
        #[arg(long)]
        port: Option<u16>,
    },
    /// Terminal UI over the same data as `rtok web` (D23: one model, two renderings)
    Tui {
        /// Start on this tab (a page name both surfaces carry)
        #[arg(long)]
        tab: Option<String>,
        /// Model re-read cadence in seconds
        #[arg(long)]
        tick_secs: Option<u64>,
    },
    /// Measurements from session logs and the proxy
    Stats {
        /// How far back to read transcripts (`60d`, `24h`)
        #[arg(long)]
        since: Option<String>,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
        /// Restrict to one tool or plugin id
        #[arg(long)]
        plugin: Option<String>,
        /// Write this report JSON to `<home>/measurements/<name>.json`
        #[arg(long, value_name = "NAME")]
        save_baseline: Option<String>,
        /// Print deltas against a saved baseline
        #[arg(long, value_name = "NAME")]
        compare: Option<String>,
        /// Fit chars-per-token via count_tokens (skipped without an API key)
        #[arg(long)]
        calibrate: bool,
        /// Cache health per session from proxy usage rows: busts and their cause
        #[arg(long)]
        cache: bool,
        /// Show per-model USD costs from `[stats.prices]` (`--price`)
        #[arg(long)]
        price: bool,
    },
    /// A/B benchmark of host configurations
    Bench {
        /// Task list TOML
        #[arg(long)]
        tasks: Option<std::path::PathBuf>,
        /// Repeats per task × config
        #[arg(long)]
        runs: Option<u32>,
        /// Print the schedule and exit
        #[arg(long)]
        dry_run: bool,
        /// Per-run timeout in seconds
        #[arg(long)]
        timeout: Option<u64>,
        /// Task suite (`graph` = with/without rtok MCP)
        #[arg(long)]
        suite: Option<String>,
    },
    /// Inspect hooks, MCP servers and the proxy chain
    Doctor {
        /// Also run the instruction-file audit (T7.2)
        #[arg(long)]
        instructions: bool,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// Git worktrees of this repository: owner, state and disk cost
    Worktree {
        #[command(subcommand)]
        action: WorktreeCmd,
    },
    /// Version, effective paths, disk usage, error count and proxy status
    Info {
        /// JSON instead of the text lines
        #[arg(long)]
        json: bool,
    },
    /// Agent hosts (`rtok agents install|uninstall|list|info …`)
    #[command(visible_alias = "agent")]
    Agents {
        #[command(subcommand)]
        action: AgentCmd,
    },
    /// Deprecated spelling of `rtok agents install <host>`; still runs, still prints where to go
    #[command(hide = true)]
    Setup(SetupArgs),
    /// Execute a command, archive its raw output, print the filtered version
    Run {
        /// Sub-agent id from PreToolUse; scopes the dedup pointer (T127)
        #[arg(long, value_name = "ID")]
        agent: Option<String>,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Filter text from stdin without executing
    Filter {
        /// Read the payload from stdin (OpenCode `tool.execute.after`)
        #[arg(long)]
        stdin: bool,
        /// Command family hint (`git status`, `cargo test`, …)
        #[arg(long)]
        cmd: Option<String>,
        /// Archive stdin and print the expand trailer (same path as `run`)
        #[arg(long)]
        archive: bool,
    },
    /// Print an archived payload
    Expand {
        id: String,
        /// Inclusive 1-based range `a-b`
        #[arg(long)]
        lines: Option<String>,
        /// Regex filter (literal when it does not compile); hits print as `N:line`
        #[arg(long)]
        grep: Option<String>,
        /// With `--grep`: N lines around each hit, overlapping windows merged with `--`
        #[arg(long)]
        context: Option<u32>,
    },
    /// The archive live zone (`rtok archive rewrite` — pi `context` carrier, T70.2)
    #[cfg(feature = "archive")]
    Archive {
        #[command(subcommand)]
        action: ArchiveCmd,
    },
    /// Print shell completions for `bash`, `zsh`, `fish` or `powershell`
    Completions {
        /// Shell to complete for
        shell: clap_complete::Shell,
    },
    /// Print the man page (roff) to stdout
    Man,
    /// List plugins: id, enabled, surfaces
    Plugins {
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// The one config file
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Notes (`mem_save` / import / export)
    #[cfg(feature = "memory")]
    Memory {
        #[command(subcommand)]
        action: MemoryCmd,
    },
    /// Symbol index (`rtok graph index`)
    #[cfg(feature = "graph")]
    Graph {
        #[command(subcommand)]
        action: GraphCmd,
    },
    /// Duplicate-call verdict (`rtok guard check` — pi / OpenCode plugin path, T70.5)
    #[cfg(feature = "guard")]
    Guard {
        #[command(subcommand)]
        action: GuardCmd,
    },
    /// Deprecated spelling of `rtok web`; still runs, still prints where to go
    #[command(hide = true)]
    Dashboard {
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Keep `rtok proxy` (or `mcp` / `web`) running in the background
    Demon {
        #[command(subcommand)]
        action: DemonCmd,
    },
    /// OpenTelemetry export (`rtok otel flush | status`)
    Otel {
        #[command(subcommand)]
        action: OtelCmd,
    },
    /// rtok's own log (`rtok logs` prints, `rtok logs export` strips numbering and colour)
    Logs {
        #[command(subcommand)]
        action: Option<LogsCmd>,
        /// Override `[log] lines`
        #[arg(long, global = true)]
        lines: Option<usize>,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// The operator model as one document (D24): Markdown, HTML and PDF
    Report {
        /// Output format
        #[arg(long, value_enum, default_value = "md")]
        format: ReportFormat,
        /// Write to this path instead of stdout
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// How far back the report reads (`30d`, `24h`)
        #[arg(long)]
        since: Option<String>,
        /// Model-shaped rendering of the same document instead of `--format` (T22.4)
        #[arg(long)]
        ai: bool,
    },
}

/// `rtok archive rewrite` — the pi `context` carrier (T70.2): the same live-zone
/// rewrite the proxy runs, driven over a pi message array on stdin.
#[cfg(feature = "archive")]
#[derive(Subcommand)]
enum ArchiveCmd {
    /// Rewrite old tool results to archive pointers; message array JSON on stdin,
    /// rewritten array on stdout (input bytes echoed when nothing is eligible)
    Rewrite {
        /// Read the message array from stdin (pi `context` event)
        #[arg(long)]
        stdin: bool,
    },
}

/// Every verb takes optional services; with none they act on what is already up, falling back
/// to `[demon] services`; `status` alone shows every service. `Service` is a `ValueEnum`, so
/// clap validates the name, lists the choices in `--help` and completes them in a shell (D14).
#[derive(Subcommand)]
enum DemonCmd {
    /// Detach a supervisor that restarts the service whenever it dies
    Start { service: Vec<Service> },
    /// Ask the supervisor and its child to exit
    Stop { service: Vec<Service> },
    /// Stop, then start
    Restart { service: Vec<Service> },
    /// Stop live surfaces, replace the binary, start the same set
    #[command(hide = true)]
    Upgrade,
    /// State, pids, uptime, restarts and log path; every service when none is named
    Status {
        service: Vec<Service>,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// SIGKILL instead of SIGTERM, and drop the state file
    Kill { service: Vec<Service> },
    /// The detached half; `demon start` runs this, you do not
    #[command(hide = true)]
    Supervise { service: Service },
}

#[derive(Subcommand)]
enum OtelCmd {
    /// Post rows past the watermarks to the endpoint, once
    Flush {
        /// T143: hook-spawned only. Coalesces concurrent hook flushes to at most one
        /// running + one queued process instead of one per `Stop`/`SessionEnd` event.
        /// A manual `rtok otel flush` never passes this — it always flushes.
        #[arg(long, hide = true)]
        coalesce: bool,
    },
    /// Endpoint, watermarks, pending rows, last exporter log line
    Status {
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LogsCmd {
    /// Same selection, no numbering, no colour — for `rtok logs export > my.log`
    Export,
    /// Print the last lines, then follow: new lines arrive above the old, newest first
    Watch,
}

/// `--format` for `rtok report` (D14: a `ValueEnum`, like `demon`'s `Service`, so clap
/// validates, lists and completes it). `Pdf` landed with T22.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ReportFormat {
    Md,
    Html,
    Pdf,
}

impl ReportFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Md => "md",
            Self::Html => "html",
            Self::Pdf => "pdf",
        }
    }
}

#[cfg(feature = "memory")]
#[derive(Subcommand)]
enum MemoryCmd {
    /// Import `{kind,title,body}` JSONL; dedupe by body sha256
    Import {
        file: std::path::PathBuf,
        /// Count what would be imported and write nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Print every note but session checkpoints as the JSONL `import` reads
    Export {
        /// Only notes of this project
        #[arg(long)]
        project: Option<String>,
    },
    /// Retire a note: a tombstone — never recalled or searched, body kept (T69.1)
    Retire {
        id: i32,
        /// The replacement note id this one is superseded by
        #[arg(long)]
        superseded_by: Option<i32>,
    },
    /// Pin a note so it leads SessionStart recall (T69.1)
    Pin { id: i32 },
    /// Drop a note back to newest-first recall order (T69.1)
    Unpin { id: i32 },
    /// Save a replacement (title, body) for a note and retire the old row (T69.1)
    Revise {
        id: i32,
        /// The replacement title
        #[arg(long)]
        title: String,
        /// The replacement body
        #[arg(long)]
        body: String,
    },
    /// Write pinned-then-newest titles into a managed CLAUDE.md / AGENTS.md block (T69.6)
    Sync {
        /// CLAUDE.md or AGENTS.md
        #[arg(long, default_value = "CLAUDE.md")]
        file: std::path::PathBuf,
        /// Token budget; default `[plugins.memory] sync_tokens`
        #[arg(long)]
        budget: Option<u32>,
        /// Print the unified diff and write nothing
        #[arg(long)]
        dry_run: bool,
        /// Delete the managed block and nothing else
        #[arg(long)]
        remove: bool,
        /// Overwrite a hand-edited block
        #[arg(long)]
        force: bool,
    },
    /// Notes live/pinned/retired, recall and MCP call counts (T69.4)
    Status {
        /// Only notes of this project
        #[arg(long)]
        project: Option<String>,
        /// Window for recalls and MCP calls (`30d`, `24h`)
        #[arg(long)]
        since: Option<String>,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum WorktreeCmd {
    /// Create the worktree for a task: one location, one name, one locked owner; prints its path
    Add {
        /// Task id, e.g. `t158`; directory `<repo>-<task>`, branch `<task>[-<slug>]`
        task: String,
        /// Optional branch suffix
        slug: Option<String>,
        /// Who holds it, as `<provider> / <model>`; written into the lock reason
        #[arg(long)]
        owner: String,
    },
    /// Every worktree and orphan with its owner, state, source and build-cache bytes
    List {
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// Remove merged, clean, idle worktrees with their branches; drop records of deleted ones
    Gc {
        /// Apply; without it this is a dry run that changes nothing
        #[arg(long)]
        yes: bool,
        /// Open locks whose reason starts with this owner; every other lock is a hard stop
        #[arg(long)]
        owner: Option<String>,
        /// Keep worktrees modified within this window (`24h`, `7d`)
        #[arg(long, default_value = "24h")]
        idle: String,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// Delete idle tagged build caches (`CACHEDIR.TAG`) and keep the worktrees; dry run without `--yes`
    Clean {
        /// Only these worktrees; the one this command runs from is cleaned only when named
        paths: Vec<PathBuf>,
        /// Keep caches modified within this window (`24h`, `7d`)
        #[arg(long, default_value = "24h")]
        idle: String,
        /// Apply; without it this is a dry run that changes nothing
        #[arg(long)]
        yes: bool,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
}

#[cfg(feature = "graph")]
#[derive(Subcommand)]
enum GraphCmd {
    /// Walk a tree and insert definitions + references
    Index {
        path: Option<PathBuf>,
        /// Report what would be indexed and write no rows
        #[arg(long)]
        dry_run: bool,
    },
    /// List unreferenced private definitions (skips pub, trait impls, tests, macros)
    Dead {
        path: Option<PathBuf>,
        /// JSON rows instead of `path:line kind name` lines (T60.1, uncapped)
        #[arg(long)]
        json: bool,
    },
    /// Index health for the current or given root (T68.3)
    Status {
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Symbol impact or call paths to a target (T68.4)
    Impact {
        name: String,
        #[arg(long, default_value_t = 2)]
        depth: u32,
        #[arg(long)]
        to: Option<String>,
        path: Option<PathBuf>,
    },
    /// Tests that reach files changed in git (`git diff --name-only`)
    Affected {
        /// Diff against this ref
        #[arg(long, conflicts_with = "staged")]
        since: Option<String>,
        /// Staged files only (`git diff --cached --name-only`)
        #[arg(long)]
        staged: bool,
        /// JSON instead of `file ← via symbol` lines
        #[arg(long)]
        json: bool,
    },
}

/// `rtok guard check` — the same allow/deny `plugins::guard` returns on PreToolUse.
#[cfg(feature = "guard")]
#[derive(Subcommand)]
enum GuardCmd {
    /// Print `{"allow":true}` or `{"allow":false,"reason":…}` (fail open: bad input allows)
    Check {
        /// Host tool name (`bash`, `Read`, …)
        #[arg(long)]
        tool: String,
        /// Tool arguments as JSON
        #[arg(long, value_name = "INPUT")]
        json: String,
        /// Host session id (the cache is per session)
        #[arg(long)]
        session: Option<String>,
        /// Overlay `[hook] host`
        #[arg(long)]
        host: Option<String>,
    },
}

#[derive(Subcommand)]
enum AgentCmd {
    /// Install hooks, MCP server and proxy into a host
    #[command(alias = "setup")]
    Install(SetupArgs),
    /// Take rtok back out of a host: hooks, MCP entry, proxy variable, plugin link
    #[command(visible_alias = "remove")]
    Uninstall(RemoveArgs),
    /// Bring what rtok installed up to date: in place where it can, reinstalled where not
    Update(UpdateArgs),
    /// Every known app: kind and name, path and version, config files, rtok modules
    List {
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// One host: the same block `agents list` prints, just for that app
    Info {
        /// Host(s), comma-separated (`claude`, `cursor`, `codex`, `opencode`, `pi`, `zcode`, `kimi`, `copilot`, `aider`, `windsurf`, `zed`, `vscode`)
        host: String,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// What is running in this project: host, provider, model, tokens, start, run time
    Sessions {
        /// Also show sessions that have ended
        #[arg(long, global = true)]
        all: bool,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: Option<SessionsCmd>,
    },
    /// Junk rtok owns under its own home: log siblings and archive payloads past retention
    Junk {
        #[command(subcommand)]
        action: JunkCmd,
    },
}

#[derive(Subcommand)]
enum JunkCmd {
    /// List what `agents junk clear` would remove; `--yes` applies it
    Clear {
        /// Apply; without it this is a dry run that changes nothing
        #[arg(long)]
        yes: bool,
        /// JSON instead of the table
        #[arg(long)]
        json: bool,
    },
}

/// `rtok agents sessions watch` (T25.3): the same table, live. One screen, no keys:
///
/// the TTY repaints in place through T24.3's `watch_loop`, a pipe gets the whole
/// table again whenever it changes.
#[derive(Subcommand)]
enum SessionsCmd {
    /// Redraw the sessions table in place as sessions appear, end or spend
    Watch,
}

#[derive(clap::Args)]
struct RemoveArgs {
    /// Host(s), comma-separated (`claude`, `cursor`, `codex`, `opencode`, `pi`, `zcode`, `kimi`, `copilot`, `aider`, `windsurf`, `zed`, `vscode`)
    host: String,
    /// Print what would be removed and exit
    #[arg(long)]
    dry_run: bool,
    /// Skip closing/reopening a running desktop app around the write (T141)
    #[arg(long)]
    no_restart: bool,
    /// Also remove rtok entries you changed, without asking (T246)
    #[arg(long)]
    yes: bool,
}

#[derive(clap::Args)]
struct UpdateArgs {
    /// Host(s), comma-separated; omitted = every host rtok is installed in
    host: Option<String>,
    /// Print the planned edits and exit
    #[arg(long)]
    dry_run: bool,
    /// Only the CLI app (default is all)
    #[arg(long)]
    cli: bool,
    /// Only the desktop app (default is all)
    #[arg(long, alias = "gui")]
    desktop: bool,
    /// All variants (the default when neither `--cli` nor `--desktop` is given)
    #[arg(long)]
    all: bool,
    /// Skip closing/reopening a running desktop app around the write (T141)
    #[arg(long)]
    no_restart: bool,
}

/// One definition behind `rtok agents install` and the deprecated `rtok setup`.
#[derive(clap::Args)]
struct SetupArgs {
    /// Host(s), comma-separated (`claude`, `cursor`, `codex`, `opencode`, `pi`, `zcode`, `kimi`, `copilot`, `aider`, `windsurf`, `zed`, `vscode`)
    host: String,
    /// Print the planned edits and exit
    #[arg(long)]
    dry_run: bool,
    /// Remove rtok from the host (hooks, MCP, proxy, plugin link); prefer `rtok agents uninstall <host>`
    #[arg(long)]
    remove: bool,
    /// Enable prompt modes (`terse,yagni`)
    #[arg(long, value_delimiter = ',')]
    mode: Vec<String>,
    /// Confirm destructive `--replace`
    #[arg(long)]
    yes: bool,
    /// Remove legacy token hooks and retarget the proxy
    #[arg(long)]
    replace: bool,
    /// Register `rtok mcp` in the host MCP map
    #[arg(long)]
    mcp: bool,
    /// Set `env.ANTHROPIC_BASE_URL` to this proxy
    #[arg(long)]
    proxy: bool,
    /// Only the CLI app (`cursor`/`opencode` have CLI and desktop; default is all)
    #[arg(long)]
    cli: bool,
    /// Only the desktop app (`cursor`/`opencode` have CLI and desktop; default is all)
    #[arg(long, alias = "gui")]
    desktop: bool,
    /// All variants (the default when neither `--cli` nor `--desktop` is given)
    #[arg(long)]
    all: bool,
    /// Skip closing/reopening a running desktop app around the write (T141)
    #[arg(long)]
    no_restart: bool,
}

impl SetupArgs {
    /// `rtok agents uninstall <host>` is the install run backwards; nothing else about it differs.
    fn removing(args: RemoveArgs) -> Self {
        Self {
            host: args.host,
            dry_run: args.dry_run,
            remove: true,
            mode: Vec::new(),
            yes: args.yes,
            replace: false,
            mcp: false,
            proxy: false,
            cli: false,
            desktop: false,
            all: true,
            no_restart: args.no_restart,
        }
    }
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Write the annotated reference file to `<home>/config.toml`
    Init {
        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
        /// Print the diff it would write and exit
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the path of the config file
    Path,
    /// Print every effective key
    Show {
        /// Append `(default|user|project|env|flag)` from figment metadata
        #[arg(long)]
        sources: bool,
        /// JSON array of `{key,value,source}`
        #[arg(long)]
        json: bool,
    },
    /// Print one key's effective value
    Get {
        key: String,
        /// JSON `{key,value,source}` instead of the bare value (T228)
        #[arg(long)]
        json: bool,
    },
    /// Reject unknown keys, wrong types, and out-of-range values
    Validate {
        /// File to check (else the user config file)
        path: Option<PathBuf>,
    },
    /// Edit one key in the user file, preserving comments
    Set {
        key: String,
        value: String,
        /// Print the diff it would write and exit
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run() -> Result<()> {
    // T225: `RUST_LOG` debug log on stderr, before clap so a parse failure is logged too.
    crate::log::init_stderr();
    log::debug!(target: "rtok::cli", "argv {:?}", std::env::args_os().collect::<Vec<_>>());
    let cli = Cli::parse();
    let config_file = cli.config.clone();
    match cli.cmd {
        Cmd::Plugins { json } => {
            let config = Config::load_with(config_file.as_deref(), None)?;
            // The command renders the model's Plugins page (T15.11); the registry keeps
            // the same formatter for library users.
            let store = crate::store::Store::open(&config.core.db_path).ok();
            let pages = model::Model::new(&config, store.as_ref()).plugins();
            if json {
                print_json(&pages)?;
            } else {
                let rows: Vec<(&str, bool, Vec<&str>)> = pages
                    .iter()
                    .map(|p| (p.id, p.enabled, p.surfaces.clone()))
                    .collect();
                print!("{}", crate::render::plugins_table(&rows));
            }
        }
        Cmd::Config { action } => {
            let home = Config::home_dir();
            let user = Config::user_path(&home, config_file.as_deref());
            match action {
                ConfigCmd::Init { force, dry_run } => {
                    let (path, diff) =
                        Config::init_maybe(&home, config_file.as_deref(), force, dry_run)?;
                    println!("{}", path.display());
                    print_diff(&diff);
                }
                ConfigCmd::Path => println!("{}", user.display()),
                ConfigCmd::Show { sources, json } => {
                    let rows = model::config_entries(&home, config_file.as_deref())?;
                    show(&rows, sources, json)?;
                }
                ConfigCmd::Get { key, json } => {
                    let rows = model::config_entries(&home, config_file.as_deref())?;
                    match rows.into_iter().find(|r| r.key == key) {
                        Some(r) if json => print_json(&r)?,
                        Some(r) => println!("{}", r.value),
                        None => bail!("unknown key: {key}"),
                    }
                }
                ConfigCmd::Validate { path } => {
                    let path = path.unwrap_or(user);
                    let mut errs = validate::issues(&path)?;
                    // The filter drop-ins are deployment state, not part of the
                    // file: read them through the same file as the user layer
                    // (`--config` wins when both are given). `layers::load`
                    // creates nothing, so a read-only check stays read-only.
                    let layer = config_file.as_deref().or(Some(&path));
                    let cfg = crate::config::layers::load(&home, layer, None).unwrap_or_default();
                    errs.extend(validate::rules_issues(
                        &cfg.plugins.cmd.rules,
                        &cfg.plugins.cmd.rules_dir,
                    ));
                    if errs.is_empty() {
                        println!("ok {}", path.display());
                    } else {
                        for e in &errs {
                            eprintln!("{e}");
                        }
                        std::process::exit(1);
                    }
                }
                ConfigCmd::Set {
                    key,
                    value,
                    dry_run,
                } => {
                    let (_, diff) =
                        validate::set_with(&home, config_file.as_deref(), &key, &value, dry_run)?;
                    if dry_run {
                        // Nothing was written, so the loader would still report the old value.
                        print_diff(&diff);
                    } else {
                        let rows = model::config_entries(&home, config_file.as_deref())?;
                        match rows.into_iter().find(|r| r.key == key) {
                            Some(r) => println!("{}", r.value),
                            None => println!("{value}"),
                        }
                        print_diff(&diff);
                    }
                }
            }
        }
        Cmd::Hook { serve: true, .. } => crate::hooks::resident::serve()?,
        Cmd::Hook { event, host, .. } => {
            let cfg = Config::load_lenient(config_file.as_deref(), hook_host_flag(host));
            crate::hooks::run(&event.unwrap_or_default(), io::stdin(), io::stdout(), &cfg);
            let _ = io::stdout().flush();
        }
        Cmd::Stats {
            since,
            json,
            plugin,
            save_baseline,
            compare,
            calibrate,
            cache,
            price,
        } => {
            let cfg = Config::load_with(
                config_file.as_deref(),
                stats_flags(since, json, plugin.clone(), compare.clone(), price),
            )?;
            if calibrate {
                println!("{}", crate::tokens::calibrate_or_skip(&cfg));
                return Ok(());
            }
            // Everything `stats` prints below is a rendering of the operator model (T15.11):
            // the command owns no store of its own.
            if cache {
                let report = crate::web::model::cache_health(&cfg)?;
                print!(
                    "{}",
                    if cfg.stats.format == "json" {
                        serde_json::to_string_pretty(&report)?
                    } else {
                        crate::measure::cache::table(&report)
                    }
                );
                return Ok(());
            }
            if crate::config::CATALOGUE
                .iter()
                .any(|(id, _)| *id == cfg.stats.plugin)
            {
                print!(
                    "{}",
                    serde_json::to_string_pretty(&crate::web::model::plugin_stats(
                        &cfg,
                        &cfg.stats.plugin
                    )?)?
                );
                return Ok(());
            }
            let report = crate::web::model::stats_report(&cfg)?;
            if let Some(name) = save_baseline {
                let p = crate::measure::baseline::save(&cfg.home, &name, &report)?;
                println!("{}", p.display());
            } else if let Some(name) = compare.or_else(|| {
                let b = cfg.stats.baseline.trim();
                if b.is_empty() {
                    None
                } else {
                    Some(b.to_string())
                }
            }) {
                print!(
                    "{}",
                    crate::measure::baseline::compare(&cfg.home, &name, &report)?
                );
            } else if cfg.stats.format == "json" {
                print!("{}", report.to_json()?);
            } else {
                print!("{}", report.to_table());
            }
        }
        Cmd::Bench {
            tasks,
            runs,
            dry_run,
            timeout,
            suite,
        } => {
            let cfg = Config::load_with(
                config_file.as_deref(),
                bench_flags(tasks, runs, dry_run, timeout, suite),
            )?;
            print!("{}", crate::bench::run(&cfg)?);
        }
        Cmd::Doctor { instructions, json } => {
            let cfg = Config::load_with(config_file.as_deref(), doctor_flags(instructions))?;
            let report = model::doctor(&cfg)?;
            if json {
                print_json(&report)?;
            } else {
                print!("{}", report.to_console());
            }
        }
        Cmd::Worktree {
            action: WorktreeCmd::Add { task, slug, owner },
        } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let root = Some(cfg.worktree.root.as_path()).filter(|r| !r.as_os_str().is_empty());
            let id = (task.as_str(), slug.as_deref());
            let path = crate::worktree::add::run(&std::env::current_dir()?, root, id, &owner)?;
            println!("{}", path.display());
        }
        Cmd::Worktree {
            action: WorktreeCmd::List { json },
        } => {
            let mut rows = crate::worktree::list::rows(&std::env::current_dir()?)?;
            // T154: ownership from the sessions the hooks recorded. The listing must not
            // depend on the store — without one it prints without attribution.
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            if let Ok(store) = crate::store::Store::open(&cfg.core.db_path)
                && let Ok(seen) = store.sessions_by_cwd()
            {
                crate::worktree::list::attribute(&mut rows, &seen);
            }
            if json {
                print_json(&rows)?;
            } else {
                let now = std::time::SystemTime::now();
                print!("{}", crate::worktree::list::to_table(&rows, now));
            }
        }
        Cmd::Worktree {
            action:
                WorktreeCmd::Gc {
                    yes,
                    owner,
                    idle,
                    json,
                },
        } => {
            use crate::worktree::gc;
            use anyhow::Context as _;
            let policy = gc::Policy {
                owner: owner.as_deref(),
                idle: crate::measure::stats::parse_since(&idle).context("--idle")?,
                now: std::time::SystemTime::now(),
            };
            let outcomes = gc::run(&std::env::current_dir()?, &policy, yes)?;
            if json {
                print_json(&outcomes)?;
            } else {
                print!("{}", gc::to_table(&outcomes, yes));
            }
            if outcomes.iter().any(|o| o.failed) {
                bail!("some worktrees could not be removed");
            }
        }
        Cmd::Worktree {
            action:
                WorktreeCmd::Clean {
                    paths,
                    idle,
                    yes,
                    json,
                },
        } => {
            use crate::worktree::clean;
            use anyhow::Context as _;
            let policy = clean::Policy {
                idle: crate::measure::stats::parse_since(&idle).context("--idle")?,
                now: std::time::SystemTime::now(),
            };
            let outcomes = clean::run(&std::env::current_dir()?, &paths, &policy, yes)?;
            if json {
                print_json(&outcomes)?;
            } else {
                print!("{}", clean::to_table(&outcomes, yes, policy.now));
            }
            if outcomes.iter().any(|o| o.failed) {
                bail!("some caches could not be deleted");
            }
        }
        Cmd::Info { json } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let info = crate::info::collect(&cfg, config_file.as_deref());
            if json {
                print_json(&info)?;
            } else {
                print!("{}", info.to_text());
            }
        }
        Cmd::Proxy {
            port,
            upstream,
            mode,
            dry_run,
        } => {
            let cfg = Config::load_with(
                config_file.as_deref(),
                layers::proxy_flags(port, dry_run, upstream, mode),
            )?;
            if cfg.proxy.dry_run {
                println!("bind = {}", cfg.proxy.bind);
                println!("port = {}", cfg.proxy.port);
                println!("mode = {}", cfg.proxy.mode);
                println!("upstream = {}", cfg.proxy.upstream);
                println!("openai_upstream = {}", cfg.proxy.openai_upstream);
                println!("timeout_s = {}", cfg.proxy.timeout_s);
                println!("include_usage = {}", cfg.proxy.include_usage);
                println!("dry_run = {}", cfg.proxy.dry_run);
                return Ok(());
            }
            crate::proxy::serve_blocking(cfg)?;
        }
        Cmd::Web { host, port } => {
            let cfg = Config::load_with(config_file.as_deref(), layers::web_flags(host, port))?;
            crate::web::serve_blocking(cfg)?;
        }
        Cmd::Tui { tab, tick_secs } => {
            let cfg = Config::load_with(config_file.as_deref(), layers::tui_flags(tab, tick_secs))?;
            crate::tui::run(cfg)?;
        }
        Cmd::Dashboard { host, port } => {
            eprintln!("warning: `rtok dashboard` is deprecated; use `rtok web`");
            let cfg = Config::load_with(config_file.as_deref(), layers::web_flags(host, port))?;
            crate::web::serve_blocking(cfg)?;
        }
        Cmd::Agents { action } => match action {
            AgentCmd::Install(args) => setup_host(config_file.as_deref(), args)?,
            AgentCmd::Uninstall(args) => {
                setup_host(config_file.as_deref(), SetupArgs::removing(args))?
            }
            AgentCmd::Update(args) => update_hosts(config_file.as_deref(), args)?,
            AgentCmd::List { json } => {
                let cfg = Config::load_with(config_file.as_deref(), None)?;
                if json {
                    let rows = with_loader("listing hosts", || model::agents_list(&cfg));
                    print_json(&rows)?;
                } else {
                    let text = with_loader("listing hosts", || crate::agents::list(&cfg));
                    print!("{text}");
                }
            }
            AgentCmd::Info { host, json } => {
                let cfg = Config::load_with(config_file.as_deref(), None)?;
                let hosts = parse_hosts(&host)?;
                if json {
                    let agents = crate::agents::resolve(&hosts)?;
                    let ids: Vec<&str> = agents.iter().map(|a| a.id()).collect();
                    let rows = with_loader("reading host", || model::agents_listed(&cfg, &ids));
                    print_json(&rows)?;
                } else {
                    let text = with_loader("reading host", || crate::agents::info(&cfg, &hosts))?;
                    print!("{text}");
                }
            }
            // The command renders the model's Sessions page (T25.2): newest first, live
            // only unless `--all`. `since = 0` because the default view's window is
            // liveness itself — a `started_at` floor could hide a session that began
            // before it and is still running, which is the row this command exists for.
            AgentCmd::Sessions { all, json, action } => {
                let cfg = Config::load_with(config_file.as_deref(), None)?;
                // T25.3: live repaint through T24.3's `watch_loop` — no second loop.
                // The loop only writes characters (no raw mode, no alternate screen),
                // so Ctrl-C under the default handling leaves the terminal as found.
                if matches!(action, Some(SessionsCmd::Watch)) {
                    let mut out = io::stdout();
                    let tty = out.is_terminal();
                    let mut prev = String::new();
                    let run =
                        crate::log::watch_loop(&mut out, tty, crate::log::WATCH_POLL, move || {
                            let now = crate::log::now() as i64;
                            match model::sessions(&cfg, 0) {
                                Ok(rows) => {
                                    Some(crate::render::sessions_tick(&mut prev, &rows, all, now))
                                }
                                Err(_) => {
                                    // A transient unreadable store is a missed poll,
                                    // not a blank screen: keep showing what we had.
                                    let screen: Vec<String> =
                                        prev.lines().map(str::to_string).collect();
                                    Some(crate::log::WatchTick {
                                        fresh: Vec::new(),
                                        screen,
                                    })
                                }
                            }
                        });
                    match run {
                        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => return Ok(()),
                        other => other?,
                    }
                    return Ok(());
                }
                let rows = model::sessions(&cfg, 0)?;
                if json {
                    let rows: Vec<_> = rows
                        .into_iter()
                        .filter(|r| all || r.ended_at.is_none())
                        .collect();
                    print_json(&rows)?;
                } else {
                    print!(
                        "{}",
                        crate::render::sessions_table(&rows, all, crate::log::now() as i64)
                    );
                }
            }
            AgentCmd::Junk {
                action: JunkCmd::Clear { yes, json },
            } => {
                let cfg = Config::load_with(config_file.as_deref(), None)?;
                let outcomes = crate::agents::junk::run(&cfg, yes);
                let failed = outcomes.iter().any(|o| o.failed);
                if json {
                    print_json(&outcomes)?;
                } else {
                    print!("{}", crate::agents::junk::to_table(&outcomes, yes));
                }
                if failed {
                    bail!("some junk could not be removed");
                }
            }
        },
        Cmd::Setup(args) => {
            eprintln!(
                "warning: `rtok setup {0}` is deprecated; use `rtok agents install {0}`",
                args.host
            );
            setup_host(config_file.as_deref(), args)?;
        }
        #[cfg(feature = "cmd")]
        Cmd::Run { agent, command } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let code = crate::plugins::cmd::run::run(&cfg, &command, agent.as_deref())?;
            std::process::exit(code);
        }
        #[cfg(feature = "cmd")]
        Cmd::Filter {
            stdin: _,
            cmd,
            archive,
        } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let hint = cmd.unwrap_or_else(|| cfg.filter.cmd.clone());
            if archive {
                let mut buf = Vec::new();
                let _ = io::stdin().read_to_end(&mut buf);
                let argv: Vec<String> = hint.split_whitespace().map(str::to_string).collect();
                // No dispatch-time context reaches this surface (OpenCode's
                // `tool.execute.after`, not the Claude Code PreToolUse rewrite).
                crate::plugins::cmd::run::emit_filtered(&cfg, &argv, &buf, 0, None);
            } else {
                let mut buf = String::new();
                let _ = io::stdin().read_to_string(&mut buf);
                print!(
                    "{}",
                    crate::plugins::cmd::filter::run_with_store(&cfg, &hint, &buf)
                );
            }
        }
        Cmd::Mcp { call, json, wrap } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            if let Some(name) = call {
                if !wrap.is_empty() {
                    bail!("rtok mcp --call does not wrap a foreign server");
                }
                let raw = json.as_deref().unwrap_or("{}");
                let args: serde_json::Value = serde_json::from_str(raw)?;
                match crate::mcp::call(&cfg, &name, &args) {
                    Ok(text) => print!("{text}"),
                    Err(e) => {
                        print!("{e}");
                        std::process::exit(1);
                    }
                }
            } else if wrap.is_empty() {
                crate::mcp::run(&cfg)?;
            } else {
                #[cfg(feature = "cmd")]
                std::process::exit(crate::mcp::wrap::run(&cfg, &wrap)?);
                #[cfg(not(feature = "cmd"))]
                bail!("rtok mcp -- <server>: the wrapper needs the `cmd` feature");
            }
        }
        Cmd::Expand {
            id,
            lines,
            grep,
            context,
        } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            crate::expand::run(
                &cfg,
                &id,
                lines.as_deref(),
                grep.as_deref(),
                context.map_or(0, |n| n as usize),
            )?;
        }
        #[cfg(feature = "archive")]
        Cmd::Archive { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            match action {
                ArchiveCmd::Rewrite { stdin: _ } => {
                    let cx = crate::plugin::Runtime::open(cfg, "archive-rewrite")?;
                    let mut buf = Vec::new();
                    let _ = io::stdin().read_to_end(&mut buf);
                    let out = crate::plugins::archive::pi::rewrite_stdin(
                        &buf,
                        &crate::plugin::Ctx::new(&cx),
                    )?;
                    io::stdout().write_all(&out)?;
                }
            }
        }
        Cmd::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "rtok", &mut io::stdout());
        }
        Cmd::Man => {
            clap_mangen::Man::new(Cli::command()).render(&mut io::stdout())?;
        }
        #[cfg(feature = "memory")]
        Cmd::Memory { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            match action {
                MemoryCmd::Import { file, dry_run } => {
                    println!(
                        "{}",
                        crate::plugins::memory::import::run(&cfg, &file, dry_run)?
                    );
                }
                MemoryCmd::Export { project } => {
                    let mut out = io::stdout().lock();
                    crate::plugins::memory::export::run(&cfg, project.as_deref(), &mut out)?;
                }
                MemoryCmd::Retire { id, superseded_by } => {
                    let cx = crate::plugin::Runtime::open(cfg, "memory")?;
                    println!(
                        "{}",
                        crate::plugins::memory::mem_update(&cx, id, true, superseded_by, None)?
                    );
                }
                MemoryCmd::Pin { id } => {
                    let cx = crate::plugin::Runtime::open(cfg, "memory")?;
                    println!(
                        "{}",
                        crate::plugins::memory::mem_update(&cx, id, false, None, Some(true))?
                    );
                }
                MemoryCmd::Unpin { id } => {
                    let cx = crate::plugin::Runtime::open(cfg, "memory")?;
                    println!(
                        "{}",
                        crate::plugins::memory::mem_update(&cx, id, false, None, Some(false))?
                    );
                }
                MemoryCmd::Revise { id, title, body } => {
                    let cx = crate::plugin::Runtime::open(cfg, "memory")?;
                    let (new, retired) =
                        crate::plugins::memory::mem_revise(&cx, id, &title, &body)?;
                    match retired {
                        Some(old) => println!("revised note {old} → {new}"),
                        None => println!("updated note {new} in place"),
                    }
                }
                MemoryCmd::Sync {
                    file,
                    budget,
                    dry_run,
                    remove,
                    force,
                } => crate::plugins::memory::sync::run(&cfg, file, budget, dry_run, remove, force)?,
                MemoryCmd::Status {
                    project,
                    since,
                    json,
                } => crate::plugins::memory::status::run(
                    &cfg,
                    project.as_deref(),
                    since.as_deref(),
                    json,
                )?,
            }
        }
        #[cfg(feature = "graph")]
        Cmd::Graph { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let cx = crate::plugin::Runtime::open(cfg.clone(), "graph")?;
            match action {
                GraphCmd::Index { path, dry_run } => {
                    let root = path.unwrap_or(std::env::current_dir()?);
                    let pb = crate::render::spinner("indexing");
                    let r = crate::plugins::graph::index::run_with(
                        &crate::plugin::Ctx::new(&cx),
                        &root,
                        dry_run,
                        &pb,
                    )?;
                    println!(
                        "indexed {} files · {} rows · {} skipped · {} read · exclude {} · include {} · mapped {}",
                        r.indexed,
                        r.inserted,
                        r.skipped,
                        r.read,
                        r.exclude_skipped,
                        r.include_added,
                        r.extension_mapped,
                    );
                }
                GraphCmd::Dead { path, json } => {
                    let root = path.unwrap_or(std::env::current_dir()?);
                    let ctx = crate::plugin::Ctx::new(&cx);
                    if json {
                        let rows = crate::plugins::graph::dead_rows(&ctx, &root)?;
                        println!("{}", serde_json::to_string_pretty(&rows)?);
                    } else {
                        print!("{}", crate::plugins::graph::dead(&ctx, &root)?);
                    }
                }
                GraphCmd::Status { path, json } => {
                    crate::plugins::graph::status::run(&cfg, path, json)?;
                }
                GraphCmd::Impact {
                    name,
                    depth,
                    to,
                    path,
                } => {
                    let root = path.unwrap_or(std::env::current_dir()?);
                    let ctx = crate::plugin::Ctx::new(&cx);
                    print!(
                        "{}",
                        crate::plugins::graph::impact(&ctx, &root, &name, depth, to.as_deref(),)?
                    );
                }
                GraphCmd::Affected {
                    since,
                    staged,
                    json,
                } => {
                    let root = std::env::current_dir()?;
                    print!(
                        "{}",
                        crate::plugins::graph::affected(
                            &crate::plugin::Ctx::new(&cx),
                            &root,
                            since.as_deref(),
                            staged,
                            json,
                        )?
                    );
                }
            }
        }
        #[cfg(feature = "guard")]
        Cmd::Guard { action } => {
            let GuardCmd::Check {
                tool,
                json,
                session,
                host,
            } = action;
            let cfg = Config::load_with(config_file.as_deref(), hook_host_flag(host))?;
            let sid = session.unwrap_or_else(|| "guard-check".into());
            let cx = crate::plugin::Runtime::open(cfg, sid)?;
            println!("{}", crate::plugins::guard::check(&tool, &json, &cx));
        }
        Cmd::Demon { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let c = config_file.as_deref();
            // `status` renders the model's Demon page (T15.11); the other verbs
            // write state and stay CLI-only (D27).
            match action {
                DemonCmd::Start { service } => crate::demon::start(&cfg, c, &service)?,
                DemonCmd::Stop { service } => crate::demon::stop(&cfg, &service, false)?,
                DemonCmd::Restart { service } => crate::demon::restart(&cfg, c, &service)?,
                DemonCmd::Upgrade => crate::demon::upgrade(&cfg, c)?,
                DemonCmd::Status { service, json } => {
                    let rows = model::Model::new(&cfg, None).demon(&service)?;
                    if json {
                        print_json(&rows)?;
                    } else {
                        print!("{}", crate::demon::table(&rows));
                    }
                }
                DemonCmd::Kill { service } => crate::demon::stop(&cfg, &service, true)?,
                DemonCmd::Supervise { service } => crate::demon::supervise(&cfg, c, service)?,
            }
        }
        Cmd::Otel { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            match action {
                OtelCmd::Flush { coalesce } => {
                    let cx = crate::plugin::Runtime::open(cfg, "otel")?;
                    let rep = if coalesce {
                        crate::otel::export::flush_coalesced_blocking(&cx)
                    } else {
                        crate::otel::export::flush_blocking(&cx)
                    };
                    println!("{rep}");
                }
                OtelCmd::Status { json } => {
                    if json {
                        print_json(&model::otel_status(&cfg)?)?;
                    } else {
                        let cx = crate::plugin::Runtime::open(cfg, "otel")?;
                        print!("{}", crate::otel::export::status(&cx)?);
                    }
                }
            }
        }
        Cmd::Logs {
            action,
            lines,
            json,
        } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let tty = io::stdout().is_terminal();
            let out = match action {
                // T24.3: runs until Ctrl-C. The loop only ever writes characters — no raw
                // mode, no alternate screen — so there is no terminal state to restore.
                // T225.1: through tailspin the stream is a pipe from the loop's point of
                // view; `tspin --print` colours each row as it arrives.
                Some(LogsCmd::Watch) => {
                    if let Some(mut viewer) = crate::log::Tspin::start(&cfg, tty) {
                        let watched = crate::log::watch(&cfg, lines, viewer.sink(), false);
                        viewer.finish();
                        watched?;
                    } else {
                        crate::log::watch(&cfg, lines, &mut io::stdout(), tty)?;
                    }
                    return Ok(());
                }
                // The selection is the model's Logs page (T15.11); the numbering and colour
                // are this command's rendering of it — tailspin's when `[log] tspin` says so.
                None => {
                    let plain = model::Model::new(&cfg, None).log_lines(lines);
                    if !json
                        && !plain.is_empty()
                        && let Some(viewer) = crate::log::Tspin::start(&cfg, tty)
                    {
                        viewer.print(&crate::log::numbered(&plain));
                        return Ok(());
                    }
                    crate::log::screen(&plain)
                }
                Some(LogsCmd::Export) => model::Model::new(&cfg, None).log_lines(lines),
            };
            if json {
                print_json(&model::Model::new(&cfg, None).log_lines(lines))?;
                return Ok(());
            }
            if out.is_empty() {
                println!("no logs yet");
            } else {
                for line in out {
                    println!("{line}");
                }
            }
        }
        Cmd::Report {
            format,
            out,
            since,
            ai,
        } => {
            let cfg =
                Config::load_with(config_file.as_deref(), report_flags(format, out, since, ai))?;
            // D24: the command picks the renderer and the sink; every number was already
            // computed by the model (`src/report/` touches nothing else).
            let home = Config::home_dir();
            let doc = crate::report::document(&cfg, &home, config_file.as_deref())?;
            if cfg.report.ai {
                emit(
                    &cfg.report.out,
                    crate::report::ai::render(&doc, &cfg).as_bytes(),
                )?;
                return Ok(());
            }
            match cfg.report.format.as_str() {
                "md" => emit(
                    &cfg.report.out,
                    crate::report::markdown::render(&doc).as_bytes(),
                )?,
                "html" => emit(
                    &cfg.report.out,
                    crate::report::html::render(&doc).as_bytes(),
                )?,
                "pdf" => emit(&cfg.report.out, &crate::report::pdf::render(&doc))?,
                other => bail!("--format {other} is unknown (md, html, pdf)"),
            }
        }
        #[cfg(not(feature = "cmd"))]
        Cmd::Run { .. } => eprintln!("rtok run: not implemented"),
        #[cfg(not(feature = "cmd"))]
        Cmd::Filter { .. } => {
            let mut buf = String::new();
            let _ = io::stdin().read_to_string(&mut buf);
            print!("{buf}");
        }
    }
    Ok(())
}

fn stats_flags(
    since: Option<String>,
    json: bool,
    plugin: Option<String>,
    compare: Option<String>,
    price: bool,
) -> Option<figment::value::Dict> {
    use figment::value::{Dict, Value};
    let mut stats = Dict::new();
    if let Some(s) = since {
        stats.insert("since".into(), Value::from(s));
    }
    if json {
        stats.insert("format".into(), Value::from("json"));
    }
    if let Some(p) = plugin {
        stats.insert("plugin".into(), Value::from(p));
    }
    if let Some(c) = compare {
        stats.insert("baseline".into(), Value::from(c));
    }
    if price {
        stats.insert("price".into(), Value::from(true));
    }
    if stats.is_empty() {
        return None;
    }
    let mut flags = Dict::new();
    flags.insert("stats".into(), Value::from(stats));
    Some(flags)
}

fn bench_flags(
    tasks: Option<std::path::PathBuf>,
    runs: Option<u32>,
    dry_run: bool,
    timeout: Option<u64>,
    suite: Option<String>,
) -> Option<figment::value::Dict> {
    if tasks.is_none() && runs.is_none() && !dry_run && timeout.is_none() && suite.is_none() {
        return None;
    }
    use figment::value::{Dict, Value};
    let mut bench = Dict::new();
    if let Some(p) = tasks {
        bench.insert(
            "tasks".into(),
            Value::from(p.to_string_lossy().into_owned()),
        );
    }
    if let Some(n) = runs {
        bench.insert("runs".into(), Value::from(i64::from(n)));
    }
    if dry_run {
        bench.insert("dry_run".into(), Value::from(true));
    }
    if let Some(s) = timeout {
        bench.insert(
            "timeout_s".into(),
            Value::from(i64::try_from(s).unwrap_or(i64::MAX)),
        );
    }
    if let Some(s) = suite {
        bench.insert("suite".into(), Value::from(s));
    }
    let mut flags = Dict::new();
    flags.insert("bench".into(), Value::from(bench));
    Some(flags)
}

fn with_loader<T>(msg: &str, f: impl FnOnce() -> T) -> T {
    let pb = crate::render::loader(msg);
    let out = f();
    pb.finish_and_clear();
    out
}

fn parse_hosts(host: &str) -> Result<Vec<String>> {
    let hosts: Vec<String> = host
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if hosts.is_empty() {
        bail!("unknown host: {host}");
    }
    Ok(hosts)
}

/// The host installers, one call site for `rtok agents install|uninstall` and the deprecated
/// `rtok setup`. Unknown hosts are refused before any backup is taken.
fn setup_host(config_file: Option<&std::path::Path>, args: SetupArgs) -> Result<()> {
    let SetupArgs {
        host,
        dry_run,
        remove,
        mode,
        yes,
        replace,
        mcp,
        proxy,
        cli,
        desktop,
        all,
        no_restart,
    } = args;
    let mut cfg = Config::load_with(config_file, setup_flags(dry_run, yes, mcp, proxy, &mode))?;
    // Comma-separated hosts: `rtok agents install opencode,cursor` installs both.
    let hosts = parse_hosts(&host)?;
    let mode = if remove {
        crate::agents::Mode::Remove
    } else if replace {
        crate::agents::Mode::Replace
    } else {
        crate::agents::Mode::Install
    };
    let req = crate::agents::Request {
        hosts,
        mode,
        cli,
        desktop,
        all,
    };
    apply_hosts(&mut cfg, &req, no_restart)
}

/// `rtok agents update [host,…]` (T242.2): the named hosts, or every host rtok is installed
/// in, through the same backup/restart path as install.
fn update_hosts(config_file: Option<&std::path::Path>, args: UpdateArgs) -> Result<()> {
    let mut cfg = Config::load_with(
        config_file,
        setup_flags(args.dry_run, false, false, false, &[]),
    )?;
    let hosts = match &args.host {
        Some(h) => parse_hosts(h)?,
        None => crate::agents::installed_hosts(&cfg),
    };
    if hosts.is_empty() {
        println!(
            "nothing to update: rtok is not installed in any host (rtok agents install <host>)"
        );
        return Ok(());
    }
    let req = crate::agents::Request {
        hosts,
        mode: crate::agents::Mode::Update,
        cli: args.cli,
        desktop: args.desktop,
        all: args.all,
    };
    apply_hosts(&mut cfg, &req, args.no_restart)
}

/// Run `req` with the loader and desktop-restart handling every `agents` writer shares.
fn apply_hosts(cfg: &mut Config, req: &crate::agents::Request, no_restart: bool) -> Result<()> {
    // T81: `agents::run` may ask the plugin question mid-run, and a loader ticking on
    // stderr redraws right over a prompt — the question turns invisible and the wait for
    // its answer reads as a hang. A spinner must never share a terminal with a question,
    // so interactive runs render no loader; pipes and CI (which can never be asked) keep it.
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());
    // T141: closes a running desktop app before the write if it would change that host's
    // config, and reopens it after; CLI-only hosts just get a "restart your session" note.
    let out = if interactive {
        crate::agents::restart::run(cfg, req, no_restart)?
    } else {
        with_loader("updating host", || {
            crate::agents::restart::run(cfg, req, no_restart)
        })?
    };
    print!("{out}");
    Ok(())
}

/// A rendered diff, when there is one. An empty diff means the file was already right.
fn print_diff(diff: &str) {
    if !diff.is_empty() {
        println!("{diff}");
    }
}

fn setup_flags(
    dry_run: bool,
    yes: bool,
    mcp: bool,
    proxy: bool,
    mode: &[String],
) -> Option<figment::value::Dict> {
    if !dry_run && !yes && !mcp && !proxy && mode.is_empty() {
        return None;
    }
    use figment::value::{Dict, Value};
    let mut setup = Dict::new();
    if dry_run {
        setup.insert("dry_run".into(), Value::from(true));
    }
    if yes {
        setup.insert("yes".into(), Value::from(true));
    }
    if mcp {
        setup.insert("mcp".into(), Value::from(true));
    }
    if proxy {
        setup.insert("proxy".into(), Value::from(true));
    }
    if !mode.is_empty() {
        setup.insert(
            "modes".into(),
            Value::from(
                mode.iter()
                    .map(|s| Value::from(s.as_str()))
                    .collect::<Vec<_>>(),
            ),
        );
    }
    let mut flags = Dict::new();
    flags.insert("setup".into(), Value::from(setup));
    Some(flags)
}

pub(crate) fn hook_host_flag(host: Option<String>) -> Option<figment::value::Dict> {
    let host = host?;
    use figment::value::{Dict, Value};
    let mut hook = Dict::new();
    hook.insert("host".into(), Value::from(host));
    let mut flags = Dict::new();
    flags.insert("hook".into(), Value::from(hook));
    Some(flags)
}

fn doctor_flags(instructions: bool) -> Option<figment::value::Dict> {
    if !instructions {
        return None;
    }
    use figment::value::{Dict, Value};
    let mut doctor = Dict::new();
    doctor.insert("instructions".into(), Value::from(true));
    let mut flags = Dict::new();
    flags.insert("doctor".into(), Value::from(doctor));
    Some(flags)
}

/// The report's sink (D12: `report.out`): stdout when empty, else the file. One sink
/// for every `--format` so the renderings cannot disagree about where it went.
/// Bytes, not `&str`: the PDF renderer emits binary, and the text renderings are
/// UTF-8 either way.
fn emit(out: &std::path::Path, body: &[u8]) -> Result<()> {
    if out.as_os_str().is_empty() {
        std::io::Write::write_all(&mut std::io::stdout(), body)?;
    } else {
        std::fs::write(out, body)?;
        println!("{}", out.display());
    }
    Ok(())
}

/// The `[report]` flag layer (D12): `--format md` is the default, so it sets nothing.
fn report_flags(
    format: ReportFormat,
    out: Option<PathBuf>,
    since: Option<String>,
    ai: bool,
) -> Option<figment::value::Dict> {
    if format == ReportFormat::Md && out.is_none() && since.is_none() && !ai {
        return None;
    }
    use figment::value::{Dict, Value};
    let mut report = Dict::new();
    if format != ReportFormat::Md {
        report.insert("format".into(), Value::from(format.as_str()));
    }
    if let Some(o) = out {
        report.insert("out".into(), Value::from(o.to_string_lossy().into_owned()));
    }
    if let Some(s) = since {
        report.insert("since".into(), Value::from(s));
    }
    if ai {
        report.insert("ai".into(), Value::from(true));
    }
    let mut flags = Dict::new();
    flags.insert("report".into(), Value::from(report));
    Some(flags)
}

fn print_json(value: &(impl serde::Serialize + ?Sized)) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn show(rows: &[model::ConfigEntry], sources: bool, json: bool) -> Result<()> {
    if json {
        return print_json(rows);
    }
    for r in rows {
        if sources {
            println!("{} = {} ({})", r.key, r.value, r.source);
        } else {
            println!("{} = {}", r.key, r.value);
        }
    }
    Ok(())
}
