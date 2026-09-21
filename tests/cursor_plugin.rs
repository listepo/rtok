//! T10.5 + D21: Cursor host plugin is one MCP, a singleton, desktop+CLI, ketch if missing.
//!
//! Check: `rtok agents install cursor --dry-run` names `plugins/cursor` and
//! `~/.cursor/plugins/local`; `--yes` links the plugin and does not add a second
//! `rtok` entry to `mcp.json`; second apply is `no changes`.

use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins/cursor")
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtok-t105-{name}-{}-{}",
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

fn write_cfg(home: &Path) -> PathBuf {
    let cursor = home.join(".cursor");
    fs::create_dir_all(&cursor).unwrap();
    let cfg = home.join("config.toml");
    fs::write(
        &cfg,
        format!(
            "[setup.cursor]\nhooks_path = \"{}/hooks.json\"\n",
            cursor.display()
        ),
    )
    .unwrap();
    cfg
}

/// Every call here is `agents install cursor …`; `--no-restart` (T141) keeps the test from
/// ever probing or touching a real Cursor process on the machine running it.
fn setup(args: &[&str], cfg: &Path, home: &Path) -> (String, String, i32) {
    let out = Command::new(bin())
        .args(["--config", cfg.to_str().unwrap()])
        .args(args)
        .arg("--no-restart")
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("RTOK_HOME", home.join(".rtok"))
        .output()
        .expect("rtok setup");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(1),
    )
}

#[test]
fn d21_plugin_and_mcp_are_one_unit() {
    let dir = root();
    let cursor = fs::read_to_string(dir.join(".cursor-plugin/plugin.json")).unwrap();
    let agent = fs::read_to_string(dir.join("plugin.json")).unwrap();
    let mcp: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("mcp.json")).unwrap()).unwrap();
    let servers = mcp["mcpServers"].as_object().expect("mcpServers");
    assert_eq!(servers.len(), 1, "singleton: one MCP server");
    assert!(servers.contains_key("rtok"), "{mcp}");
    assert!(cursor.contains("\"mcpServers\""), "{cursor}");
    assert!(cursor.contains("./mcp.json"), "{cursor}");
    assert!(agent.contains("\"name\": \"rtok\""), "{agent}");
}

#[test]
fn d21_desktop_and_cli_manifests() {
    let dir = root();
    assert!(dir.join(".cursor-plugin/plugin.json").is_file());
    assert!(dir.join("plugin.json").is_file());
}

#[test]
fn d21_no_duplicate_call_paths() {
    let hooks: Value =
        serde_json::from_str(&fs::read_to_string(root().join("hooks/hooks.json")).unwrap())
            .unwrap();
    let cmd = hooks["hooks"]["beforeShellExecution"][0]["command"]
        .as_str()
        .unwrap_or("");
    assert!(cmd.contains("rtok hook PreToolUse --host cursor"), "{cmd}");
    assert!(
        !cmd.contains("rtok read") && !cmd.contains("rtok search"),
        "hooks must not duplicate MCP read/search: {cmd}"
    );
    let mcp: Value =
        serde_json::from_str(&fs::read_to_string(root().join("mcp.json")).unwrap()).unwrap();
    let rtok = &mcp["mcpServers"]["rtok"];
    assert_eq!(rtok["command"], "rtok", "{mcp}");
    assert_eq!(rtok["args"], serde_json::json!(["mcp"]), "{mcp}");
}

#[test]
fn d21_mcp_json_invokes_rtok_directly() {
    let mcp: Value =
        serde_json::from_str(&fs::read_to_string(root().join("mcp.json")).unwrap()).unwrap();
    let rtok = &mcp["mcpServers"]["rtok"];
    assert_eq!(rtok["command"], "rtok", "cross-platform: no sh wrapper");
    assert_eq!(rtok["args"], serde_json::json!(["mcp"]));
}

#[cfg(unix)]
#[test]
fn d21_missing_rtok_names_ketch() {
    let script = root().join("scripts/mcp.sh");
    let out = Command::new("sh")
        .arg(&script)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("mcp.sh");
    assert!(
        !out.status.success(),
        "missing rtok must fail the MCP start"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("ketch install listepo/rtok"),
        "want ketch install, got {err}"
    );
    assert!(err.contains("rtok is not installed"), "{err}");
}

#[cfg(windows)]
#[test]
fn d21_missing_rtok_names_ketch_cmd() {
    let script = root().join("scripts/mcp.cmd");
    let out = Command::new("cmd")
        .args(["/C", script.to_str().unwrap()])
        .env_clear()
        .env("PATH", "C:\\Windows\\System32")
        .output()
        .expect("mcp.cmd");
    assert!(
        !out.status.success(),
        "missing rtok must fail the MCP start"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("ketch install listepo/rtok"),
        "want ketch install, got {err}"
    );
    assert!(err.contains("rtok is not installed"), "{err}");
}

#[test]
fn d21_ketch_helpers_exist_for_both_platforms() {
    assert!(root().join("scripts/mcp.sh").is_file());
    assert!(root().join("scripts/mcp.cmd").is_file());
}

#[test]
fn setup_cursor_dry_run_offers_plugin() {
    let home = tmp("dry");
    let cfg = write_cfg(&home);
    let (stdout, stderr, code) = setup(&["agents", "install", "cursor", "--dry-run"], &cfg, &home);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("plugins/cursor"), "stdout={stdout}");
    assert!(
        stdout.contains("~/.cursor/plugins/local"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("ketch install listepo/rtok"),
        "stdout={stdout}"
    );
    assert!(
        !home.join(".cursor/plugins/local/rtok").exists(),
        "dry-run must not link"
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn setup_cursor_yes_links_plugin_without_mcp_json() {
    let home = tmp("yes");
    let cfg = write_cfg(&home);
    let (stdout, stderr, code) = setup(&["agents", "install", "cursor", "--yes"], &cfg, &home);
    assert_eq!(code, 0, "stderr={stderr} stdout={stdout}");
    let dest = home.join(".cursor/plugins/local/rtok");
    let meta = fs::symlink_metadata(&dest).unwrap_or_else(|e| panic!("{}: {e}", dest.display()));
    assert!(meta.file_type().is_symlink() || dest.is_dir(), "{dest:?}");
    let mcp_path = home.join(".cursor/mcp.json");
    if mcp_path.is_file() {
        let body = fs::read_to_string(&mcp_path).unwrap();
        assert!(
            !body.contains("\"rtok\""),
            "plugin is the MCP; no second registration: {body}"
        );
    }
    let (again, stderr2, code2) = setup(&["agents", "install", "cursor", "--yes"], &cfg, &home);
    assert_eq!(code2, 0, "stderr={stderr2}");
    assert!(again.contains("already installed"), "second apply: {again}");
    let (rm, stderr3, code3) = setup(&["agents", "install", "cursor", "--remove"], &cfg, &home);
    assert_eq!(code3, 0, "stderr={stderr3}");
    assert!(!dest.exists(), "remove must unlink plugin; stdout={rm}");
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn setup_cursor_clears_leftover_mcp_when_plugin_already_linked() {
    let home = tmp("leftover-mcp");
    let cfg = write_cfg(&home);
    let (stdout, stderr, code) = setup(&["agents", "install", "cursor", "--yes"], &cfg, &home);
    assert_eq!(code, 0, "stderr={stderr} stdout={stdout}");
    let dest = home.join(".cursor/plugins/local/rtok");
    assert!(dest.symlink_metadata().is_ok(), "plugin must be linked");
    let mcp_path = home.join(".cursor/mcp.json");
    fs::write(
        &mcp_path,
        r#"{"mcpServers":{"rtok":{"type":"stdio","command":"rtok","args":["mcp"]},"other":{"command":"x"}}}"#,
    )
    .unwrap();
    let (again, stderr2, code2) = setup(&["agents", "install", "cursor"], &cfg, &home);
    assert_eq!(code2, 0, "stderr={stderr2} stdout={again}");
    let body = fs::read_to_string(&mcp_path).unwrap();
    assert!(
        !body.contains("\"rtok\""),
        "already-linked setup must clear leftover mcpServers.rtok: {body}"
    );
    assert!(
        body.contains("other"),
        "foreign MCP servers must remain: {body}"
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn post_tool_use_shortens_long_mcp_results_and_skips_small_and_rtok() {
    let home = tmp("mcp-hook");
    let rtok_home = home.join(".rtok");
    fs::create_dir_all(&rtok_home).unwrap();
    let long: String = (1..=200).map(|i| format!("line {i}\n")).collect();
    let hook = |server: &str, tool: &str, text: &str| -> Value {
        let result = serde_json::json!({"content":[{"type":"text","text": text}]});
        let stdin = serde_json::json!({
            "hook_event_name": "postToolUse",
            "tool_name": tool,
            "tool_input": {},
            "tool_output": result.to_string(),
            "conversation_id": "e2e",
            "mcp_server_name": server
        });
        let mut child = Command::new(bin())
            .args(["hook", "PostToolUse", "--host", "cursor"])
            .env("RTOK_HOME", &rtok_home)
            .env("HOME", &home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&stdin).unwrap())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let big = hook("linear", "MCP:list_issues", &long);
    let printed = big["updated_mcp_tool_output"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(printed.contains("expand: rtok expand "), "{big}");
    assert!(printed.len() < long.len());
    let rows = rtok::store::Store::open(&rtok_home.join("rtok.db"))
        .unwrap()
        .list_measurements("archive")
        .unwrap();
    assert_eq!(rows.iter().filter(|r| r.kind == "mcp").count(), 1);
    assert_eq!(hook("linear", "MCP:list_issues", "ok\n"), json!({}));
    assert_eq!(hook("rtok", "MCP:search", &long), json!({}));
    let _ = fs::remove_dir_all(&home);
}
