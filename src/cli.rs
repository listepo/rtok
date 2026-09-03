//! Clap tree. `tests/config_coverage.rs` walks [`Cli::command`] (plan T12.4).

use std::io::{self, Read, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::config::layers;
use crate::config::validate;
use crate::plugins::Registry;
use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

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
    /// Install hooks, MCP server and proxy into a host
    Setup {
        /// Host (`claude`, `cursor`, `codex`, `opencode`)
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
    },
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
}

#[cfg(feature = "memory")]
#[derive(Subcommand)]
enum MemoryCmd {
    /// Import `{kind,title,body}` JSONL; dedupe by body sha256
    Import { file: std::path::PathBuf },
}

#[cfg(feature = "graph")]
#[derive(Subcommand)]
enum GraphCmd {
    /// Walk a tree and insert definitions + references
    Index { path: Option<PathBuf> },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Write the annotated reference file to `<home>/config.toml`
    Init {
        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
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
    Set { key: String, value: String },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_file = cli.config.clone();
    match cli.cmd {
        Cmd::Plugins => {
            let config = Config::load_with(config_file.as_deref(), None)?;
            print!("{}", Registry::new(&config).table());
        }
        Cmd::Config { action } => {
            let home = Config::home_dir();
            match action {
                ConfigCmd::Init { force } => {
                    println!("{}", Config::init(&home, force)?.display());
                }
                ConfigCmd::Path => println!("{}", Config::path_for(&home).display()),
                ConfigCmd::Show { sources, json } => {
                    let fig = load_figment(config_file.as_deref())?;
                    show(&fig, sources, json)?;
                }
                ConfigCmd::Get { key } => {
                    let fig = load_figment(config_file.as_deref())?;
                    match layers::entries(&fig).into_iter().find(|(k, ..)| k == &key) {
                        Some((_, value, _)) => println!("{value}"),
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
                ConfigCmd::Set { key, value } => {
                    validate::set(&home, &key, &value)?;
                    let fig = load_figment(config_file.as_deref())?;
                    match layers::entries(&fig).into_iter().find(|(k, ..)| k == &key) {
                        Some((_, v, _)) => println!("{v}"),
                        None => println!("{value}"),
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
            if cache {
                print!("{}", crate::measure::cache::run(&cfg)?);
                return Ok(());
            }
            if crate::config::CATALOGUE
                .iter()
                .any(|(id, _)| *id == cfg.stats.plugin)
            {
                print!(
                    "{}",
                    crate::measure::stats::plugin_json(&cfg, &cfg.stats.plugin)?
                );
                return Ok(());
            }
            let since = crate::measure::stats::parse_since(&cfg.stats.since)?;
            let mut report = crate::measure::stats::collect(
                &cfg.stats.transcripts_dir,
                since,
                &cfg.stats.plugin,
                crate::measure::stats::Replay::from_cfg(&cfg),
            )?;
            if let Ok(store) = crate::store::Store::open(&cfg.core.db_path) {
                let _ = crate::measure::stats::attach_api(&mut report, &store);
            }
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
            print!("{}", crate::doctor::run(&cfg)?);
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
        Cmd::Setup {
            host,
            dry_run,
            remove,
            mode,
            yes,
            replace,
            mcp,
            proxy,
        } => {
            let cfg = Config::load_with(
                config_file.as_deref(),
                setup_flags(dry_run, yes, mcp, proxy, &mode),
            )?;
            match host.as_str() {
                "claude" if replace => println!("{}", crate::setup::migrate::run(&cfg)?),
                "claude" => {
                    let hooks = crate::setup::claude::run(&cfg, remove)?;
                    let mut lines = vec![hooks];
                    if cfg.setup.mcp && !remove {
                        lines.push(crate::setup::claude::register_mcp(&cfg)?);
                    }
                    if cfg.setup.proxy && !remove {
                        lines.push(crate::proxy::cli::register_proxy(&cfg)?);
                    }
                    if lines.iter().all(|s| s == "no changes") {
                        println!("no changes");
                    } else {
                        println!("{}", lines.join("\n"));
                    }
                }
                "cursor" => {
                    let hooks = crate::setup::cursor::run(&cfg, remove)?;
                    if cfg.setup.mcp && !remove {
                        let mcp = crate::setup::cursor::register_mcp(&cfg)?;
                        if hooks == "no changes" && mcp == "no changes" {
                            println!("no changes");
                        } else {
                            println!("{hooks}\n{mcp}");
                        }
                    } else {
                        println!("{hooks}");
                    }
                }
                // Codex has no hooks; MCP plus optional proxy (T11.5) is the install.
                "codex" => {
                    let mut lines = vec![crate::setup::codex::run(&cfg, remove)?];
                    if cfg.setup.proxy {
                        lines.push(crate::setup::codex::register_proxy(&cfg, remove)?);
                    }
                    if lines.iter().all(|s| s == "no changes") {
                        println!("no changes");
                    } else {
                        println!("{}", lines.join("\n"));
                    }
                }
                "opencode" => println!("{}", crate::setup::opencode::run(&cfg, remove)?),
                other => bail!("unknown host: {other}"),
            }
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
                MemoryCmd::Import { file } => {
                    println!("{}", crate::plugins::memory::import::run(&cfg, &file)?);
                }
            }
        }
        #[cfg(feature = "graph")]
        Cmd::Graph { action } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let GraphCmd::Index { path } = action;
            let cx = crate::plugin::Ctx::open(cfg, "graph")?;
            let root = path.unwrap_or(std::env::current_dir()?);
            let r = crate::plugins::graph::index::run(&cx, &root)?;
            println!(
                "indexed {} files · {} rows · {} skipped · {} read",
                r.indexed, r.inserted, r.skipped, r.read
            );
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

fn load_figment(config_file: Option<&std::path::Path>) -> Result<figment::Figment> {
    let home = Config::home_dir();
    Config::ensure_user_file(&home, config_file)?;
    Ok(layers::figment(&home, config_file, None))
}

fn show(fig: &figment::Figment, sources: bool, json: bool) -> Result<()> {
    let rows = layers::entries(fig);
    if json {
        let v: Vec<_> = rows
            .iter()
            .map(|(k, v, s)| serde_json::json!({ "key": k, "value": v, "source": s }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    for (k, v, s) in rows {
        if sources {
            println!("{k} = {v} ({s})");
        } else {
            println!("{k} = {v}");
        }
    }
    Ok(())
}
