// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

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

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::agents::{bin, tmp, write_cfg};
use semver::Version;
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

/// T279 PR 3: `agents update claude` wired to the decision engine
/// (`src/agents/plugin_version.rs`, `src/agents/claude/mod.rs`'s
/// `plugin_update`/`reinstall`/`update_in_place`), exercised end to end with a fake `claude`
/// CLI (`common::agents::fake_claude_path`, reused rather than respelled) so no test ever
/// shells out to the real thing. `#[cfg(unix)]`, same as `tests/claude_plugin.rs`: the shim is
/// a shell script, and these are the only tests in this file that need one — the file/manifest
/// checks above run everywhere and skip without `bash`.
#[cfg(unix)]
mod claude_reinstall {
    use super::*;
    use common::agents::{claude_log, fake_claude_path, json, raw, rtok, slash, tmp, write_cfg};

    /// `known_marketplaces.json` pointing `rtok` at `source_json` (the `"source"` object's body),
    /// and `installed_plugins.json` recording `rtok@rtok` at `version` — a legacy host record with
    /// no `.rtok-plugin-version` and no receipt, exactly what every install before T279 left
    /// (`seed_claude_plugin` in `tests/agents_update.rs` covers the same ground for the older,
    /// receipt-less T242.3 tests; this one also seeds the marketplace shape so the T279 source/
    /// stale-repoint decisions have something to read).
    fn seed(home: &Path, source_json: &str, version: &str) {
        let dir = home.join(".claude/plugins");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("known_marketplaces.json"),
            format!(r#"{{"rtok":{{"source":{source_json}}}}}"#),
        )
        .unwrap();
        fs::write(
        dir.join("installed_plugins.json"),
        format!(
            r#"{{"version":2,"plugins":{{"rtok@rtok":[{{"scope":"user","version":"{version}"}}]}}}}"#
        ),
    )
    .unwrap();
    }

    /// `known_marketplaces.json`'s `"source"` body for the GitHub marketplace `plugin_against`
    /// treats as current (T139).
    const GITHUB_SOURCE: &str = r#"{"source":"github","repo":"listepo/rtok"}"#;

    /// Where the T279 receipt lands for a `home` these tests redirected `HOME`/`XDG_STATE_HOME`/
    /// `LOCALAPPDATA` into (`common::agents::raw_with_path` pins all three so a real value on the
    /// test machine never leaks in) — the same OS branches as
    /// `agents::plugin_version::default_receipt_path`, which is private to the production crate.
    fn receipt_path(home: &Path) -> PathBuf {
        if cfg!(target_os = "macos") {
            home.join("Library/Application Support/rtok/plugins.json")
        } else if cfg!(target_os = "windows") {
            home.join("AppData/Local/rtok/plugins.json")
        } else {
            home.join(".local/state/rtok/plugins.json")
        }
    }

    /// A throwaway home with the fake `claude` first on `PATH` and rtok's own config pointed at
    /// it, ready for [`seed`].
    fn home_with_fake_claude(name: &str) -> (PathBuf, PathBuf) {
        let home = tmp(name);
        let cfg = write_cfg(&home);
        fake_claude_path(&home); // installs the fake `claude`/`codex`/`copilot` shims under `home`
        (home, cfg)
    }

    /// Older installed → `plugin marketplace update` + `plugin update`, and the T279 receipt is
    /// written at the running version; a rerun then finds the receipt already current and calls
    /// `claude` for nothing (T279 step 3, plan check: "a second run prints `up to date` and runs
    /// no `claude` command").
    #[test]
    fn update_writes_the_receipt_then_a_rerun_skips_with_no_claude_call() {
        let (home, cfg) = home_with_fake_claude("pv-up-to-date");
        seed(&home, GITHUB_SOURCE, "0.1.0");

        let out = rtok(&["agents", "update", "claude", "--cli"], &cfg, &home);
        assert!(out.contains("~ plugin rtok@rtok updated"), "{out}");
        assert_eq!(
            claude_log(&home),
            "plugin marketplace update rtok\nplugin update rtok@rtok\n"
        );
        let receipt = json(&receipt_path(&home));
        assert_eq!(receipt["claude"]["source"], "github");
        assert_eq!(receipt["claude"]["version"], env!("CARGO_PKG_VERSION"));

        let before = claude_log(&home);
        let again = rtok(&["agents", "update", "claude", "--cli"], &cfg, &home);
        assert!(again.contains("already current"), "{again}");
        assert_eq!(claude_log(&home), before, "up to date calls no claude");
    }

    /// `--force` reinstalls — uninstall then install — even though the receipt is already at the
    /// running version, and never calls `plugin update` (the decision function is bypassed
    /// entirely, plan T279 step 6).
    #[test]
    fn force_reinstalls_even_when_already_up_to_date() {
        let (home, cfg) = home_with_fake_claude("pv-force");
        seed(&home, GITHUB_SOURCE, env!("CARGO_PKG_VERSION"));

        let out = rtok(
            &["agents", "update", "claude", "--cli", "--force"],
            &cfg,
            &home,
        );
        assert!(out.contains("+ plugin"), "{out}");
        let log = claude_log(&home);
        assert_eq!(
            log,
            "plugin uninstall rtok@rtok\nplugin install rtok@rtok\n"
        );
        assert_eq!(
            json(&receipt_path(&home))["claude"]["version"],
            env!("CARGO_PKG_VERSION")
        );
    }

    /// A stale ketch-store marketplace (the pre-T139 local path, not GitHub) is repointed on a
    /// plain `agents update`, no `--force` needed: `plugin_update`'s own stale check feeds the
    /// same `true` into `decide` that `--force` would.
    #[test]
    fn a_stale_local_marketplace_is_repointed_back_to_github_on_update() {
        let (home, cfg) = home_with_fake_claude("pv-stale-marketplace");
        seed(
            &home,
            r#"{"source":"directory","path":"/old/store/rtok/v0.1.0/plugins/claude"}"#,
            "0.1.0",
        );

        let out = rtok(&["agents", "update", "claude", "--cli"], &cfg, &home);
        assert!(out.contains("+ plugin"), "{out}");
        assert_eq!(
            claude_log(&home),
            "plugin uninstall rtok@rtok\nplugin marketplace remove rtok\n\
         plugin marketplace add listepo/rtok\nplugin install rtok@rtok\n"
        );
        assert_eq!(json(&receipt_path(&home))["claude"]["source"], "github");
    }

    /// `--source local` reinstalls from the local checkout tree and switches the receipt over —
    /// a source change alone forces a reinstall (`plugin_version::decide`), no `--force` needed.
    #[test]
    fn source_local_reinstalls_from_the_checkout_and_switches_the_receipt_source() {
        let (home, cfg) = home_with_fake_claude("pv-source-local");
        seed(&home, GITHUB_SOURCE, "0.1.0");

        let out = rtok(
            &["agents", "update", "claude", "--cli", "--source", "local"],
            &cfg,
            &home,
        );
        assert!(out.contains("+ plugin"), "{out}");
        let log = slash(claude_log(&home));
        assert!(
            log.lines()
                .any(|l| l.starts_with("plugin marketplace add ") && l.ends_with("plugins/claude")),
            "{log}"
        );
        assert_eq!(json(&receipt_path(&home))["claude"]["source"], "local");
    }

    /// T279 step 3/6 "Failure": a forced reinstall whose uninstall succeeds and whose install then
    /// fails reports it plainly, deletes the receipt row (so the next `agents update` treats the
    /// host as not installed), and `rtok agents update` exits non-zero — unlike every other
    /// `claude` failure, which keeps the old copy and stays a non-fatal `offer …`/`~ …` line.
    #[test]
    fn a_failed_forced_reinstall_after_a_successful_uninstall_deletes_the_receipt_and_exits_non_zero()
     {
        let (home, cfg) = home_with_fake_claude("pv-reinstall-fail");
        seed(&home, GITHUB_SOURCE, "0.1.0");
        // A normal update first, so there is a receipt row to lose.
        rtok(&["agents", "update", "claude", "--cli"], &cfg, &home);
        assert_eq!(json(&receipt_path(&home))["claude"]["source"], "github");

        fs::write(home.join("fake-claude-fail-install"), "").unwrap();
        let out = raw(
            &["agents", "update", "claude", "--cli", "--force"],
            &cfg,
            &home,
        );
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(
            !out.status.success(),
            "exit code {:?}: {stdout}",
            out.status.code()
        );
        assert!(
            stdout.contains("plugin rtok@rtok removed, reinstall failed: install failed"),
            "{stdout}"
        );
        assert!(
            json(&receipt_path(&home)).get("claude").is_none(),
            "the receipt row must be gone: {}",
            json(&receipt_path(&home))
        );

        // A failure before anything was removed (no `--force`: `installed_plugins.json` is gone,
        // so the plain path has nothing to uninstall and goes straight to `install`) keeps
        // today's `offer …` line instead, and never touches the receipt (there is none to touch).
        let plain = raw(&["agents", "update", "claude", "--cli"], &cfg, &home);
        let plain_out = String::from_utf8_lossy(&plain.stdout).into_owned();
        assert!(plain.status.success(), "{plain_out}");
        assert!(plain_out.contains("offer plugins/claude"), "{plain_out}");
        assert!(
            plain_out.contains("claude failed: install failed"),
            "{plain_out}"
        );

        // The fake CLI healthy again: the next update finds nothing installed and installs fresh.
        fs::remove_file(home.join("fake-claude-fail-install")).unwrap();
        let again = rtok(&["agents", "update", "claude", "--cli"], &cfg, &home);
        assert!(again.contains("+ plugin"), "{again}");
    }
} // mod claude_reinstall

// ── T279.1: `rtok agents outdated` ───────────────────────────────────────────

fn target() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Receipt path the binary reads for this `HOME` (`plugin_version::default_receipt_path`).
fn receipt_file(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/rtok/plugins.json")
    } else if cfg!(target_os = "windows") {
        home.join("AppData/Local/rtok/plugins.json")
    } else {
        home.join(".local/state/rtok/plugins.json")
    }
}

fn cfg_with_receipt(home: &Path) -> PathBuf {
    write_cfg(home)
}

/// `path` as the inside of a JSON string literal: a Windows path's `\` separators must be
/// escaped, or the receipt/host JSON the binary reads is invalid and silently ignored.
fn js(path: &Path) -> String {
    let quoted = serde_json::to_string(&path.display().to_string()).unwrap();
    quoted[1..quoted.len() - 1].to_string()
}

fn claude_plugin_installed(home: &Path, version: &str, install_path: Option<&Path>) {
    let dir = home.join(".claude/plugins");
    fs::create_dir_all(&dir).unwrap();
    let install = install_path
        .map(js)
        .unwrap_or_else(|| js(&home.join(".claude/plugins/cache/rtok/rtok/0.0.1")));
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
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
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
            js(&install),
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
            js(&home.join(".claude/plugins/cache/rtok/rtok/0.0.1")),
            js(&cursor_install),
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
            js(&install),
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
