//! T10.9: `rtok agents remove <host>` takes back everything `rtok agents setup <host>` wrote,
//! and both commands copy the host's config files before they touch anything.
//!
//! Check: per host, seed a foreign entry, install, remove, and assert rtok is gone while the
//! foreign entry stays; assert the `.bak-<ts>` copy holds the file as it was before the command;
//! assert a second remove is `no changes` and that `--dry-run` writes nothing at all.

mod common;

use common::agents::{backups, json, rtok, tmp, write_cfg};
use std::fs;

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
        &["agents", "setup", "claude", "--mcp", "--proxy"],
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

    let out = rtok(&["agents", "remove", "claude"], &cfg, &home);
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

    let again = rtok(&["agents", "remove", "claude"], &cfg, &home);
    assert!(again.contains("no changes"), "second remove: {again}");
}

#[test]
fn cursor_remove_strips_hooks_mcp_and_plugin_link() {
    let home = tmp("cursor");
    let cfg = write_cfg(&home);
    let hooks = home.join(".cursor/hooks.json");
    let mcp = home.join(".cursor/mcp.json");
    fs::write(&mcp, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();

    rtok(&["agents", "setup", "cursor", "--yes"], &cfg, &home);
    assert!(fs::read_to_string(&hooks).unwrap().contains("rtok hook"));
    let link = home.join(".cursor/plugins/local/rtok");
    assert!(link.symlink_metadata().is_ok(), "plugin linked");

    rtok(&["agents", "remove", "cursor"], &cfg, &home);
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

    rtok(&["agents", "setup", "codex", "--proxy"], &cfg, &home);
    let installed = fs::read_to_string(&path).unwrap();
    assert!(installed.contains("[mcp_servers.rtok]"), "{installed}");

    rtok(&["agents", "remove", "codex"], &cfg, &home);
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

    rtok(&["agents", "setup", "opencode"], &cfg, &home);
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("OPENAI_BASE_URL"),
        "installed"
    );

    rtok(&["agents", "remove", "opencode"], &cfg, &home);
    let left = fs::read_to_string(&path).unwrap();
    assert!(!left.contains("OPENAI_BASE_URL"), "{left}");
    assert!(left.contains("\"KEEP\""), "{left}");
}

#[test]
fn pi_remove_unlinks_the_extension() {
    let home = tmp("pi");
    let cfg = write_cfg(&home);
    rtok(&["agents", "setup", "pi", "--yes"], &cfg, &home);
    let link = home.join(".pi/agent/extensions/rtok");
    assert!(link.symlink_metadata().is_ok(), "extension linked");

    rtok(&["agents", "remove", "pi"], &cfg, &home);
    assert!(link.symlink_metadata().is_err(), "extension unlinked");
    let again = rtok(&["agents", "remove", "pi"], &cfg, &home);
    assert!(again.contains("no changes"), "{again}");
}

#[test]
fn setup_copies_the_config_before_it_writes() {
    let home = tmp("bak");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    let before = r#"{"env":{"KEEP":"1"}}"#;
    fs::write(&settings, before).unwrap();

    let out = rtok(&["agents", "setup", "claude"], &cfg, &home);
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
    rtok(&["agents", "remove", "claude"], &cfg, &home);
    assert_eq!(backups(&settings).len(), 2, "each run keeps its own copy");
}

#[test]
fn dry_run_remove_writes_nothing() {
    let home = tmp("dry");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    fs::write(&settings, r#"{"env":{"KEEP":"1"}}"#).unwrap();
    rtok(&["agents", "setup", "claude"], &cfg, &home);
    let installed = fs::read_to_string(&settings).unwrap();

    rtok(&["agents", "remove", "claude", "--dry-run"], &cfg, &home);
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
