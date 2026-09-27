//! T279: `plugins/<host>/.rtok-plugin-version` and every plugin manifest's `version` field must
//! equal the running rtok version, so a host that caches a plugin by its manifest version (Claude
//! Code) sees a new build as a new version. `tools/plugin-versions.sh --set` writes both from
//! `Cargo.toml` in the same commit as the version bump (`tools/release.sh`); this test is the
//! local half of the guarantee, alongside `tools/plugin-versions.sh --check` in `ci.yml` and
//! `release.yml` (docs/plugin-versions.md has the full scheme).
//!
//! The file list is not duplicated here: it comes from `tools/plugin-versions.sh --files` (also
//! how `tools/release.sh`'s `git add` gets it), so the test and the script can never disagree.
//! Skips (does not fail) when `bash` is not on `PATH` — Windows CI images carry Git Bash, but a
//! bare Windows box may not; once `--files` runs, a file it lists but that is missing on disk
//! still fails the test.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn bash_on_path() -> bool {
    Command::new("bash")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Every file `tools/plugin-versions.sh` touches, repo-root-relative, straight from its own
/// `--files` mode.
fn plugin_version_files() -> Vec<PathBuf> {
    let script = root().join("tools/plugin-versions.sh");
    let out = Command::new("bash")
        .arg(&script)
        .arg("--files")
        .current_dir(root())
        .output()
        .unwrap_or_else(|e| panic!("{}: {e}", script.display()));
    assert!(
        out.status.success(),
        "{}: {}",
        script.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(PathBuf::from)
        .collect()
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{}: not valid JSON: {e}", path.display()))
}

#[test]
fn every_rtok_plugin_version_file_parses_with_schema_1_and_the_right_plugin_name() {
    if !bash_on_path() {
        eprintln!("skip: bash not on PATH");
        return;
    }
    for rel in plugin_version_files() {
        if rel.file_name().and_then(|n| n.to_str()) != Some(".rtok-plugin-version") {
            continue;
        }
        let path = root().join(&rel);
        let host = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| panic!("{}: no host directory", path.display()));
        let json = read_json(&path);
        assert_eq!(json["schema"], 1, "{}: schema", path.display());
        assert_eq!(json["plugin"], host, "{}: plugin", path.display());
        assert_eq!(
            json["version"],
            env!("CARGO_PKG_VERSION"),
            "{}: version",
            path.display()
        );
    }
}

#[test]
fn every_plugin_manifest_version_matches_cargo_pkg_version() {
    if !bash_on_path() {
        eprintln!("skip: bash not on PATH");
        return;
    }
    for rel in plugin_version_files() {
        if rel.file_name().and_then(|n| n.to_str()) == Some(".rtok-plugin-version") {
            continue;
        }
        let path = root().join(&rel);
        let json = read_json(&path);
        assert_eq!(
            json["version"],
            env!("CARGO_PKG_VERSION"),
            "{}: version",
            path.display()
        );
    }
}

// ── T279.1: `rtok agents outdated` ───────────────────────────────────────────

mod common;

use common::agents::{bin, tmp, write_cfg};
use semver::Version;

fn target() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Default plugin install receipt for an isolated `HOME` (macOS layout; tests run on the creator's Mac).
fn receipt_file(home: &Path) -> PathBuf {
    home.join("Library/Application Support/rtok/plugins.json")
}

fn cfg_with_receipt(home: &Path) -> PathBuf {
    write_cfg(home)
}

fn claude_plugin_installed(home: &Path, version: &str, install_path: Option<&Path>) {
    let dir = home.join(".claude/plugins");
    fs::create_dir_all(&dir).unwrap();
    let install = install_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| {
            home.join(".claude/plugins/cache/rtok/rtok/0.0.1")
                .display()
                .to_string()
        });
    fs::write(
        dir.join("installed_plugins.json"),
        format!(
            r#"{{"version":2,"plugins":{{"rtok@rtok":[{{"scope":"user","version":"{version}","installPath":"{install}"}}]}}}}"#
        ),
    )
    .unwrap();
}

fn write_version_file(dir: &Path, host: &str, version: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join(".rtok-plugin-version"),
        format!(r#"{{"schema":1,"plugin":"{host}","version":"{version}"}}"#),
    )
    .unwrap();
}

fn run_outdated(args: &[&str], cfg: &Path, home: &Path) -> (String, i32) {
    let path = if cfg!(windows) {
        std::ffi::OsString::from(r"C:\Windows\System32")
    } else {
        std::ffi::OsString::from("/usr/bin:/bin")
    };
    let out = Command::new(bin())
        .args(["--config", cfg.to_str().unwrap()])
        .args(args)
        .env("PATH", path)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("RTOK_HOME", home.join(".rtok"))
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(1),
    )
}

fn write_receipt(home: &Path, body: &str) {
    let path = receipt_file(home);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn mark_cursor_plugin_ours(dest: &Path) {
    fs::write(dest.join(".rtok-owned"), b"").unwrap();
}

#[test]
fn outdated_no_plugins_installed_message_and_empty_json() {
    let home = tmp("outdated-none");
    let cfg = cfg_with_receipt(&home);
    let (text, code) = run_outdated(&["agents", "outdated"], &cfg, &home);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("no rtok plugins installed"), "{text}");
    let (json, _) = run_outdated(&["agents", "outdated", "--json"], &cfg, &home);
    let v: Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(v["rtok"], target());
    assert!(v["outdated"].as_array().unwrap().is_empty());
    assert_eq!(v["installed"], 0);
}

#[test]
fn outdated_all_current_summary_line() {
    let home = tmp("outdated-current");
    let cfg = cfg_with_receipt(&home);
    let install = home.join("cursor/rtok");
    write_version_file(&install, "cursor", &target());
    write_receipt(
        &home,
        &format!(
            r#"{{"cursor":{{"source":"local","ref":"checkout","path":"{}","version":"{}","installed_at":"t"}}}}"#,
            install.display(),
            target()
        ),
    );
    let dest = home.join(".cursor/plugins/local/rtok");
    fs::create_dir_all(dest.join(".cursor-plugin")).unwrap();
    mark_cursor_plugin_ours(&dest);
    fs::write(
        dest.join(".cursor-plugin/plugin.json"),
        r#"{"name":"rtok","version":"9.9.9"}"#,
    )
    .unwrap();
    fs::write(
        home.join(".cursor/hooks.json"),
        r#"{"version":1,"hooks":[]}"#,
    )
    .unwrap();
    let (text, code) = run_outdated(&["agents", "outdated", "cursor"], &cfg, &home);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains(&format!(
            "all rtok plugins are up to date (2 installed, rtok {})",
            target()
        )),
        "{text}"
    );
}

#[test]
fn outdated_lists_only_behind_rows() {
    let home = tmp("outdated-mixed");
    let cfg = cfg_with_receipt(&home);
    claude_plugin_installed(&home, "0.0.1", None);
    let cursor_install = home.join("cursor/rtok");
    write_version_file(&cursor_install, "cursor", &target());
    write_receipt(
        &home,
        &format!(
            r#"{{"claude":{{"source":"github","ref":"v0.10.0","path":"{}","version":"0.0.1","installed_at":"t"}},"cursor":{{"source":"local","ref":"x","path":"{}","version":"{}","installed_at":"t"}}}}"#,
            home.join(".claude/plugins/cache/rtok/rtok/0.0.1").display(),
            cursor_install.display(),
            target()
        ),
    );
    let dest = home.join(".cursor/plugins/local/rtok");
    fs::create_dir_all(dest.join(".cursor-plugin")).unwrap();
    mark_cursor_plugin_ours(&dest);
    fs::write(
        dest.join(".cursor-plugin/plugin.json"),
        r#"{"name":"rtok","version":"9.9.9"}"#,
    )
    .unwrap();
    let (text, _) = run_outdated(&["agents", "outdated"], &cfg, &home);
    assert!(text.contains("claude"), "{text}");
    assert!(text.contains("legacy"), "{text}");
    assert!(!text.lines().any(|l| l.starts_with("cursor ")), "{text}");
}

#[test]
fn outdated_legacy_install_shows_legacy() {
    let home = tmp("outdated-legacy");
    let cfg = cfg_with_receipt(&home);
    claude_plugin_installed(&home, "0.0.1", None);
    let (text, _) = run_outdated(&["agents", "outdated", "claude"], &cfg, &home);
    assert!(text.contains("legacy"), "{text}");
}

#[test]
fn outdated_skips_newer_than_running() {
    let home = tmp("outdated-newer");
    let cfg = cfg_with_receipt(&home);
    let install = home.join("claude/rtok");
    fs::create_dir_all(&install).unwrap();
    let mut newer = Version::parse(&target()).unwrap();
    newer.major += 10;
    claude_plugin_installed(&home, &newer.to_string(), Some(&install));
    write_version_file(&install, "claude", &newer.to_string());
    let (text, _) = run_outdated(&["agents", "outdated", "claude"], &cfg, &home);
    assert!(text.contains("all rtok plugins are up to date"), "{text}");
}

#[test]
fn outdated_ignores_build_metadata_on_same_base() {
    let home = tmp("outdated-metadata");
    let cfg = cfg_with_receipt(&home);
    let install = home.join("cursor/rtok");
    let local = format!("{}+g12c7e91", target());
    write_version_file(&install, "cursor", &local);
    write_receipt(
        &home,
        &format!(
            r#"{{"cursor":{{"source":"local","ref":"x","path":"{}","version":"{}","installed_at":"t"}}}}"#,
            install.display(),
            local
        ),
    );
    let dest = home.join(".cursor/plugins/local/rtok");
    fs::create_dir_all(dest.join(".cursor-plugin")).unwrap();
    mark_cursor_plugin_ours(&dest);
    fs::write(
        dest.join(".cursor-plugin/plugin.json"),
        r#"{"name":"rtok","version":"9.9.9"}"#,
    )
    .unwrap();
    let (text, _) = run_outdated(&["agents", "outdated", "cursor"], &cfg, &home);
    assert!(text.contains("all rtok plugins are up to date"), "{text}");
}

#[test]
fn outdated_json_schema_and_exit_code() {
    let home = tmp("outdated-json-exit");
    let cfg = cfg_with_receipt(&home);
    claude_plugin_installed(&home, "0.0.1", None);
    let (json, code) = run_outdated(
        &["agents", "outdated", "claude", "--json", "--exit-code"],
        &cfg,
        &home,
    );
    assert_eq!(code, 10);
    let v: Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(v["rtok"], target());
    assert_eq!(v["installed"], 1);
    let row = &v["outdated"][0];
    assert_eq!(row["agent"], "claude");
    assert_eq!(row["variant"], "cli");
    assert_eq!(row["installed"], "legacy");
    assert_eq!(row["available"], target());
    assert_eq!(row["source"], "github");
    assert_eq!(row["legacy"], true);
    let home2 = tmp("outdated-json-ok");
    let cfg2 = cfg_with_receipt(&home2);
    let (_, ok2) = run_outdated(
        &["agents", "outdated", "--json", "--exit-code"],
        &cfg2,
        &home2,
    );
    assert_eq!(ok2, 0);
}

#[test]
fn update_check_matches_outdated_output() {
    let home = tmp("outdated-alias");
    let cfg = cfg_with_receipt(&home);
    claude_plugin_installed(&home, "0.0.1", None);
    let (a, _) = run_outdated(&["agents", "outdated"], &cfg, &home);
    let (b, _) = run_outdated(&["agents", "update", "--check"], &cfg, &home);
    assert_eq!(a, b);
}

#[test]
fn outdated_does_not_spawn_host_cli() {
    let home = tmp("outdated-offline");
    let cfg = cfg_with_receipt(&home);
    claude_plugin_installed(&home, "0.0.1", None);
    let _ = run_outdated(&["agents", "outdated", "claude"], &cfg, &home);
    let log = home.join("claude.log");
    assert!(!log.exists() || fs::read_to_string(&log).unwrap_or_default().is_empty());
}
