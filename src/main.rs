use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use rtok::config::Config;
use rtok::config::layers;
use rtok::plugins::Registry;

/// Token-reduction CLI for AI coding agents. See plan.md for the task list.
#[derive(Parser)]
#[command(name = "rtok", version, about)]
struct Cli {
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
    Stats,
    /// A/B benchmark of host configurations
    Bench,
    /// Inspect hooks, MCP servers and the proxy chain
    Doctor,
    /// Install hooks, MCP server and proxy into a host
    Setup,
    /// Execute a command, archive its raw output, print the filtered version
    Run {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Print an archived payload
    Expand { id: String },
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
}

impl Cmd {
    fn name(&self) -> &'static str {
        match self {
            Cmd::Hook { .. } => "hook",
            Cmd::Mcp => "mcp",
            Cmd::Proxy { .. } => "proxy",
            Cmd::Stats => "stats",
            Cmd::Bench => "bench",
            Cmd::Doctor => "doctor",
            Cmd::Setup => "setup",
            Cmd::Run { .. } => "run",
            Cmd::Expand { .. } => "expand",
            Cmd::Plugins => "plugins",
            Cmd::Config { .. } => "config",
        }
    }
}

fn main() -> Result<()> {
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
            }
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
        // Stubs exit 0 so hooks fail open until each surface lands (plan P2–P5).
        other => eprintln!("rtok {}: not implemented", other.name()),
    }
    Ok(())
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
