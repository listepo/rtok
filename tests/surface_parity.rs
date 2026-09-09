//! D23's gate: `rtok web` and `rtok tui` are two renderings of one operator model, so
//! a page that exists on one surface and not the other is a defect. Each surface
//! contributes its own set here and the assert fails naming the page that drifted;
//! when `rtok tui` lands (T15.1+) its tabs join the same compare.
//!
//! D27's gate (T15.12) is the second test: every command `Cli::command()` offers is
//! either a reading command that renders a page the model offers, or exempt with a
//! reason. A command in neither list fails by name, so a reading command cannot land
//! without its page and no new command can land unclassified.

use clap::{Command, CommandFactory};
use rtok::cli::Cli;
use rtok::config::Config;
use rtok::web::frame;
use rtok::web::model;

fn config() -> Config {
    let dir = std::env::temp_dir().join(format!("rtok-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Config::load_from(&dir).expect("config")
}

/// The pages the model offers, by name.
fn model_pages() -> Vec<String> {
    let mut names: Vec<String> = model::pages()
        .iter()
        .map(|(page, _)| page.to_string())
        .collect();
    names.sort();
    names
}

/// The pages the web surface serves: the keys of the frame `/ws` sends, envelope
/// dropped and each key read back to its page name. A key with no entry in the
/// model's table is its own page, so a web-only page names itself in the failure.
fn web_pages(cfg: &Config) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(&frame(cfg)).expect("frame is json");
    let mut names: Vec<String> = v
        .as_object()
        .expect("frame is an object")
        .keys()
        .filter(|key| *key != "type") // the wire envelope, not a page
        .map(|key| {
            model::pages()
                .iter()
                .find(|(_, wire_key)| *wire_key == key.as_str())
                .map_or_else(|| key.clone(), |(page, _)| page.to_string())
        })
        .collect();
    names.sort();
    names
}

#[test]
fn web_serves_exactly_the_pages_the_model_offers() {
    let cfg = config();
    assert_eq!(
        model_pages(),
        web_pages(&cfg),
        "a page exists on one surface and not the other (D23)"
    );
}

/// Reading commands and the page of the model they render: (command path, page). The
/// page must be one `model::pages()` offers — a page both surfaces carry in the frame.
/// The mapping is many-to-one: several commands may render the same page.
const COMMAND_PAGES: &[(&str, &str)] = &[
    ("plugins", "plugins"),
    // the Sessions page rides the snapshot since T25.1, so the command renders a
    // real page, not an on-demand call
    ("agent sessions", "sessions"),
];

/// The commands D27 exempts, each with its reason. Streaming commands print a stream,
/// not a state; writing commands mutate a host, a file or the store; surfaces render
/// the model rather than sit inside it; helpers answer a location or a verdict. The
/// last group is reading commands whose page is on-demand today — T15.11 moved the
/// query into the model, but the frame does not carry the page yet; each entry moves
/// to `COMMAND_PAGES` when it does.
const EXEMPT: &[(&str, &str)] = &[
    // streaming: the output is a stream, not a state (D27)
    (
        "hook",
        "reads the event JSON on stdin, writes JSON to stdout, exits",
    ),
    ("mcp", "serves MCP tools over stdio"),
    ("run", "executes a command and filters its live output"),
    ("filter", "filters stdin without executing"),
    ("expand", "prints one archived payload"),
    // surfaces: renderers of the model, not pages of it (D23)
    ("web", "the web surface itself"),
    ("dashboard", "deprecated spelling of `rtok web`"),
    ("proxy", "serves the proxy; `--dry-run` echoes its settings"),
    (
        "tui",
        "the terminal surface; its tabs are model::pages() (T15.1)",
    ),
    (
        "logs watch",
        "streams the log file as lines arrive (D27 exempts streaming)",
    ),
    // writing: a surface that shows numbers is not one that mutates a tree (D27)
    (
        "agent setup",
        "installs hooks, MCP and the proxy into a host",
    ),
    ("agent remove", "takes rtok back out of a host"),
    ("setup", "deprecated spelling of `rtok agent setup`"),
    ("config init", "writes the annotated reference file"),
    ("config set", "edits one key in the user file"),
    ("bench", "runs the A/B schedule and writes Measurement rows"),
    ("memory import", "inserts note rows"),
    ("graph index", "walks a tree and inserts symbol rows"),
    ("demon start", "starts the supervisor"),
    ("demon stop", "asks the supervisor and its child to exit"),
    ("demon restart", "stop, then start"),
    ("demon kill", "SIGKILL and drop the state file"),
    ("demon update", "restarts under the binary on disk now"),
    ("demon supervise", "the detached half of `demon start`"),
    ("otel flush", "posts rows past the watermarks"),
    // helpers: a location or a verdict, not model data
    ("config path", "prints where the config file is"),
    (
        "config validate",
        "checks a file and exits nonzero on issues",
    ),
    (
        "otel status",
        "exporter echo: endpoint, watermarks, pending rows",
    ),
    // reading, but on-demand today (T15.11); the frame does not carry the page yet
    (
        "report",
        "renders model::report_ledgers into a document (P22); no snapshot page",
    ),
    ("stats", "renders model::stats_report; no snapshot page yet"),
    (
        "doctor",
        "renders model::doctor; the Doctor tab surfaces it (T15.6)",
    ),
    (
        "config show",
        "renders model::config_entries; no snapshot page yet",
    ),
    (
        "config get",
        "renders model::config_entries; no snapshot page yet",
    ),
    (
        "logs",
        "renders Model::log_lines; the Logs tab surfaces it (T15.7)",
    ),
    (
        "logs export",
        "the same Logs selection, unnumbered and uncoloured",
    ),
    ("demon status", "renders Model::demon; no snapshot page yet"),
    ("demon list", "the same Demon page for every service"),
];

/// Every runnable command path, space-joined — the walk `config_coverage` already
/// does, minus the flags: a parent that needs a verb is navigation, not a command,
/// and clap's built-in `help` is not ours to classify.
fn walk(cmd: &Command, path: &mut Vec<String>, out: &mut Vec<String>) {
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        path.push(sub.get_name().to_string());
        if !sub.has_subcommands() || !sub.is_subcommand_required_set() {
            out.push(path.join(" "));
        }
        walk(sub, path, out);
        path.pop();
    }
}

#[test]
fn every_command_is_exempt_or_renders_a_page_of_the_model() {
    let pages: Vec<&str> = model::pages().iter().map(|(page, _)| *page).collect();
    let mut commands = Vec::new();
    walk(&Cli::command(), &mut Vec::new(), &mut commands);
    assert!(!commands.is_empty(), "the walk found no commands");

    for cmd in &commands {
        if EXEMPT.iter().any(|(path, _)| *path == cmd.as_str()) {
            continue;
        }
        let Some((_, page)) = COMMAND_PAGES.iter().find(|(path, _)| *path == cmd.as_str()) else {
            panic!(
                "unclassified command `{cmd}`: a reading command renders a page of the model \
                 (COMMAND_PAGES); everything else needs a reason in EXEMPT (D27, T15.12)"
            );
        };
        assert!(
            pages.contains(page),
            "command `{cmd}` renders page `{page}`, which model::pages() does not offer"
        );
    }

    // The lists are data, extended as commands land (`report`, `tui`, `logs watch` at
    // integration); these hold them to the tree they claim to describe.
    for (path, reason) in EXEMPT {
        assert!(!reason.is_empty(), "exempt `{path}` carries no reason");
        assert!(
            commands.iter().any(|c| c.as_str() == *path),
            "`{path}` is exempt but is not a command"
        );
    }
    for (path, _) in COMMAND_PAGES {
        assert!(
            commands.iter().any(|c| c.as_str() == *path),
            "`{path}` is mapped to a page but is not a command"
        );
        assert!(
            !EXEMPT.iter().any(|(exempt, _)| exempt == path),
            "`{path}` is both mapped to a page and exempt"
        );
    }
}
