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
use rstest::rstest;
use rtok::cli::Cli;
use rtok::config::Config;
use rtok::doctor;
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
        .filter(|key| *key != "type" && *key != "ref_ids") // envelope / T60.4 expand map, not pages
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

/// T60.10: the TUI's page match has no placeholder fallback any more, so a model
/// page without a TUI body names itself here. The check reads the match arms out of
/// `src/tui/view.rs` — the same source-scan shape the WASM `PAGE_IDS` check uses.
#[test]
fn every_model_page_has_a_tui_body() {
    let view = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui/view.rs"),
    )
    .expect("src/tui/view.rs is readable");
    for (page, _) in model::pages() {
        assert!(
            view.contains(&format!("\"{page}\" =>")),
            "page `{page}` has no TUI body in src/tui/view.rs — the placeholder that used to \
             hide the gap is gone (T60.10), so the page must render for real"
        );
    }
}

/// T19.4: the Slint WASM UI's tab bar is `model::pages()`, not a second list. The webui
/// crate is outside the workspace (wasm toolchain), so this reads its `PAGE_IDS` from
/// source — the same pin `rtok-webui`'s own `page_ids_cover_the_d23_set` holds locally.
/// T60.3: both surfaces render session drill-down from the same model accessor.
#[test]
fn session_detail_exists_on_both_surfaces() {
    let model = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/web/model.rs"));
    let tui = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tui/view.rs"));
    let web = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/rtok-webui/src/lib.rs"
    ));
    let slint = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/rtok-webui/ui/app.slint"
    ));
    assert!(
        model.contains("pub fn session_detail"),
        "the one accessor lives on the model (D23)"
    );
    assert!(
        tui.contains("model::session_detail"),
        "the TUI renders model::session_detail"
    );
    assert!(
        web.contains("fn session_detail"),
        "the web UI rebuilds the same snapshot filter"
    );
    assert!(
        slint.contains("session.detail"),
        "the web Sessions page has a detail pane"
    );
}

/// The five sources every "both surfaces" case reads: model, TUI view and app, the web
/// crate and its Slint UI.
struct Surfaces {
    model: &'static str,
    tui: &'static str,
    app: &'static str,
    web: &'static str,
    slint: &'static str,
}

const SURFACES: Surfaces = Surfaces {
    model: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/web/model.rs")),
    tui: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tui/view.rs")),
    app: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tui/app.rs")),
    web: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/rtok-webui/src/lib.rs"
    )),
    slint: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/rtok-webui/ui/app.slint"
    )),
};

/// T60.4: both surfaces render archive expand from the same model accessor.
#[test]
fn expand_payload_exists_on_both_surfaces() {
    let Surfaces {
        model,
        tui,
        app,
        web,
        slint,
    } = SURFACES;
    let inbound = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/web/mod.rs"));
    assert!(
        model.contains("pub fn expand_payload"),
        "the one accessor lives on the model (D23)"
    );
    assert!(
        tui.contains("expand_pane") && app.contains("open_expand"),
        "the TUI opens model::expand_payload"
    );
    assert!(
        inbound.contains(r#""expand""#) && inbound.contains("expand_payload"),
        "web inbound answers expand through expand_payload"
    );
    assert!(
        web.contains("on_expand_archive") && slint.contains("expand-archive"),
        "the web Calls page has an expand button and pane"
    );
}

/// T63.1: both surfaces render the skills page from the same model accessor.
#[test]
fn skills_page_exists_on_both_surfaces() {
    let Surfaces {
        model,
        tui,
        app,
        web,
        slint,
    } = SURFACES;
    assert!(
        model.contains("pub fn skills_from"),
        "the one accessor lives on the model (D23)"
    );
    assert!(
        model.contains("(\"skills\", \"skills\")"),
        "pages() offers skills"
    );
    assert!(
        tui.contains("\"skills\" =>") && app.contains("skills_key"),
        "the TUI renders the skills page"
    );
    assert!(
        web.contains("skills_of") && slint.contains("page-id == \"skills\""),
        "the web Skills page renders the same rows"
    );
    assert!(
        slint.contains("never invoked only"),
        "web filter matches TUI n"
    );
}

/// T227: both surfaces render the Stats page from the same model accessor — `rtok
/// stats --price`'s table plus `rtok stats --cache`'s table, D27's one page for two
/// commands.
#[test]
fn stats_page_exists_on_both_surfaces() {
    let Surfaces {
        model,
        tui,
        web,
        slint,
        ..
    } = SURFACES;
    assert!(
        model.contains("(\"stats\", \"stats\")"),
        "pages() offers stats"
    );
    assert!(
        model.contains("fn stats_page_text"),
        "the one accessor lives on the model (D23)"
    );
    assert!(
        tui.contains("\"stats\" =>"),
        "the TUI renders the stats page"
    );
    assert!(
        web.contains("stats_text") && slint.contains("page-id == \"stats\""),
        "the web Stats page renders the same text"
    );
}

#[test]
fn wasm_ui_renders_every_model_page() {
    let lib = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/rtok-webui/src/lib.rs"
    ));
    let slint = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/rtok-webui/ui/app.slint"
    ));
    let model: Vec<&str> = model::pages().iter().map(|(page, _)| *page).collect();
    // PAGE_IDS array contents, in order — strip the const block between `[` and `];`.
    let start = lib
        .find("pub const PAGE_IDS")
        .and_then(|i| lib[i..].find("= &[").map(|j| i + j + "= &[".len()))
        .expect("PAGE_IDS = &[...] in rtok-webui");
    let close = lib[start..].find(']').expect("PAGE_IDS ]");
    let wasm: Vec<&str> = lib[start..start + close]
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            let s = s.strip_prefix('"')?.strip_suffix('"')?;
            Some(s)
        })
        .collect();
    assert_eq!(
        model, wasm,
        "WASM PAGE_IDS drifted from model::pages() (D23 / T19.4)"
    );
    for page in &model {
        assert!(
            slint.contains(&format!("page-id == \"{page}\"")),
            "app.slint has no body for page `{page}`"
        );
    }
}

/// Reading commands and the page of the model they render: (command path, page). The
/// page must be one `model::pages()` offers — a page both surfaces carry in the frame.
/// The mapping is many-to-one: several commands may render the same page.
const COMMAND_PAGES: &[(&str, &str)] = &[
    ("plugins", "plugins"),
    ("memory status", "plugins"),
    // the Sessions page rides the snapshot since T25.1, so the command renders a
    // real page, not an on-demand call
    ("agents sessions", "sessions"),
    // the Doctor page rides the snapshot since T15.6, so `rtok doctor` renders it
    ("doctor", "doctor"),
    // the Logs page rides the snapshot since T15.7, so `rtok logs` renders it
    ("logs", "logs"),
    // the Stats page rides the snapshot since T227, so `rtok stats` renders it
    ("stats", "stats"),
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
    // `wrap` (T51.4) joins EXEMPT when the clap command lands — not before.
    ("filter", "filters stdin without executing"),
    ("expand", "prints one archived payload"),
    (
        "guard check",
        "prints the plugins::guard allow/deny verdict (T70.5)",
    ),
    (
        "archive rewrite",
        "rewrites a pi `context` array on stdin to stdout (T70.2)",
    ),
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
    (
        "agents sessions watch",
        "repaints the Sessions page as sessions change (D27 exempts streaming)",
    ),
    // writing: a surface that shows numbers is not one that mutates a tree (D27)
    (
        "agents install",
        "installs hooks, MCP and the proxy into a host",
    ),
    ("agents uninstall", "takes rtok back out of a host"),
    (
        "agents list",
        "lists known hosts with app type, version, install state and modules",
    ),
    (
        "agents info",
        "lists one host with app type, version, install state and modules",
    ),
    ("setup", "deprecated spelling of `rtok agents install`"),
    ("config init", "writes the annotated reference file"),
    ("config set", "edits one key in the user file"),
    ("bench", "runs the A/B schedule and writes Measurement rows"),
    ("memory import", "inserts note rows"),
    (
        "memory retire",
        "tombstones a note row; the Memory page renders the notes, not the verdict (T69.1)",
    ),
    ("memory pin", "flags a note row to lead recall (T69.1)"),
    ("memory unpin", "drops the recall lead flag (T69.1)"),
    ("memory revise", "replaces and retires note rows (T69.1)"),
    (
        "memory sync",
        "writes a managed CLAUDE.md / AGENTS.md block (T69.6)",
    ),
    ("graph index", "walks a tree and inserts symbol rows"),
    ("demon start", "starts the supervisor"),
    ("demon stop", "asks the supervisor and its child to exit"),
    ("demon restart", "stop, then start"),
    ("demon kill", "SIGKILL and drop the state file"),
    ("demon supervise", "the detached half of `demon start`"),
    (
        "demon upgrade",
        "stops live surfaces, replaces the binary, starts the same set",
    ),
    ("otel flush", "posts rows past the watermarks"),
    (
        "worktree gc",
        "removes finished git worktrees and their merged branches (T153)",
    ),
    (
        "worktree add",
        "creates a locked git worktree and prints its path (T158)",
    ),
    (
        "worktree clean",
        "deletes tagged build caches on the checkout's file system, not the store (T152)",
    ),
    // helpers: a location or a verdict, not model data
    ("config path", "prints where the config file is"),
    (
        "info",
        "prints version, paths, disk usage, error count and proxy status",
    ),
    (
        "config validate",
        "checks a file and exits nonzero on issues",
    ),
    (
        "completions",
        "prints shell completions generated from the clap tree (T53.2)",
    ),
    (
        "man",
        "prints the man page generated from the clap tree (T53.2)",
    ),
    (
        "memory export",
        "dumps notes as the portable JSONL `memory import` reads; the Memory page is where they render (T66.2)",
    ),
    (
        "otel status",
        "exporter echo: endpoint, watermarks, pending rows",
    ),
    (
        "worktree list",
        "reads git and the checkout's file system, not the store: no model data (T151)",
    ),
    // reading, but on-demand today (T15.11); the frame does not carry the page yet
    (
        "report",
        "renders model::report_ledgers into a document (P22); no snapshot page",
    ),
    (
        "graph dead",
        "reads the symbol index on demand; no snapshot page yet",
    ),
    (
        "graph status",
        "index health on demand (T68.3); no snapshot page yet",
    ),
    (
        "graph impact",
        "symbol impact on demand (T68.4); no snapshot page yet",
    ),
    (
        "graph affected",
        "reads the symbol index on demand; no snapshot page yet",
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
        "logs export",
        "the same Logs selection, unnumbered and uncoloured",
    ),
    ("demon status", "renders Model::demon; no snapshot page yet"),
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

/// Table-printing readers must accept `--json` and serialize the model page (T60.1).
const JSON_READERS: &[&str] = &[
    "stats",
    "info",
    "config show",
    "doctor",
    "plugins",
    "agents list",
    "agents info",
    "agents sessions",
    "logs",
    "demon status",
    "otel status",
    "memory status",
    "worktree list",
];

fn command_at<'a>(root: &'a Command, path: &str) -> &'a Command {
    let mut cur = root;
    for part in path.split(' ') {
        cur = cur
            .find_subcommand(part)
            .unwrap_or_else(|| panic!("no command `{path}`"));
    }
    cur
}

#[test]
fn reading_commands_accept_json() {
    let root = Cli::command();
    for path in JSON_READERS {
        let ok = command_at(&root, path)
            .get_arguments()
            .any(|a| a.get_long() == Some("json"));
        assert!(ok, "reading command `{path}` has no --json (T60.1)");
    }
    for (path, _) in COMMAND_PAGES {
        assert!(
            JSON_READERS.contains(path),
            "reading command `{path}` renders a model page but is not gated for --json (T60.1)"
        );
    }
}

/// Emission order inside the instructions tail — `doctor::Report::to_text` and
/// `snapshot::doctor_of` must share it (T36.15).
const INSTRUCTION_TAIL: &[&str] = &["instructions", "tokens", "duplicate"];

fn tail_after<'a>(src: &'a str, marker: &str) -> &'a str {
    src.split(marker).nth(1).expect(marker)
}

fn markers_in_order(haystack: &str, markers: &[&str]) -> bool {
    let mut pos = 0;
    for m in markers {
        let Some(i) = haystack[pos..].find(m) else {
            return false;
        };
        pos += i + m.len();
    }
    true
}

/// T36.15: the WASM Doctor tab mirrors `doctor::Report::to_text` for the instruction
/// audit — source-pinned here because `rtok-webui` is outside the workspace.
#[rstest]
fn web_doctor_instruction_audit_matches_cli_order() {
    let lib = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/rtok-webui/src/lib.rs"
    ));
    let doctor_rs = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doctor.rs"));
    let wasm_tail = tail_after(
        lib.split("fn doctor_of").nth(1).expect("doctor_of"),
        "autoCompactWindow",
    );
    let cli_tail = tail_after(
        doctor_rs.split("pub fn to_text").nth(1).expect("to_text"),
        "autoCompactWindow",
    );
    for (name, tail) in [("doctor_of", wasm_tail), ("Report::to_text", cli_tail)] {
        assert!(
            markers_in_order(tail, INSTRUCTION_TAIL),
            "{name} instructions tail markers drifted"
        );
    }
    assert!(
        wasm_tail.contains(r"  {} {} tokens {}{}\n"),
        "doctor_of row format matches Report::to_text"
    );
    assert!(
        wasm_tail.contains("duplicate `{sent}` in {}"),
        "doctor_of duplicate format matches Report::to_text"
    );

    let dir = std::env::temp_dir().join(format!("rtok-parity-doctor-{}", std::process::id()));
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
    let mut cfg = Config::load_from(&dir).expect("config");
    cfg.doctor.settings_path = dir.join("settings.json");
    cfg.doctor.claude_json = dir.join("claude.json");
    cfg.doctor.instructions = true;

    let cli = doctor::page(&cfg).expect("doctor").to_text();
    assert!(cli.contains("instructions\n"));
    let compact = cli.find("autoCompactWindow").expect("compact line");
    let instr = cli.find("instructions\n").expect("instructions section");
    assert!(
        instr > compact,
        "`rtok doctor` prints instructions after autoCompactWindow"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
