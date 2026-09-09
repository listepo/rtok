//! T10.6 + D21: pi host plugin is one bash call path, no MCP, ketch if missing.
//!
//! Check: `rtok agent setup pi --dry-run` names `plugins/pi` and
//! `ketch install listepo/rtok` and touches nothing; `--yes` links the
//! extension, second apply is `no changes`, `--remove` unlinks; the TS
//! extension owns the single bash call path with no `read`/`search`
//! duplication.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins/pi")
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtok-t106-{name}-{}-{}",
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
    let cfg = home.join("config.toml");
    fs::write(
        &cfg,
        format!(
            "[setup.pi]\nextensions_path = \"{}/extensions\"\n",
            home.display()
        ),
    )
    .unwrap();
    cfg
}

fn setup(args: &[&str], cfg: &Path, home: &Path) -> (String, String, i32) {
    let out = Command::new(bin())
        .args(["--config", cfg.to_str().unwrap()])
        .args(args)
        .env("HOME", home)
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
fn pi_package_is_extension_and_skill_without_tools() {
    let dir = root();
    let pkg: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
    let exts = pkg["pi"]["extensions"].as_array().expect("pi.extensions");
    assert_eq!(exts.len(), 1, "one extension entry");
    assert!(exts[0].as_str().unwrap().ends_with("rtok.ts"), "{exts:?}");
    assert!(dir.join("extensions/rtok.ts").is_file());
    assert!(dir.join("skills/rtok/SKILL.md").is_file());
}

#[test]
fn pi_extension_owns_the_single_bash_call_path() {
    let ts = fs::read_to_string(root().join("extensions/rtok.ts")).unwrap();
    assert!(ts.contains("\"tool_call\""), "rewrites bash calls");
    assert!(ts.contains("\"bash\""), "scoped to bash");
    assert!(ts.contains("rtok run --"), "rewrite target");
    assert!(ts.contains("\"tool_result\""), "compresses results");
    assert!(ts.contains("rtok filter"), "filter path");
    assert!(ts.contains("expand"), "expand trailer");
    assert!(ts.contains("ketch install listepo/rtok"), "ketch hint");
    for dup in [
        "registerTool",
        "\"read\"",
        "\"search\"",
        "rtok read",
        "rtok search",
    ] {
        assert!(!ts.contains(dup), "no duplicate call path: {dup}");
    }
}

#[test]
fn setup_pi_dry_run_offers_plugin() {
    let home = tmp("dry");
    let cfg = write_cfg(&home);
    let (stdout, stderr, code) = setup(&["agent", "setup", "pi", "--dry-run"], &cfg, &home);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("plugins/pi"), "stdout={stdout}");
    assert!(
        stdout.contains("ketch install listepo/rtok"),
        "stdout={stdout}"
    );
    assert!(
        !home.join("extensions/rtok").exists(),
        "dry-run must not link"
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn setup_pi_yes_links_remove_unlinks() {
    let home = tmp("yes");
    let cfg = write_cfg(&home);
    let (stdout, stderr, code) = setup(&["agent", "setup", "pi", "--yes"], &cfg, &home);
    assert_eq!(code, 0, "stderr={stderr} stdout={stdout}");
    let dest = home.join("extensions/rtok");
    let meta = fs::symlink_metadata(&dest).unwrap_or_else(|e| panic!("{}: {e}", dest.display()));
    assert!(meta.file_type().is_symlink() || dest.is_dir(), "{dest:?}");
    let (again, stderr2, code2) = setup(&["agent", "setup", "pi", "--yes"], &cfg, &home);
    assert_eq!(code2, 0, "stderr={stderr2}");
    assert!(again.contains("no changes"), "second apply: {again}");
    let (rm, stderr3, code3) = setup(&["agent", "setup", "pi", "--remove"], &cfg, &home);
    assert_eq!(code3, 0, "stderr={stderr3}");
    assert!(!dest.exists(), "remove must unlink; stdout={rm}");
    let _ = fs::remove_dir_all(&home);
}
