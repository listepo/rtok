//! T197: every file under `plugins/*/scripts/` must be reachable — referenced by a
//! manifest/hooks file in its own tree, or allowlisted by name in that tree's
//! `README.md` — and `plugins/zcode/scripts/mcp.cmd` must forward `rtok mcp`'s
//! exit code instead of masking it to 0.
//!
//! Wire-vs-delete decision (documented here and in the READMEs): Cursor's
//! launchers are deleted — `plugins/cursor/mcp.json` spawns `rtok mcp`
//! directly through a single `command`/`args` pair with no per-OS slot, so no
//! launcher could ever run (the T85/I-37 decision; Kimi is the precedent: the
//! ketch hint lives in the README). ZCode's launchers are kept —
//! `plugins/zcode/.mcp.json` wires `scripts/mcp.sh` via
//! `${ZCODE_PLUGIN_ROOT}`, and `scripts/mcp.cmd` stays as the
//! README-allowlisted Windows counterpart (`.mcp.json` has one `command` slot
//! and no per-OS branch, so on Windows the config-file install carrying the
//! absolute `rtok mcp` is the path).

use std::fs;
use std::path::PathBuf;

fn plugins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins")
}

/// Every script must be named by some manifest/hooks file in its tree (a
/// `.json` under `plugins/<host>/` containing the file name) or by that
/// tree's `README.md` (the allowlist for platform counterparts no manifest
/// slot can reference, e.g. zcode's `mcp.cmd`).
#[test]
fn every_plugin_script_is_referenced_or_readme_allowlisted() {
    let mut unreferenced = Vec::new();
    let mut hosts: Vec<PathBuf> = fs::read_dir(plugins())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    hosts.sort();
    assert!(!hosts.is_empty(), "no hosts under plugins/");
    for host in hosts {
        let scripts = host.join("scripts");
        if !scripts.is_dir() {
            continue;
        }
        let mut files: Vec<PathBuf> = fs::read_dir(&scripts)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        // All manifest/hooks text plus the README text of this tree.
        let mut tree_text = String::new();
        for entry in ignore::WalkBuilder::new(&host).build() {
            let path = entry.unwrap().into_path();
            if !path.is_file() {
                continue;
            }
            let is_manifest = path.extension().is_some_and(|e| e == "json");
            let is_readme = path.file_name().is_some_and(|n| n == "README.md");
            if is_manifest || is_readme {
                tree_text.push_str(&fs::read_to_string(&path).unwrap_or_default());
                tree_text.push('\n');
            }
        }
        for file in files {
            let name = file.file_name().unwrap().to_str().unwrap().to_string();
            if !tree_text.contains(&name) {
                unreferenced.push(file);
            }
        }
    }
    assert!(
        unreferenced.is_empty(),
        "scripts no manifest/hooks file references and no README allowlists: {unreferenced:?}"
    );
}

/// T197.1 (static, runs everywhere): `mcp.cmd` must forward `rtok mcp`'s exit
/// code with a bare `exit /b`. `exit /b %ERRORLEVEL%` inside the parenthesized
/// `if` block expands at parse time, so every failure exited 0.
#[test]
fn zcode_mcp_cmd_forwards_the_exit_code() {
    let text = fs::read_to_string(plugins().join("zcode/scripts/mcp.cmd")).expect("zcode mcp.cmd");
    assert!(
        !text.contains("exit /b %"),
        "parse-time %ERRORLEVEL% masks failures to 0: {text}"
    );
    assert!(
        text.lines().any(|l| l.trim() == "exit /b"),
        "want a bare `exit /b` after each `rtok mcp` call: {text}"
    );
}

/// T197 check, Windows runtime, mirroring `d21_missing_rtok_names_ketch_cmd`:
/// a stub `rtok` exiting 7 → `mcp.cmd` exits 7; a missing `rtok` still exits 1
/// with the ketch hint. Written here so Windows CI executes it; macOS/Linux
/// can only run the static test above.
#[cfg(windows)]
#[test]
fn zcode_mcp_cmd_preserves_failure_and_names_ketch_when_missing() {
    use std::process::Command;

    let script = plugins().join("zcode/scripts/mcp.cmd");
    let dir = std::env::temp_dir().join(format!(
        "rtok-t197-zcode-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // Stub `rtok` that fails loudly: `mcp.cmd` must exit with its code.
    fs::write(dir.join("rtok.bat"), "@echo off\r\nexit /b 7\r\n").unwrap();
    let path = format!("{};C:\\Windows\\System32", dir.display());
    let out = Command::new("cmd")
        .args(["/C", script.to_str().unwrap()])
        .env_clear()
        .env("PATH", &path)
        .output()
        .expect("mcp.cmd with stub rtok");
    assert_eq!(
        out.status.code(),
        Some(7),
        "a failing `rtok mcp` must not exit 0: {out:?}"
    );

    // No `rtok` anywhere: exit 1 with the ketch hint (D21).
    let out = Command::new("cmd")
        .args(["/C", script.to_str().unwrap()])
        .env_clear()
        .env("PATH", "C:\\Windows\\System32")
        .output()
        .expect("mcp.cmd without rtok");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("ketch install listepo/rtok"),
        "want ketch install, got {err}"
    );
    assert!(err.contains("rtok is not installed"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}
