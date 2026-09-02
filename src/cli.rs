//! Clap tree. `tests/config_coverage.rs` walks [`Cli::command`] (plan T12.4).

use std::io::{self, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::config::layers;
use crate::config::validate;
use crate::plugins::Registry;
use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

/// Token-reduction CLI for AI coding agents. See plan.md for the task list.
#[derive(Parser)]
#[command(name = "rtok", version, about)]
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
    Hook { event: String },
    /// Serve MCP tools over stdio
    Mcp,
    /// Local API proxy for ANTHROPIC_BASE_URL
    Proxy {
        /// Override `[proxy] port`
        #[arg(long)]
        port: Option<u16>,
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
    },
    /// A/B benchmark of host configurations
    Bench,
    /// Inspect hooks, MCP servers and the proxy chain
    Doctor {
        /// Also run the instruction-file audit (T7.2)
        #[arg(long)]
        instructions: bool,
    },
    /// Install hooks, MCP server and proxy into a host
    Setup {
        /// Host (`claude`, later `cursor` / `codex` / `opencode`)
        host: String,
        /// Print the planned edits and exit
        #[arg(long)]
        dry_run: bool,
        /// Delete rtok hook entries only
        #[arg(long)]
        remove: bool,
    },
    /// Execute a command, archive its raw output, print the filtered version
    Run {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
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

impl Cmd {
    fn name(&self) -> &'static str {
        match self {
            Cmd::Hook { .. } => "hook",
            Cmd::Mcp => "mcp",
            Cmd::Proxy { .. } => "proxy",
            Cmd::Stats { .. } => "stats",
            Cmd::Bench => "bench",
            Cmd::Doctor { .. } => "doctor",
            Cmd::Setup { .. } => "setup",
            Cmd::Run { .. } => "run",
            Cmd::Expand { .. } => "expand",
            Cmd::Plugins => "plugins",
            Cmd::Config { .. } => "config",
        }
    }
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
        Cmd::Hook { event } => {
            let cfg = Config::load_lenient(config_file.as_deref(), None);
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
        } => {
            let cfg = Config::load_with(
                config_file.as_deref(),
                stats_flags(since, json, plugin.clone(), compare.clone()),
            )?;
            if calibrate {
                println!("{}", crate::tokens::calibrate_or_skip(&cfg));
                return Ok(());
            }
            let since = crate::measure::stats::parse_since(&cfg.stats.since)?;
            let report = crate::measure::stats::collect(
                &cfg.stats.transcripts_dir,
                since,
                &cfg.stats.plugin,
            )?;
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
        Cmd::Doctor { instructions } => {
            let cfg = Config::load_with(config_file.as_deref(), doctor_flags(instructions))?;
            print!("{}", crate::doctor::run(&cfg)?);
        }
        Cmd::Proxy { port, dry_run } => {
            let cfg =
                Config::load_with(config_file.as_deref(), layers::proxy_flags(port, dry_run))?;
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
            eprintln!("rtok proxy: not implemented");
        }
        Cmd::Setup {
            host,
            dry_run,
            remove,
        } => {
            let cfg = Config::load_with(config_file.as_deref(), setup_flags(dry_run))?;
            match host.as_str() {
                "claude" => println!("{}", crate::setup::claude::run(&cfg, remove)?),
                other => bail!("unknown host: {other}"),
            }
        }
        #[cfg(feature = "cmd")]
        Cmd::Run { command } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            let code = crate::plugins::cmd::run::run(&cfg, &command)?;
            std::process::exit(code);
        }
        Cmd::Expand { id, lines, grep } => {
            let cfg = Config::load_with(config_file.as_deref(), None)?;
            crate::expand::run(&cfg, &id, lines.as_deref(), grep.as_deref())?;
        }
        // Stubs exit 0 so hooks fail open until each surface lands (plan P2–P5).
        other => eprintln!("rtok {}: not implemented", other.name()),
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

fn setup_flags(dry_run: bool) -> Option<figment::value::Dict> {
    if !dry_run {
        return None;
    }
    use figment::value::{Dict, Value};
    let mut setup = Dict::new();
    setup.insert("dry_run".into(), Value::from(true));
    let mut flags = Dict::new();
    flags.insert("setup".into(), Value::from(setup));
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
