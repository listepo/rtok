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
        out.push(config_key(path, long));
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
        _ => name.as_str(),
    };
    match path {
        ["run", ..] | ["filter", ..] => match name {
            "shell" => "plugins.cmd.shell".into(),
            "no_trailer" => "plugins.cmd.trailer_min_lines".into(),
            "cmd" => "filter.cmd".into(),
            other => format!("plugins.cmd.{other}"),
        },
        [] if name == "log_level" => "core.log_level".into(),
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
