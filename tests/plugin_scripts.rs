// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T197: every file under `plugins/*/scripts/` must be reachable — referenced by a
//! manifest/hooks file in its own tree, or allowlisted by name in that tree's
//! `README.md`.
//!
//! Wire-vs-delete decision (documented here and in the READMEs): Cursor's MCP
//! launchers were deleted first — `plugins/cursor/mcp.json` spawned `rtok mcp`
//! directly through a single `command`/`args` pair with no per-OS slot, so no
//! launcher could ever run (the T85/I-37 decision; Kimi is the precedent: the
//! ketch hint lives in the README). T275/D33 then dropped MCP from every
//! plugin except Gemini's, so ZCode's `scripts/mcp.sh` and `scripts/mcp.cmd`
//! (and its `.mcp.json`) went with it — `mcp.servers.rtok` is now written by
//! `rtok agents install zcode` directly, plugin linked or not; only
//! `scripts/hook.sh` remains.

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

/// T159: with no rtok anywhere, `claude/scripts/worktree.sh` makes exactly the host's own
/// default for `foo` — `.claude/worktrees/foo` on `worktree-foo`. A POSIX `sed` ends the
/// `field` output with a newline, which `tr -c` once turned into a trailing `-`; the shim
/// stands in for such a `sed` (the stock one on macOS and GNU sed drop the newline).
#[cfg(unix)]
#[test]
fn worktree_launcher_fallback_names_the_worktree_exactly_as_the_host_does() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::{Command, Stdio};

    let tmp = rtok::testutil::tmp_dir("worktree-launcher-name");
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(tmp.join("repo"))
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {:?}", out.stderr);
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    fs::create_dir_all(tmp.join("repo")).unwrap();
    git(&["init", "-q"]);
    git(&["commit", "-q", "--allow-empty", "-m", "init"]);

    let shim = tmp.join("bin");
    fs::create_dir_all(&shim).unwrap();
    let sed = shim.join("sed");
    fs::write(
        &sed,
        "#!/bin/sh\n/usr/bin/sed \"$@\" | /usr/bin/awk '{print}'\n",
    )
    .unwrap();
    fs::set_permissions(&sed, fs::Permissions::from_mode(0o755)).unwrap();

    let script = plugins().join("claude/scripts/worktree.sh");
    let payload = serde_json::json!({"session_id": "s", "cwd": tmp.join("repo"), "name": "foo"});
    let mut child = Command::new("/bin/sh")
        .arg(&script)
        .arg("WorktreeCreate")
        .env("HOME", &tmp)
        .env("PATH", format!("{}:/usr/bin:/bin", shim.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    { stdin }.write_all(payload.to_string().as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let want = tmp.join("repo/.claude/worktrees/foo");
    let printed = String::from_utf8(out.stdout).unwrap();
    let printed = fs::canonicalize(printed.trim_end()).expect("the printed path exists");
    assert_eq!(printed, fs::canonicalize(&want).unwrap());
    let branch = git(&[
        "-C",
        want.to_str().unwrap(),
        "rev-parse",
        "--abbrev-ref",
        "HEAD",
    ]);
    assert_eq!(branch, "worktree-foo");
    let _ = fs::remove_dir_all(&tmp);
}
