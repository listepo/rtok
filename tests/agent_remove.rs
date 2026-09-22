//! T10.9: `rtok agents uninstall <host>` (alias `remove`) takes back everything `rtok agents install <host>` wrote,
//! and both commands copy the host's config files before they touch anything.
//!
//! Check: per host, seed a foreign entry, install, remove, and assert rtok is gone while the
//! foreign entry stays; assert the `.bak-<ts>` copy holds the file as it was before the command;
//! assert a second remove is `no changes` and that `--dry-run` writes nothing at all.

mod common;

use common::agents::{backups, contains_hook, json, rtok, rtok_without_claude, tmp, write_cfg};
use std::fs;
use std::path::PathBuf;

/// No `claude` on PATH (T139: the plugin is the default once it is there), so this exercises
/// the settings-file fallback: hooks, MCP and the proxy env var, all in `~/.claude/*`.
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

    rtok_without_claude(
        &["agents", "install", "claude", "--mcp", "--proxy"],
        &cfg,
        &home,
    );
    let after_setup = fs::read_to_string(&settings).unwrap();
    assert!(contains_hook(&after_setup, "PreToolUse"), "{after_setup}");
    assert!(after_setup.contains("ANTHROPIC_BASE_URL"), "{after_setup}");
    assert!(json(&claude_json)["mcpServers"]["rtok"].is_object());

    let out = rtok_without_claude(&["agents", "remove", "claude"], &cfg, &home);
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

    let again = rtok_without_claude(&["agents", "remove", "claude"], &cfg, &home);
    assert!(again.contains("no changes"), "second remove: {again}");
}

#[test]
fn cursor_remove_strips_hooks_mcp_and_plugin_link() {
    let home = tmp("cursor");
    let cfg = write_cfg(&home);
    let hooks = home.join(".cursor/hooks.json");
    let mcp = home.join(".cursor/mcp.json");
    fs::write(&mcp, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();

    rtok(&["agents", "install", "cursor", "--yes"], &cfg, &home);
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

    rtok(&["agents", "install", "codex", "--proxy"], &cfg, &home);
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

    rtok(&["agents", "install", "opencode"], &cfg, &home);
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
    rtok(&["agents", "install", "pi", "--yes"], &cfg, &home);
    let link = home.join(".pi/agent/extensions/rtok");
    assert!(link.symlink_metadata().is_ok(), "extension linked");

    rtok(&["agents", "remove", "pi"], &cfg, &home);
    assert!(link.symlink_metadata().is_err(), "extension unlinked");
    let again = rtok(&["agents", "remove", "pi"], &cfg, &home);
    assert!(again.contains("no changes"), "{again}");
}

#[test]
fn zcode_remove_keeps_foreign_events_and_servers() {
    let home = tmp("zcode");
    let cfg = write_cfg(&home);
    let path = home.join(".zcode/cli/config.json");
    fs::write(
        &path,
        r#"{"hooks":{"events":{"Stop":[{"hooks":[{"type":"command","command":"echo other"}]}]}},"mcp":{"servers":{"foreign":{"command":"x"}}}}"#,
    )
    .unwrap();

    // T164: no `--yes` — ZCode is detected by the explicit host name, so the plugin
    // links by default and becomes the only call path, leaving the config-file hooks
    // and mcp entries untouched (only the foreign ones were ever there).
    rtok(&["agents", "install", "zcode"], &cfg, &home);
    // Built the same way `write_cfg` + `plugin_dest` derive it: the config path is one
    // all-forward-slash string (`write_cfg` normalizes `home` before embedding it), and
    // `plugin_dest` then does one `.join()` per segment, which inserts a native separator
    // (`\` on Windows) at each call. Matching that construction keeps this byte-identical
    // with what actually lands in `plugins.dirs`.
    let zcode_cli = PathBuf::from(format!(
        "{}/.zcode/cli",
        home.display().to_string().replace('\\', "/")
    ));
    let link = zcode_cli.join("plugins").join("local").join("rtok");
    assert!(link.symlink_metadata().is_ok(), "plugin linked by default");
    let installed = json(&path);
    let link_str = link.display().to_string();
    assert!(
        installed["plugins"]["dirs"]
            .as_array()
            .is_some_and(|dirs| dirs.iter().any(|d| d.as_str() == Some(link_str.as_str()))),
        "{installed}"
    );
    assert!(!installed.to_string().contains("PreToolUse"), "{installed}");
    assert!(installed["mcp"]["servers"]["rtok"].is_null(), "{installed}");
    assert!(
        installed["mcp"]["servers"]["foreign"].is_object(),
        "{installed}"
    );
    assert_eq!(
        installed["hooks"]["events"]["Stop"][0]["hooks"][0]["command"],
        "echo other"
    );

    rtok(&["agents", "remove", "zcode"], &cfg, &home);
    assert!(link.symlink_metadata().is_err(), "plugin link unlinked");
    let left = json(&path);
    assert!(
        !left["plugins"]["dirs"]
            .as_array()
            .is_some_and(|dirs| dirs.iter().any(|d| d.as_str() == Some(link_str.as_str()))),
        "{left}"
    );
    assert!(left["mcp"]["servers"]["foreign"].is_object(), "{left}");
    assert_eq!(
        left["hooks"]["events"]["Stop"][0]["hooks"][0]["command"],
        "echo other"
    );
    let again = rtok(&["agents", "remove", "zcode"], &cfg, &home);
    assert!(again.contains("no changes"), "{again}");
}

#[test]
fn kimi_remove_keeps_comments_and_foreign_hooks() {
    let home = tmp("kimi");
    let cfg = write_cfg(&home);
    let path = home.join(".kimi-code/config.toml");
    let mcp = home.join(".kimi-code/mcp.json");
    fs::write(
        &path,
        "# mine\n[[hooks]]\nevent = \"Stop\"\ncommand = \"echo other\"\n",
    )
    .unwrap();
    fs::write(&mcp, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();

    rtok(&["agents", "install", "kimi"], &cfg, &home);
    assert!(fs::read_to_string(&path).unwrap().contains("rtok"));
    assert!(json(&mcp)["mcpServers"]["rtok"].is_object());

    rtok(&["agents", "remove", "kimi"], &cfg, &home);
    let left = fs::read_to_string(&path).unwrap();
    assert!(!left.contains("rtok"), "every hook goes: {left}");
    assert!(left.contains("# mine"), "comments survive: {left}");
    assert!(left.contains("echo other"), "foreign hook stays: {left}");
    let servers = json(&mcp);
    assert!(servers["mcpServers"]["rtok"].is_null(), "{servers}");
    assert!(servers["mcpServers"]["foreign"].is_object(), "{servers}");
    let again = rtok(&["agents", "remove", "kimi"], &cfg, &home);
    assert!(again.contains("no changes"), "{again}");
}

/// T86 D21 singleton at the binary level: with the plugin installed
/// (`plugins/managed/rtok/kimi.plugin.json` seeded, as Kimi's own
/// `/plugins install` would write it), install strips rtok's own tables and
/// `mcpServers.rtok` instead of adding them and reports `plugin`; remove leaves
/// the managed copy alone with its own remove line.
#[test]
fn kimi_plugin_singleton_strips_own_tables_and_keeps_the_managed_copy() {
    let home = tmp("kimi-singleton");
    let cfg = write_cfg(&home);
    let path = home.join(".kimi-code/config.toml");
    let mcp = home.join(".kimi-code/mcp.json");
    let marker = home.join(".kimi-code/plugins/managed/rtok/kimi.plugin.json");

    // Plain install first: hooks + MCP land in the user files.
    rtok(&["agents", "install", "kimi"], &cfg, &home);
    assert!(fs::read_to_string(&path).unwrap().contains("rtok hook"));
    assert!(json(&mcp)["mcpServers"]["rtok"].is_object());

    // Kimi installs the plugin: seed the managed copy it would write.
    fs::create_dir_all(marker.parent().unwrap()).unwrap();
    fs::write(&marker, "{}").unwrap();

    let second = rtok(&["agents", "install", "kimi"], &cfg, &home);
    assert!(second.contains("plugin"), "{second}");
    assert!(
        !fs::read_to_string(&path).unwrap().contains("rtok hook"),
        "own tables stripped while the plugin serves them"
    );
    assert!(
        json(&mcp)["mcpServers"]["rtok"].is_null(),
        "own MCP entry stripped while the plugin serves it"
    );

    let rm = rtok(&["agents", "remove", "kimi"], &cfg, &home);
    assert!(rm.contains("/plugins remove rtok"), "{rm}");
    assert!(marker.is_file(), "remove leaves the managed copy alone");
}

#[test]
fn copilot_remove_deletes_hooks_file_and_keeps_foreign_servers() {
    let home = tmp("copilot");
    let cfg = write_cfg(&home);
    let hooks = home.join(".copilot/hooks/rtok.json");
    let foreign_hooks = home.join(".copilot/hooks/other.json");
    let mcp = home.join(".copilot/mcp-config.json");
    fs::write(&foreign_hooks, r#"{"version":1,"hooks":{}}"#).unwrap();
    fs::write(&mcp, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();

    rtok(&["agents", "install", "copilot"], &cfg, &home);
    assert_eq!(json(&hooks)["version"], 1);
    assert!(json(&mcp)["mcpServers"]["rtok"].is_object());

    let out = rtok(&["agents", "remove", "copilot"], &cfg, &home);
    assert!(!hooks.exists(), "rtok's own hooks file goes: {out}");
    let bak = backups(&hooks);
    assert_eq!(bak.len(), 1, "the deleted file is copied first: {bak:?}");
    assert_eq!(json(&bak[0])["version"], 1);
    assert!(foreign_hooks.exists(), "another hooks file stays");
    let servers = json(&mcp);
    assert!(servers["mcpServers"]["rtok"].is_null(), "{servers}");
    assert!(servers["mcpServers"]["foreign"].is_object(), "{servers}");
    let again = rtok(&["agents", "remove", "copilot"], &cfg, &home);
    assert!(again.contains("no changes"), "{again}");
}

#[test]
fn aider_remove_strips_only_ours_and_keeps_comments_and_foreign() {
    let home = tmp("aider");
    let cfg = write_cfg(&home);
    let path = home.join(".aider.conf.yml");
    fs::write(
        &path,
        "# mine\nmodel: openai/gpt-4o\nopenai-api-base: https://foreign.example/v1\n",
    )
    .unwrap();

    // A foreign base URL is not ours: remove reports no changes and keeps it.
    let none = rtok(&["agents", "remove", "aider"], &cfg, &home);
    assert!(none.contains("no changes"), "{none}");
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("https://foreign.example/v1"),
        "foreign base URL stays"
    );

    rtok(&["agents", "install", "aider", "--proxy"], &cfg, &home);
    let installed = fs::read_to_string(&path).unwrap();
    assert!(
        installed.contains("openai-api-base: http://"),
        "{installed}"
    );
    assert!(installed.contains("8790/v1"), "{installed}");
    assert!(
        installed.contains("# mine"),
        "comments survive: {installed}"
    );

    rtok(&["agents", "remove", "aider"], &cfg, &home);
    let left = fs::read_to_string(&path).unwrap();
    assert!(!left.contains("openai-api-base:"), "{left}");
    assert!(left.contains("# mine"), "comments survive: {left}");
    assert!(left.contains("model: openai/gpt-4o"), "{left}");
    let again = rtok(&["agents", "remove", "aider"], &cfg, &home);
    assert!(again.contains("no changes"), "second remove: {again}");
}

#[test]
fn windsurf_remove_keeps_foreign_servers() {
    let home = tmp("windsurf");
    let cfg = write_cfg(&home);
    let path = home.join(".codeium/windsurf/mcp_config.json");
    fs::write(&path, r#"{"mcpServers":{"foreign":{"command":"x"}}}"#).unwrap();

    rtok(&["agents", "install", "windsurf"], &cfg, &home);
    assert!(json(&path)["mcpServers"]["rtok"].is_object());

    rtok(&["agents", "remove", "windsurf"], &cfg, &home);
    let servers = json(&path);
    assert!(servers["mcpServers"]["rtok"].is_null(), "{servers}");
    assert!(servers["mcpServers"]["foreign"].is_object(), "{servers}");
    let again = rtok(&["agents", "remove", "windsurf"], &cfg, &home);
    assert!(again.contains("no changes"), "second remove: {again}");
}

#[test]
fn zed_remove_keeps_comments_and_foreign_servers() {
    let home = tmp("zed");
    let cfg = write_cfg(&home);
    let path = home.join(".config/zed/settings.json");
    fs::write(
        &path,
        "{\n  // my theme\n  \"theme\": \"One Dark\",\n  \"context_servers\": {\n    // foreign\n    \"other\": {\"command\": \"npx\", \"args\": [\"x\"]}\n  }\n}\n",
    )
    .unwrap();

    rtok(&["agents", "install", "zed"], &cfg, &home);
    let installed: serde_json::Value = serde_json::from_str(&rtok::agents::zed::strip_comments(
        &fs::read_to_string(&path).unwrap(),
    ))
    .unwrap();
    assert!(installed["context_servers"]["rtok"].is_object());

    rtok(&["agents", "remove", "zed"], &cfg, &home);
    let left = fs::read_to_string(&path).unwrap();
    assert!(!left.contains("\"rtok\""), "every rtok entry goes: {left}");
    assert!(left.contains("// my theme"), "comments survive: {left}");
    assert!(left.contains("// foreign"), "comments survive: {left}");
    let servers: serde_json::Value =
        serde_json::from_str(&rtok::agents::zed::strip_comments(&left)).unwrap();
    assert!(servers["context_servers"]["other"].is_object(), "{servers}");
    let again = rtok(&["agents", "remove", "zed"], &cfg, &home);
    assert!(again.contains("no changes"), "second remove: {again}");
}

/// No `claude` on PATH (T139), so the install writes `settings.json` itself instead of
/// handing it to the plugin — the file this test watches actually changes each run.
#[test]
fn setup_copies_the_config_before_it_writes() {
    let home = tmp("bak");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    let before = r#"{"env":{"KEEP":"1"}}"#;
    fs::write(&settings, before).unwrap();

    let out = rtok_without_claude(&["agents", "install", "claude"], &cfg, &home);
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
    rtok_without_claude(&["agents", "remove", "claude"], &cfg, &home);
    assert_eq!(backups(&settings).len(), 2, "each run keeps its own copy");
}

#[test]
fn dry_run_remove_writes_nothing() {
    let home = tmp("dry");
    let cfg = write_cfg(&home);
    let settings = home.join(".claude/settings.json");
    fs::write(&settings, r#"{"env":{"KEEP":"1"}}"#).unwrap();
    rtok(&["agents", "install", "claude"], &cfg, &home);
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

/// T75: after `agents uninstall` the row the agents/plugins UI renders must read not
/// installed — the green check may not outlive the command. The shape that used to
/// stick: a host materializes our plugin symlink into a plain copy, so the dest holds
/// our tree with no marker and no link; `installed()` counted any metadata there, so
/// the mark stayed on while remove left the "foreign" directory in place.
#[test]
fn uninstall_clears_the_installed_marks_over_a_materialized_plugin_copy() {
    let home = tmp("cursor-marks");
    let cfg = write_cfg(&home);
    rtok(&["agents", "install", "cursor", "--yes"], &cfg, &home);
    // The host "materialized" the link: replace it with a plain copy of the tree —
    // same bytes, no OWNED_MARKER, the exact shape the check got stuck on.
    let dest = home.join(".cursor/plugins/local/rtok");
    let src = fs::read_link(&dest).expect("install linked the plugin");
    fs::remove_file(&dest).unwrap();
    copy_tree(std::path::Path::new(&src), &dest);

    let installed_marks = |cfg: &std::path::Path, home: &std::path::Path| -> usize {
        let out = rtok(&["agents", "info", "cursor", "--json"], cfg, home);
        serde_json::from_str::<serde_json::Value>(&out)
            .expect("info json")
            .as_array()
            .expect("rows")
            .iter()
            .flat_map(|r| r["modules"].as_array().cloned().unwrap_or_default())
            .filter(|m| m["state"] == "installed")
            .count()
    };
    assert!(
        installed_marks(&cfg, &home) > 0,
        "sanity: the copy still reads as our install"
    );

    rtok(&["agents", "remove", "cursor"], &cfg, &home);
    assert_eq!(
        installed_marks(&cfg, &home),
        0,
        "no module may read installed after uninstall"
    );
    assert!(!dest.exists(), "the materialized copy was taken back");
}

/// `fs::copy` has no directory form; the tree here is small and shallow enough.
fn copy_tree(src: &std::path::Path, dest: &std::path::Path) {
    fs::create_dir_all(dest).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dest.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}
