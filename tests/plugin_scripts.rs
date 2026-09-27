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
