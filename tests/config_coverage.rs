//! Flag ↔ key coverage (plan T12.4, decision D12).
//!
//! Every clap long flag that is not in the allow-list must exist as a dotted key in
//! `config/default.toml`. Every leaf key's last segment must appear in `src/`.

use clap::{ArgAction, Command, CommandFactory};
use rtok::cli::Cli;
use rtok::config::layers;
use std::collections::HashSet;
use std::path::Path;

const ALLOW: &[&str] = &[
    "config",
    "home",
    "help",
    "version",
    "json",
    "remove",
    "replace",
    "calibrate",
    "save-baseline", // action: writes measurements/<name>.json (T1.3)
    "cache",
    "force",
    "sources", // action: annotates `config show`; not a stored setting
    "lines",   // expand slice; not a stored setting (T3.5)
    "grep",    // expand filter; not a stored setting (T3.5)
    "context", // expand grep context window; per call like --grep (T67.2)
    "stdin",   // action: rtok filter reads stdin (T10.2)
    "archive", // action: rtok filter --archive (T62.3)
    "tool",    // action: rtok guard check --tool (T70.5)
    "call",    // action: rtok mcp --call (T70.3)
    "session", // action: rtok guard check --session (T70.5)
    "all",     // action: agent sessions also lists ended sessions (T25.2); not a setting
    "cli",     // action: agent setup variant filter (T37.0); not a setting
    "desktop", // action: agents install variant filter (T37.0); not a setting
];

/// Flags that are actions on one command rather than settings, so they get no config key.
/// `--dry-run` has a key wherever the command owns a config table (`setup`, `proxy`, `bench`);
/// on `config init` / `config set` it sits beside `--force` and means "print, do not write".
const ALLOW_KEYS: &[&str] = &[
    "config.init.dry_run",
    "config.set.dry_run",
    "guard.check.host", // overlay `[hook] host` on the plugin CLI path (T70.5)
    "memory.import.dry_run",
    "graph.index.dry_run",
    "graph.impact.depth",
    "graph.impact.to",
    // `graph affected` (T68.x): which diff to read on one call, not a stored setting.
    "graph.affected.since",
    "graph.affected.staged",
    // `memory export --project` narrows one dump; the store, not a setting, decides it.
    "memory.export.project",
    // `memory retire/revise` (T69.1): one-shot lifecycle values on a single call, not settings.
    "memory.retire.superseded_by",
    "memory.revise.title",
    "memory.revise.body",
    // `memory sync` (T69.6): per-call file/budget/dry-run; `sync_tokens` is the setting.
    "memory.sync.file",
    "memory.sync.budget",
    "memory.sync.dry_run",
    // `memory status` (T69.4): one-shot filters, not settings.
    "memory.status.project",
    "memory.status.since",
    // `otel flush --coalesce` (T143): hidden, hook-spawned-only mode switch for one call,
    // not a stored setting — a manual `rtok otel flush` never passes it.
    "otel.flush.coalesce",
    // `worktree gc` (T153): deleting is decided per call — a stored `yes` or `owner`
    // would turn a dry run into a removal, or open locks, without anyone typing it.
    "worktree.gc.yes",
    "worktree.gc.owner",
    "worktree.gc.idle",
    // `worktree add --owner` (T158): who holds this one worktree; `[worktree] root` is the setting.
    "worktree.add.owner",
];

#[test]
fn config_coverage() {
    let toml_keys: HashSet<String> = layers::leaf_keys().into_iter().collect();
    let mut flags = Vec::new();
    walk(&Cli::command(), &[], &mut flags);

    for key in &flags {
        assert!(
            toml_keys.contains(key),
            "flag maps to `{key}` which is missing from config/default.toml"
        );
    }

    let src = src_blob();
    for key in &toml_keys {
        let leaf = key.rsplit('.').next().unwrap();
        if leaf.len() < 3 {
            continue;
        }
        assert!(
            src.contains(leaf),
            "config key `{key}` (leaf `{leaf}`) is never read in src/"
        );
    }
}

fn walk(cmd: &Command, path: &[&str], out: &mut Vec<String>) {
    for arg in cmd.get_arguments() {
        if matches!(
            arg.get_action(),
            ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version
        ) {
            continue;
        }
        let Some(long) = arg.get_long() else {
            continue;
        };
        if ALLOW.contains(&long) {
            continue;
        }
        let key = config_key(path, long);
        if ALLOW_KEYS.contains(&key.as_str()) {
            continue;
        }
        out.push(key);
    }
    for sub in cmd.get_subcommands() {
        let mut next = path.to_vec();
        next.push(sub.get_name());
        walk(sub, &next, out);
    }
}

fn config_key(path: &[&str], long: &str) -> String {
    let name = long.replace('-', "_");
    let name = match name.as_str() {
        "timeout" => "timeout_s",
        "compare" => "baseline",
        "mode" if matches!(path, ["proxy"]) => "mode",
        "mode" => "modes",
        _ => name.as_str(),
    };
    match path {
        // `rtok agents install|remove` keeps the `[setup]` table it had as `rtok setup`.
        ["agents", ..] => format!("setup.{name}"),
        // `rtok dashboard` is the hidden deprecated spelling of `rtok web`; one table, `[web]`.
        ["dashboard", ..] => format!("web.{name}"),
        ["run", ..] | ["filter", ..] => match name {
            "shell" => "plugins.cmd.shell".into(),
            "no_trailer" => "plugins.cmd.trailer_min_lines".into(),
            "cmd" => "filter.cmd".into(),
            other => format!("plugins.cmd.{other}"),
        },
        [] if name == "log_level" => "log.level".into(),
        [] => name.into(),
        _ => format!("{}.{name}", path.join(".")),
    }
}

fn src_blob() -> String {
    let mut out = String::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    fn walk_rs(dir: &Path, out: &mut String) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            if p.is_dir() {
                walk_rs(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                out.push_str(&std::fs::read_to_string(&p).unwrap());
            }
        }
    }
    walk_rs(&root, &mut out);
    out
}
