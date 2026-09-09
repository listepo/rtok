//! T10.9: `rtok agent remove <host>` takes back everything `rtok agent setup <host>` wrote,
//! and both commands copy the host's config files before they touch anything.
//!
//! Check: per host, seed a foreign entry, install, remove, and assert rtok is gone while the
//! foreign entry stays; assert the `.bak-<ts>` copy holds the file as it was before the command;
//! assert a second remove is `no changes` and that `--dry-run` writes nothing at all.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtok-t109-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// One config pointing every host at files inside `home`.
fn write_cfg(home: &Path) -> PathBuf {
    for sub in [
        ".claude",
        ".cursor",
        ".codex",
        ".config/opencode",
        ".pi/agent",
    ] {
        fs::create_dir_all(home.join(sub)).unwrap();
    }
    let cfg = home.join("config.toml");
    let h = home.display();
    fs::write(
        &cfg,
        format!(
            "[doctor]\nclaude_json = \"{h}/.claude.json\"\n\
             [setup.claude]\nsettings_path = \"{h}/.claude/settings.json\"\n\
             [setup.cursor]\nhooks_path = \"{h}/.cursor/hooks.json\"\n\
             [setup.codex]\nconfig_path = \"{h}/.codex/config.toml\"\n\
             [setup.opencode]\nconfig_path = \"{h}/.config/opencode/opencode.json\"\n\
             [setup.pi]\nextensions_path = \"{h}/.pi/agent/extensions\"\n"
        ),
    )
    .unwrap();
    cfg
}

fn rtok(args: &[&str], cfg: &Path, home: &Path) -> (String, i32) {
    let out = Command::new(bin())
        .args(["--config", cfg.to_str().unwrap()])
        .args(args)
        .env("HOME", home)
        .env("RTOK_HOME", home.join(".rtok"))
        .output()
        .expect("rtok");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "rtok {args:?} failed: {stderr}\n{stdout}"
    );
    (stdout, out.status.code().unwrap_or(1))
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The `.bak-*` copies sitting beside `path`, oldest name first.
fn backups(path: &Path) -> Vec<PathBuf> {
    let name = format!("{}.bak-", path.file_name().unwrap().to_string_lossy());
    let mut found: Vec<PathBuf> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&name))
        .collect();
    found.sort();
    found
}

#[test]
fn claude_remove_strips_hooks_mcp_and_proxy_and_keeps_foreign() {
    let home = tmp("claude");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    let claude_json = home.join(".claude.json");
    fs::write(
        &settings,
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"other-tool run"}]}]},"env":{"KEEP":"1"}}"#,
    )
    .unwrap();
    fs::write(
        &claude_json,
        r#"{"mcpServers":{"foreign":{"command":"x"}}}"#,
    )
    .unwrap();

    rtok(
        &["agent", "setup", "claude", "--mcp", "--proxy"],
        &cfg,
        &home,
    );
    let after_setup = fs::read_to_string(&settings).unwrap();
    assert!(
        after_setup.contains("rtok hook PreToolUse"),
        "{after_setup}"
    );
    assert!(after_setup.contains("ANTHROPIC_BASE_URL"), "{after_setup}");
    assert!(json(&claude_json)["mcpServers"]["rtok"].is_object());

    let (out, _) = rtok(&["agent", "remove", "claude"], &cfg, &home);
    assert!(out.contains("backup "), "remove reports its copies: {out}");

    let left = fs::read_to_string(&settings).unwrap();
    assert!(!left.contains("rtok hook"), "every hook goes: {left}");
    assert!(
        !left.contains("ANTHROPIC_BASE_URL"),
        "the proxy variable goes: {left}"
    );
    assert!(
        left.contains("other-tool run"),
        "foreign hook stays: {left}"
    );
    assert!(left.contains("\"KEEP\""), "foreign env stays: {left}");
    let servers = json(&claude_json);
    assert!(servers["mcpServers"]["rtok"].is_null(), "{servers}");
    assert!(servers["mcpServers"]["foreign"].is_object(), "{servers}");

    // The copy taken before the remove holds the installed state, so the remove is undoable.
    let bak = backups(&settings);
    let newest = fs::read_to_string(bak.last().expect("a backup beside settings.json")).unwrap();
    assert_eq!(
        newest, after_setup,
        "backup is the file as the command found it"
    );

    let (again, _) = rtok(&["agent", "remove", "claude"], &cfg, &home);
    assert!(again.contains("no changes"), "second remove: {again}");
}

#[test]
fn cursor_remove_strips_hooks_mcp_and_plugin_link() {
    let home = tmp("cursor");
    let cfg = write_cfg(&home);
    let hooks = home.join(".cursor/hooks.json");
    let mcp = home.join(".cursor/mcp.json");
    fs::write(&mcp, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();

    rtok(&["agent", "setup", "cursor", "--yes"], &cfg, &home);
    assert!(fs::read_to_string(&hooks).unwrap().contains("rtok hook"));
    let link = home.join(".cursor/plugins/local/rtok");
    assert!(link.symlink_metadata().is_ok(), "plugin linked");

    rtok(&["agent", "remove", "cursor"], &cfg, &home);
    let left = fs::read_to_string(&hooks).unwrap();
    assert!(!left.contains("rtok hook"), "{left}");
    assert!(link.symlink_metadata().is_err(), "plugin link unlinked");
    let servers = json(&mcp);
    assert!(servers["mcpServers"]["rtok"].is_null(), "{servers}");
    assert!(servers["mcpServers"]["foreign"].is_object(), "{servers}");
}

#[test]
fn codex_remove_strips_mcp_block_and_provider_and_keeps_foreign() {
    let home = tmp("codex");
    let cfg = write_cfg(&home);
    let path = home.join(".codex/config.toml");
    fs::write(&path, "# mine\n[mcp_servers.foreign]\ncommand = \"x\"\n").unwrap();

    rtok(&["agent", "setup", "codex", "--proxy"], &cfg, &home);
    let installed = fs::read_to_string(&path).unwrap();
    assert!(installed.contains("[mcp_servers.rtok]"), "{installed}");

    rtok(&["agent", "remove", "codex"], &cfg, &home);
    let left = fs::read_to_string(&path).unwrap();
    assert!(!left.contains("mcp_servers.rtok"), "{left}");
    assert!(!left.contains("rtok"), "no rtok provider either: {left}");
    assert!(left.contains("[mcp_servers.foreign]"), "{left}");
    assert!(left.contains("# mine"), "comments survive: {left}");
}

#[test]
fn opencode_remove_strips_base_url_and_keeps_foreign() {
    let home = tmp("opencode");
    let cfg = write_cfg(&home);
    let path = home.join(".config/opencode/opencode.json");
    fs::write(&path, r#"{"env":{"KEEP":"1"}}"#).unwrap();

    rtok(&["agent", "setup", "opencode"], &cfg, &home);
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("OPENAI_BASE_URL"),
        "installed"
    );

    rtok(&["agent", "remove", "opencode"], &cfg, &home);
    let left = fs::read_to_string(&path).unwrap();
    assert!(!left.contains("OPENAI_BASE_URL"), "{left}");
    assert!(left.contains("\"KEEP\""), "{left}");
}

#[test]
fn pi_remove_unlinks_the_extension() {
    let home = tmp("pi");
    let cfg = write_cfg(&home);
    rtok(&["agent", "setup", "pi", "--yes"], &cfg, &home);
    let link = home.join(".pi/agent/extensions/rtok");
    assert!(link.symlink_metadata().is_ok(), "extension linked");

    rtok(&["agent", "remove", "pi"], &cfg, &home);
    assert!(link.symlink_metadata().is_err(), "extension unlinked");
    let (again, _) = rtok(&["agent", "remove", "pi"], &cfg, &home);
    assert!(again.contains("no changes"), "{again}");
}

#[test]
fn setup_copies_the_config_before_it_writes() {
    let home = tmp("bak");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    let before = r#"{"env":{"KEEP":"1"}}"#;
    fs::write(&settings, before).unwrap();

    let (out, _) = rtok(&["agent", "setup", "claude"], &cfg, &home);
    assert!(
        out.contains("backup "),
        "setup reports its copies too: {out}"
    );
    let bak = backups(&settings);
    assert_eq!(bak.len(), 1, "one copy per file per run: {bak:?}");
    assert_eq!(
        fs::read_to_string(&bak[0]).unwrap(),
        before,
        "the copy predates the install"
    );

    // A second run inside the same second must not overwrite the first copy.
    rtok(&["agent", "remove", "claude"], &cfg, &home);
    assert_eq!(backups(&settings).len(), 2, "each run keeps its own copy");
}

#[test]
fn dry_run_remove_writes_nothing() {
    let home = tmp("dry");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    fs::write(&settings, r#"{"env":{"KEEP":"1"}}"#).unwrap();
    rtok(&["agent", "setup", "claude"], &cfg, &home);
    let installed = fs::read_to_string(&settings).unwrap();

    rtok(&["agent", "remove", "claude", "--dry-run"], &cfg, &home);
    assert_eq!(
        fs::read_to_string(&settings).unwrap(),
        installed,
        "dry-run leaves the file alone"
    );
    assert_eq!(
        backups(&settings).len(),
        1,
        "dry-run takes no copy either; only the install's"
    );
}
