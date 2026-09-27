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
