//! Clap tree. `tests/config_coverage.rs` walks [`Cli::command`] (plan T12.4).

use std::io::{self, Read, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::config::layers;
use crate::config::validate;
use crate::demon::Service;
use crate::web::model;
use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};

/// `0.1.0 (1a2b3c4d5)` — the sha comes from `build.rs` (T10.4).
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("RTOK_GIT_SHA"), ")");

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
        event: String,
        /// Overlay `[hook] host` (`claude` | `cursor`)
        #[arg(long)]
        host: Option<String>,
    },
    /// Serve MCP tools over stdio
    Mcp,
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
    },
    /// Inspect hooks, MCP servers and the proxy chain
    Doctor {
        /// Also run the instruction-file audit (T7.2)
        #[arg(long)]
        instructions: bool,
    },
    /// Agent hosts (`rtok agent setup claude|cursor|codex|opencode|pi`)
    Agent {
        #[command(subcommand)]
        action: AgentCmd,
    },
    /// Deprecated spelling of `rtok agent setup <host>`; still runs, still prints where to go
    #[command(hide = true)]
    Setup(SetupArgs),
    /// Execute a command, archive its raw output, print the filtered version
    Run {
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
    },
    /// Print an archived payload
    Expand {
        id: String,
        /// Inclusive 1-based range `a-b`
        #[arg(long)]
        lines: Option<String>,
        /// Substring filter
        #[arg(long)]
        grep: Option<String>,
    },
    /// List plugins: id, enabled, surfaces
    Plugins,
    /// The one config file
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Notes (`mem_save` / import)
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
    },
}

/// Every verb takes optional services; with none they act on what is already up, falling back
/// to `[demon] services`. `Service` is a `ValueEnum`, so clap validates the name, lists the
/// choices in `--help` and completes them in a shell (D14).
#[derive(Subcommand)]
enum DemonCmd {
    /// Detach a supervisor that restarts the service whenever it dies
    Start { service: Vec<Service> },
    /// Ask the supervisor and its child to exit
    Stop { service: Vec<Service> },
    /// Stop, then start
    Restart { service: Vec<Service> },
    /// State, pids, uptime, restarts and log path
    Status { service: Vec<Service> },
    /// `status` for every service, running or not
    List,
    /// SIGKILL instead of SIGTERM, and drop the state file
    Kill { service: Vec<Service> },
    /// Restart under the binary on disk now (after an upgrade replaced it)
    Update { service: Vec<Service> },
    /// The detached half; `demon start` runs this, you do not
    #[command(hide = true)]
    Supervise { service: Service },
}

#[derive(Subcommand)]
enum OtelCmd {
    /// Post rows past the watermarks to the endpoint, once
    Flush,
    /// Endpoint, watermarks, pending rows, last exporter log line
    Status,
}

#[derive(Subcommand)]
enum LogsCmd {
    /// Same selection, no numbering, no colour — for `rtok logs export > my.log`
    Export,
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
}

#[derive(Subcommand)]
enum AgentCmd {
    /// Install hooks, MCP server and proxy into a host
    Setup(SetupArgs),
    /// Take rtok back out of a host: hooks, MCP entry, proxy variable, plugin link
    Remove(RemoveArgs),
}

#[derive(clap::Args)]
struct RemoveArgs {
    /// Host (`claude`, `cursor`, `codex`, `opencode`, `pi`)
    host: String,
    /// Print what would be removed and exit
    #[arg(long)]
    dry_run: bool,
}

/// One definition behind `rtok agent setup` and the deprecated `rtok setup`.
#[derive(clap::Args)]
struct SetupArgs {
    /// Host (`claude`, `cursor`, `codex`, `opencode`, `pi`)
    host: String,
    /// Print the planned edits and exit
    #[arg(long)]
    dry_run: bool,
    /// Delete rtok hook entries only
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
}

impl SetupArgs {
    /// `rtok agent remove <host>` is the install run backwards; nothing else about it differs.
    fn removing(args: RemoveArgs) -> Self {
        Self {
            host: args.host,
            dry_run: args.dry_run,
            remove: true,
            mode: Vec::new(),
            yes: false,
            replace: false,
            mcp: false,
            proxy: false,
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
    Get { key: String },
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
    let cli = Cli::parse();
    let config_file = cli.config.clone();
    match cli.cmd {
        Cmd::Plugins => {
            let config = Config::load_with(config_file.as_deref(), None)?;
            // The command renders the model's Plugins page (T15.11); the registry keeps
            // the same formatter for library users.
            let rows: Vec<(&str, bool, Vec<&str>)> = crate::web::model::Model::new(&config, None)
                .plugins()
                .into_iter()
                .map(|p| (p.id, p.enabled, p.surfaces))
                .collect();
            print!("{}", crate::render::plugins_table(&rows));
        }
        Cmd::Config { action } => {
            let home = Config::home_dir();
            match action {
                ConfigCmd::Init { force, dry_run } => {
                    let (path, diff) = Config::init_maybe(&home, force, dry_run)?;
                    println!("{}", path.display());
                    print_diff(&diff);
                }
                ConfigCmd::Path => println!("{}", Config::path_for(&home).display()),
                ConfigCmd::Show { sources, json } => {
                    let rows = model::config_entries(&home, config_file.as_deref())?;
                    show(&rows, sources, json)?;
                }
                ConfigCmd::Get { key } => {
                    let rows = model::config_entries(&home, config_file.as_deref())?;
                    match rows.into_iter().find(|r| r.key == key) {
                        Some(r) => println!("{}", r.value),
                        None => bail!("unknown key: {key}"),
                    }
                }
                ConfigCmd::Validate { path } => {
                    let path = path.unwrap_or_else(|| Config::path_for(&home));
                    let errs = validate::issues(&path)?;
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
                    let (_, diff) = validate::set(&home, &key, &value, dry_run)?;
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
        Cmd::Hook { event, host } => {
            let cfg = Config::load_lenient(config_file.as_deref(), hook_host_flag(host));
            crate::hooks::run(&event, io::stdin(), io::stdout(), &cfg);
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
        } => {
            let cfg = Config::load_with(
                config_file.as_deref(),
                stats_flags(since, json, plugin.clone(), compare.clone()),
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
        } => {
            let cfg = Config::load_with(
                config_file.as_deref(),
                bench_flags(tasks, runs, dry_run, timeout),
            )?;
            print!("{}", crate::bench::run(&cfg)?);
        }
        Cmd::Doctor { instructions } => {
            let cfg = Config::load_with(config_file.as_deref(), doctor_flags(instructions))?;
            print!("{}", model::doctor(&cfg)?.to_text());
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
        Cmd::Dashboard { host, port } => {
            eprintln!("warning: `rtok dashboard` is deprecated; use `rtok web`");
            let cfg = Config::load_with(config_file.as_deref(), layers::web_flags(host, port))?;
            crate::web::serve_blocking(cfg)?;
        }
        Cmd::Agent { action } => match action {
            AgentCmd::Setup(args) => setup_host(config_file.as_deref(), args)?,
            AgentCmd::Remove(args) => {
                setup_host(config_file.as_deref(), SetupArgs::removing(args))?
            }
        },
        Cmd::Setup(args) => {
            eprintln!(
                "warning: `rtok setup {0}` is deprecated; use `rtok agent setup {0}`",
                args.host
            );
            setup_host(config_file.as_deref(), args)?;
        }
        #[cfg(feature = "cmd")]
        Cmd::Run { command } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let code = crate::plugins::cmd::run::run(&cfg, &command)?;
            std::process::exit(code);
        }
        #[cfg(feature = "cmd")]
        Cmd::Filter { stdin: _, cmd } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let hint = cmd.unwrap_or(cfg.filter.cmd);
            let mut buf = String::new();
            let _ = io::stdin().read_to_string(&mut buf);
            print!("{}", crate::plugins::cmd::filter::run(&hint, &buf));
        }
        Cmd::Mcp => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            crate::mcp::run(&cfg)?;
        }
        Cmd::Expand { id, lines, grep } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            crate::expand::run(&cfg, &id, lines.as_deref(), grep.as_deref())?;
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
            }
        }
        #[cfg(feature = "graph")]
        Cmd::Graph { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let GraphCmd::Index { path, dry_run } = action;
            let cx = crate::plugin::Runtime::open(cfg, "graph")?;
            let root = path.unwrap_or(std::env::current_dir()?);
            let pb = crate::render::spinner("indexing");
            let r = crate::plugins::graph::index::run_with(
                &crate::plugin::Ctx::new(&cx),
                &root,
                dry_run,
                &pb,
            )?;
            println!(
                "indexed {} files · {} rows · {} skipped · {} read",
                r.indexed, r.inserted, r.skipped, r.read
            );
        }
        Cmd::Demon { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let c = config_file.as_deref();
            // `status` and `list` render the model's Demon page (T15.11); the other verbs
            // write state and stay CLI-only (D27).
            match action {
                DemonCmd::Start { service } => crate::demon::start(&cfg, c, &service)?,
                DemonCmd::Stop { service } => crate::demon::stop(&cfg, &service, false)?,
                DemonCmd::Restart { service } => crate::demon::restart(&cfg, c, &service)?,
                DemonCmd::Status { service } => {
                    let rows = model::Model::new(&cfg, None).demon(&service)?;
                    print!("{}", crate::demon::table(&rows));
                }
                DemonCmd::List => {
                    let rows = model::Model::new(&cfg, None).demon(Service::value_variants())?;
                    print!("{}", crate::demon::table(&rows));
                }
                DemonCmd::Kill { service } => crate::demon::stop(&cfg, &service, true)?,
                DemonCmd::Update { service } => crate::demon::update(&cfg, c, &service)?,
                DemonCmd::Supervise { service } => crate::demon::supervise(&cfg, c, service)?,
            }
        }
        Cmd::Otel { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let cx = crate::plugin::Runtime::open(cfg, "otel")?;
            match action {
                OtelCmd::Flush => println!("{}", crate::otel::export::flush_blocking(&cx)),
                OtelCmd::Status => print!("{}", crate::otel::export::status(&cx)?),
            }
        }
        Cmd::Logs { action, lines } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            // The selection is the model's Logs page (T15.11); the numbering and colour
            // are this command's rendering of it.
            let out = match action {
                None => crate::log::screen(&model::Model::new(&cfg, None).log_lines(lines)),
                Some(LogsCmd::Export) => model::Model::new(&cfg, None).log_lines(lines),
            };
            if out.is_empty() {
                println!("no logs yet");
            } else {
                for line in out {
                    println!("{line}");
                }
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
) -> Option<figment::value::Dict> {
    if tasks.is_none() && runs.is_none() && !dry_run && timeout.is_none() {
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
    let mut flags = Dict::new();
    flags.insert("bench".into(), Value::from(bench));
    Some(flags)
}

/// The host installers, one call site for `rtok agent setup` and the deprecated `rtok setup`.
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
    } = args;
    let mut cfg = Config::load_with(config_file, setup_flags(dry_run, yes, mcp, proxy, &mode))?;
    // The copy is taken up front, before any installer runs, so one `.bak-<ts>` per file holds
    // the host exactly as it was — not as it was midway through a multi-file edit. Taking it
    // here also means the installers must not take a second one of their own.
    if !cfg.setup.dry_run && cfg.setup.backup {
        for path in crate::setup::host_files(&cfg, &host) {
            if let Some(bak) = rtok_agent_sdk::backup(&path)? {
                println!("backup {}", bak.display());
            }
        }
        cfg.setup.backup = false;
    }
    match host.as_str() {
        "claude" if replace => println!("{}", crate::setup::migrate::run(&cfg)?),
        "claude" => {
            let mut lines = vec![crate::setup::claude::run(&cfg, remove)?];
            if remove {
                lines.push(crate::setup::claude::unregister_mcp(&cfg)?);
                lines.push(crate::proxy::cli::unregister_proxy(&cfg)?);
            } else {
                if cfg.setup.mcp {
                    lines.push(crate::setup::claude::register_mcp(&cfg)?);
                }
                if cfg.setup.proxy {
                    lines.push(crate::proxy::cli::register_proxy(&cfg)?);
                }
            }
            print_lines(&lines);
        }
        "cursor" => {
            let hooks = crate::setup::cursor::run(&cfg, remove)?;
            let plugin = crate::setup::cursor::offer_plugin(&cfg, remove)?;
            let mut lines = vec![hooks, plugin];
            if remove {
                lines.push(crate::setup::cursor::unregister_mcp(&cfg)?);
            } else if cfg.setup.mcp && !crate::setup::cursor::plugin_is_mcp(&cfg, remove) {
                lines.push(crate::setup::cursor::register_mcp(&cfg)?);
            }
            print_lines(&lines);
        }
        // Codex has no hooks; MCP plus optional proxy (T11.5) is the install.
        "codex" => {
            let mut lines = vec![crate::setup::codex::run(&cfg, remove)?];
            // On the way out the provider block goes whether or not `--proxy` asked for it.
            if remove || cfg.setup.proxy {
                lines.push(crate::setup::codex::register_proxy(&cfg, remove)?);
            }
            print_lines(&lines);
        }
        "opencode" => println!("{}", crate::setup::opencode::run(&cfg, remove)?),
        // pi has no hooks and no MCP (its philosophy); the extension owns bash (T10.6).
        "pi" => println!("{}", crate::setup::pi::offer_plugin(&cfg, remove)?),
        other => bail!("unknown host: {other}"),
    }
    Ok(())
}

/// An installer that changed nothing reports it once, not once per step. The lines it does
/// report are already `+`/`-` shaped, so they take the same colours as a diff (T12.6).
fn print_lines(lines: &[String]) {
    if lines.iter().all(|s| s == "no changes") {
        println!("no changes");
    } else {
        println!("{}", crate::render::paint(&lines.join("\n")));
    }
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

fn hook_host_flag(host: Option<String>) -> Option<figment::value::Dict> {
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

fn show(rows: &[model::ConfigEntry], sources: bool, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rows)?);
        return Ok(());
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
