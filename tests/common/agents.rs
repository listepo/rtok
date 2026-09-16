//! A throwaway home with every host's files under it, and the binary run against it.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

/// A fresh empty directory, unique per test and process.
pub fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtok-agents-{name}-{}-{}",
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

/// One config pointing every host at files inside `home`; the host dirs exist so `present`
/// says yes for each of them.
pub fn write_cfg(home: &Path) -> PathBuf {
    for sub in [
        ".claude",
        ".cursor",
        ".codex",
        ".config/opencode",
        ".pi/agent",
        ".zcode/cli",
        ".kimi-code",
        ".copilot/hooks",
    ] {
        fs::create_dir_all(home.join(sub)).unwrap();
    }
    let cfg = home.join("config.toml");
    let h = home.display().to_string().replace('\\', "/");
    fs::write(
        &cfg,
        format!(
            "[doctor]\nclaude_json = \"{h}/.claude.json\"\n\
             [setup.claude]\nsettings_path = \"{h}/.claude/settings.json\"\n\
             [setup.cursor]\nhooks_path = \"{h}/.cursor/hooks.json\"\n\
             [setup.codex]\nconfig_path = \"{h}/.codex/config.toml\"\n\
             [setup.opencode]\nconfig_path = \"{h}/.config/opencode/opencode.json\"\n\
             [setup.pi]\nextensions_path = \"{h}/.pi/agent/extensions\"\n\
             [setup.zcode]\nconfig_path = \"{h}/.zcode/cli/config.json\"\n\
             [setup.kimi]\nconfig_path = \"{h}/.kimi-code/config.toml\"\n\
             [setup.copilot]\ndir = \"{h}/.copilot\"\n"
        ),
    )
    .unwrap();
    cfg
}

/// `rtok --config <cfg> <args>` with `home` as HOME, USERPROFILE and APPDATA, so every
/// platform's home-relative path lands inside the temp dir.
pub fn raw(args: &[&str], cfg: &Path, home: &Path) -> Output {
    Command::new(bin())
        .args(["--config", cfg.to_str().unwrap()])
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("RTOK_HOME", home.join(".rtok"))
        .output()
        .expect("rtok")
}

/// [`raw`] that must succeed; returns stdout.
pub fn rtok(args: &[&str], cfg: &Path, home: &Path) -> String {
    let out = raw(args, cfg, home);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "rtok {args:?} failed: {stderr}\n{stdout}"
    );
    stdout
}

pub fn json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The `.bak-*` copies sitting beside `path`, oldest name first.
pub fn backups(path: &Path) -> Vec<PathBuf> {
    let name = format!("{}.bak-", path.file_name().unwrap().to_string_lossy());
    let mut found: Vec<PathBuf> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&name))
        .collect();
    found.sort();
    found
}

/// Where Claude Desktop's config lands under `home` on this platform (the binary's rule).
pub fn claude_desktop_config(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Claude/claude_desktop_config.json")
    } else if cfg!(windows) {
        home.join("Claude/claude_desktop_config.json")
    } else {
        home.join(".config/Claude/claude_desktop_config.json")
    }
}
