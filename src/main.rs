use anyhow::Result;
use clap::{Parser, Subcommand};
use rtok::config::Config;
use rtok::plugins::Registry;

/// Token-reduction CLI for AI coding agents. See plan.md for the task list.
#[derive(Parser)]
#[command(name = "rtok", version, about)]
struct Cli {
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
    Proxy,
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
    /// List plugins: id, kind, enabled, surfaces
    Plugins,
}

impl Cmd {
    fn name(&self) -> &'static str {
        match self {
            Cmd::Hook { .. } => "hook",
            Cmd::Mcp => "mcp",
            Cmd::Proxy => "proxy",
            Cmd::Stats => "stats",
            Cmd::Bench => "bench",
            Cmd::Doctor => "doctor",
            Cmd::Setup => "setup",
            Cmd::Run { .. } => "run",
            Cmd::Expand { .. } => "expand",
            Cmd::Plugins => "plugins",
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Plugins => {
            let config = Config::load()?;
            print!("{}", Registry::new(&config).table());
        }
        // Stubs exit 0 so hooks fail open until each surface lands (plan P2–P5).
        other => eprintln!("rtok {}: not implemented", other.name()),
    }
    Ok(())
}
