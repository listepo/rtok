//! T44.4: the `rtok agents setup` matrix over every host and both app kinds.
//!
//! Check: per host, a second setup takes no new backup and heads its block `already
//! installed`; a second remove says `no changes`; `--dry-run` creates nothing and copies
//! nothing; `agent` and `agents` print the same; `--cli` / `--desktop` pick the block; Claude
//! Desktop installs MCP with the absolute binary under a temp home; an unknown host is refused
//! before any backup; a missing `rtok` on PATH is a warning at the top of the output.

mod common;

use common::agents::{backups, claude_desktop_config, json, raw, rtok, tmp, write_cfg};
use std::fs;
use std::path::{Path, PathBuf};

/// `(host, extra setup flags, the file whose backups we count)`; pi links a directory and
/// edits no file, so it has nothing to back up. Hosts that offer a plugin take `--yes`: an
/// unanswered offer is a step that changed nothing, so `already installed` needs the link.
fn hosts(home: &Path) -> Vec<(&'static str, Vec<&'static str>, Option<PathBuf>)> {
    vec![
        ("claude", vec![], Some(home.join(".claude/settings.json"))),
        (
            "cursor",
            vec!["--yes"],
            Some(home.join(".cursor/hooks.json")),
        ),
        ("codex", vec![], Some(home.join(".codex/config.toml"))),
        (
            "opencode",
            vec!["--yes"],
            Some(home.join(".config/opencode/opencode.json")),
        ),
        ("pi", vec!["--yes"], None),
        ("zcode", vec![], Some(home.join(".zcode/cli/config.json"))),
    ]
}

fn setup_args<'a>(host: &'a str, flags: &[&'a str]) -> Vec<&'a str> {
    let mut a = vec!["agents", "setup", host];
    a.extend_from_slice(flags);
    a
}

#[test]
fn setup_twice_takes_one_backup_and_says_already_installed() {
    let home = tmp("twice");
    let cfg = write_cfg(&home);
    for (host, flags, file) in hosts(&home) {
        if let Some(f) = &file {
            fs::write(
                f,
                if f.extension().is_some_and(|e| e == "toml") {
                    "# mine\n"
                } else {
                    "{}"
                },
            )
            .unwrap();
        }
        let first = rtok(&setup_args(host, &flags), &cfg, &home);
        assert!(!first.contains("already installed"), "{host}: {first}");
        assert!(!first.contains("did not read back"), "{host}: {first}");
        let second = rtok(&setup_args(host, &flags), &cfg, &home);
        assert!(second.contains("already installed"), "{host}: {second}");
        assert!(!second.contains("backup "), "{host}: {second}");
        if let Some(f) = &file {
            assert_eq!(
                backups(f).len(),
                1,
                "{host}: one copy, taken before the first write"
            );
            assert!(first.contains("backup "), "{host}: {first}");
            assert!(
                second.contains(&f.display().to_string()),
                "{host}: {second}"
            );
        }
        // Both runs print the same module and plugin rows.
        let rows = |s: &str| -> Vec<String> {
            s.lines()
                .filter(|l| {
                    l.starts_with("  ✓")
                        || l.starts_with("  ✗")
                        || l.starts_with("  −")
                        || l.starts_with("    ")
                })
                .map(str::to_string)
                .collect()
        };
        assert_eq!(rows(&first), rows(&second), "{host}");
        assert!(first.contains("  plugins\n"), "{host}: {first}");
    }
}

#[test]
fn remove_twice_says_no_changes_and_the_second_takes_no_backup() {
    let home = tmp("remove");
    let cfg = write_cfg(&home);
    for (host, flags, file) in hosts(&home) {
        rtok(&setup_args(host, &flags), &cfg, &home);
        let first = rtok(&["agents", "remove", host], &cfg, &home);
        assert!(!first.contains("no changes"), "{host}: {first}");
        let second = rtok(&["agents", "remove", host], &cfg, &home);
        assert!(second.contains("— no changes"), "{host}: {second}");
        if let Some(f) = &file {
            // setup created the file (no copy), the first remove copied it, the second
            // changed nothing and left no copy behind.
            assert_eq!(backups(f).len(), 1, "{host}: {:?}", backups(f));
            assert!(!second.contains("backup "), "{host}: {second}");
        }
    }
}

#[test]
fn dry_run_setup_creates_nothing_and_copies_nothing() {
    let home = tmp("dry");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    fs::write(&settings, "{}").unwrap();
    for (host, flags, file) in hosts(&home) {
        let mut args = setup_args(host, &flags);
        args.push("--dry-run");
        let out = rtok(&args, &cfg, &home);
        assert!(out.contains("— dry run, nothing written"), "{host}: {out}");
        assert!(!out.contains("backup "), "{host}: {out}");
        if let Some(f) = file.filter(|f| f != &settings) {
            assert!(!f.exists(), "{host}: {} created by a dry run", f.display());
        }
    }
    assert_eq!(fs::read_to_string(&settings).unwrap(), "{}");
    assert!(backups(&settings).is_empty());
    assert!(
        !home
            .join(".pi/agent/extensions/rtok")
            .symlink_metadata()
            .is_ok()
    );
}

#[test]
fn the_agent_alias_prints_what_agents_prints() {
    let home = tmp("alias");
    let cfg = write_cfg(&home);
    assert_eq!(
        rtok(&["agent", "list"], &cfg, &home),
        rtok(&["agents", "list"], &cfg, &home)
    );
    let a = rtok(&["agent", "setup", "codex", "--dry-run"], &cfg, &home);
    let b = rtok(&["agents", "setup", "codex", "--dry-run"], &cfg, &home);
    assert_eq!(a, b);
}

#[test]
fn cli_and_desktop_flags_pick_the_block() {
    let home = tmp("kinds");
    let cfg = write_cfg(&home);
    let cli = rtok(
        &["agents", "setup", "cursor", "--yes", "--cli"],
        &cfg,
        &home,
    );
    assert!(cli.contains("CLI: Cursor CLI"), "{cli}");
    assert!(!cli.contains("Desktop: Cursor"), "{cli}");
    let desktop = rtok(
        &["agents", "setup", "cursor", "--yes", "--desktop"],
        &cfg,
        &home,
    );
    assert!(desktop.contains("Desktop: Cursor"), "{desktop}");
    assert!(!desktop.contains("CLI: Cursor CLI"), "{desktop}");
    // Both kinds: the shared-config sibling points at the block above it.
    let both = rtok(&["agents", "setup", "cursor", "--yes"], &cfg, &home);
    assert!(
        both.contains("CLI: Cursor CLI") && both.contains("Desktop: Cursor — same files as above"),
        "{both}"
    );
    // `--desktop` never touches the CLI file of a host whose apps keep separate configs.
    let only = rtok(&["agents", "setup", "opencode", "--desktop"], &cfg, &home);
    assert!(only.contains("Desktop: OpenCode Desktop"), "{only}");
    assert!(!only.contains("CLI: OpenCode"), "{only}");
    assert!(!home.join(".config/opencode/opencode.json").exists());
}

#[test]
fn claude_desktop_installs_mcp_with_the_absolute_binary_under_a_temp_home() {
    let home = tmp("desktop");
    let cfg = write_cfg(&home);
    let file = claude_desktop_config(&home);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();

    let out = rtok(&["agents", "setup", "claude", "--desktop"], &cfg, &home);
    assert!(out.contains("Desktop: Claude Desktop"), "{out}");
    assert!(!out.contains("CLI: Claude Code"), "{out}");
    assert!(out.contains("✓ mcp     installed"), "{out}");
    assert!(
        out.contains("− hooks   not supported: Claude Desktop has no hook events"),
        "{out}"
    );
    assert!(
        !home.join(".claude/settings.json").exists(),
        "the CLI files stay untouched"
    );
    let servers = json(&file);
    let command = servers["mcpServers"]["rtok"]["command"].as_str().unwrap();
    assert!(Path::new(command).is_absolute(), "{command}");
    assert_eq!(servers["mcpServers"]["rtok"]["args"][0], "mcp");
    assert!(servers["mcpServers"]["foreign"].is_object(), "{servers}");
    assert_eq!(backups(&file).len(), 1);

    let again = rtok(&["agents", "setup", "claude", "--desktop"], &cfg, &home);
    assert!(
        again.contains("Desktop: Claude Desktop — already installed"),
        "{again}"
    );
    assert_eq!(backups(&file).len(), 1);

    rtok(&["agents", "remove", "claude"], &cfg, &home);
    let servers = json(&file);
    assert!(servers["mcpServers"]["rtok"].is_null(), "{servers}");
    assert!(servers["mcpServers"]["foreign"].is_object(), "{servers}");
}

#[test]
fn an_unknown_host_is_refused_before_any_backup() {
    let home = tmp("unknown");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    fs::write(&settings, "{}").unwrap();
    let out = raw(&["agents", "setup", "windsurf"], &cfg, &home);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("unknown host: windsurf"), "{err}");
    let out = raw(&["agents", "remove", "windsurf"], &cfg, &home);
    assert!(!out.status.success());
    assert!(
        backups(&settings).is_empty(),
        "nothing copied before the refusal"
    );
    assert_eq!(fs::read_to_string(&settings).unwrap(), "{}");
}

/// The configs spawn `rtok` by name; without it on PATH the setup still runs and says so
/// first. Windows writes the absolute exe instead, so it has nothing to warn about.
#[test]
fn a_missing_rtok_on_path_is_a_warning_at_the_top() {
    let home = tmp("path");
    let cfg = write_cfg(&home);
    let out = std::process::Command::new(common::agents::bin())
        .args([
            "--config",
            cfg.to_str().unwrap(),
            "agents",
            "setup",
            "codex",
        ])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("PATH", home.join("empty-bin"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    if cfg!(windows) {
        assert!(!stdout.contains("not on PATH"), "{stdout}");
    } else {
        assert!(
            stdout.starts_with("warning: rtok is not on PATH"),
            "{stdout}"
        );
    }
    assert!(stdout.contains("CLI: Codex"), "{stdout}");
    let with = raw(&["agents", "remove", "codex"], &cfg, &home);
    assert!(
        !String::from_utf8_lossy(&with.stdout).contains("not on PATH"),
        "remove never warns"
    );
}
