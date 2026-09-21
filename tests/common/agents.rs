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

/// The invoking user's own config at home-relative `rel` (`.cursor/hooks.json`), or `None`
/// when this machine has no such file — or when `CI` is set, so a runner that happens to
/// carry one skips anyway.
///
/// Tests built on this are local-only by construction: a developer who actually runs the host
/// checks rtok against the file it will really meet, and everyone else — CI included — skips
/// instead of failing. Read from the test process's own `HOME`, which [`raw`] never changes:
/// only the child binary is redirected into the throwaway home.
pub fn real_config(rel: &str) -> Option<PathBuf> {
    real_config_from(
        std::env::var_os("CI").is_some(),
        real_home().as_deref(),
        rel,
    )
}

/// This machine's home as the *test process* sees it.
pub fn real_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// The gate with its two inputs handed in, so a test can exercise both sides without setting
/// `CI` — `unsafe` is denied in this crate, and a mutated environment leaks across threads.
pub fn real_config_from(ci: bool, home: Option<&Path>, rel: &str) -> Option<PathBuf> {
    if ci {
        return None;
    }
    let path = home?.join(rel);
    path.is_file().then_some(path)
}

/// Copy [`real_config`] into the throwaway `home` at the same relative path, so the binary
/// edits a copy and the user's own file is never opened for writing.
pub fn seed_real(home: &Path, rel: &str) -> Option<PathBuf> {
    let src = real_config(rel)?;
    let dest = home.join(rel);
    fs::create_dir_all(dest.parent().expect("rel has a parent")).unwrap();
    fs::copy(&src, &dest).unwrap();
    Some(dest)
}

/// One line saying which check did not run here — a skip must be visible, not silent.
pub fn skip(what: &str) {
    println!("skipped {what}: no such config under this HOME, or CI is set");
}

/// One config pointing every host at files inside `home`; the host dirs exist so `present`
/// says yes for each of them.
pub fn write_cfg(home: &Path) -> PathBuf {
    for sub in [
        ".claude",
        ".cursor",
        ".codex",
        ".config/opencode",
        ".config/kilo",
        ".pi/agent",
        ".zcode/cli",
        ".kimi-code",
        "Library/Application Support/Code/User",
        "Library/Application Support/Code - Insiders/User",
        ".copilot/hooks",
        ".codeium/windsurf",
        ".config/zed",
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
             [setup.kilo]\nconfig_path = \"{h}/.config/kilo/kilo.json\"\n\
             [setup.pi]\nextensions_path = \"{h}/.pi/agent/extensions\"\n\
             [setup.zcode]\nconfig_path = \"{h}/.zcode/cli/config.json\"\n\
             [setup.kimi]\nconfig_path = \"{h}/.kimi-code/config.toml\"\n\
             [setup.vscode]\ncode_user_dir = \"{h}/Library/Application Support/Code/User\"\n\
             insiders_user_dir = \"{h}/Library/Application Support/Code - Insiders/User\"\n\
             [setup.copilot]\ndir = \"{h}/.copilot\"\n\
             [setup.aider]\nconfig_path = \"{h}/.aider.conf.yml\"\n\
              [setup.windsurf]\nconfig_path = \"{h}/.codeium/windsurf/mcp_config.json\"\n\
              [setup.zed]\nconfig_path = \"{h}/.config/zed/settings.json\"\n"
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
        .env("PATH", fake_claude_path(home))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("RTOK_HOME", home.join(".rtok"))
        .output()
        .expect("rtok")
}

/// A fake `claude` (T115) first on PATH, so no test ever runs the real CLI: it answers the
/// detection probe (`--version`), appends every other argv to `<home>/claude.log` and keeps `<config dir>/plugins/installed_plugins.json` the way
/// `claude plugin install` / `uninstall` do. Unix only; elsewhere PATH is left as it is.
pub fn fake_claude_path(home: &Path) -> std::ffi::OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = home.join(".fake-bin");
        let exe = dir.join("claude");
        if !exe.exists() {
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                &exe,
                r#"#!/bin/sh
[ "$1" = --version ] && { echo "2.0.0 (Claude Code)"; exit 0; }
echo "$*" >> "$HOME/claude.log"
plugins="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/plugins"
case "$*" in
  "plugin install rtok@rtok") mkdir -p "$plugins"
    printf '{"version":2,"plugins":{"rtok@rtok":[{"scope":"user"}]}}' > "$plugins/installed_plugins.json" ;;
  "plugin uninstall rtok@rtok") rm -f "$plugins/installed_plugins.json" ;;
esac
"#,
            )
            .unwrap();
            fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut dirs = vec![dir];
        dirs.extend(std::env::split_paths(&path));
        return std::env::join_paths(dirs).unwrap();
    }
    #[allow(unreachable_code)]
    path
}

/// The fake `claude`'s calls so far, one argv per line.
pub fn claude_log(home: &Path) -> String {
    fs::read_to_string(home.join("claude.log")).unwrap_or_default()
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

/// The copies of `path` in its sibling `_backup/` directory, oldest name first.
pub fn backups(path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let dir = parent.join("_backup");
    if !dir.is_dir() {
        return Vec::new();
    }
    let name = format!("{}.bak-", path.file_name().unwrap().to_string_lossy());
    let mut found: Vec<PathBuf> = fs::read_dir(&dir)
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
