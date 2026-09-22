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
        "Documents/Cline/Hooks",
        ".cline/data/settings",
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
              [setup.zed]\nconfig_path = \"{h}/.config/zed/settings.json\"\n\
             [setup.cline]\nhooks_path = \"{h}/Documents/Cline/Hooks\"\n\
             mcp_path = \"{h}/.cline/data/settings/cline_mcp_settings.json\"\n"
        ),
    )
    .unwrap();
    cfg
}

/// `rtok --config <cfg> <args>` with `home` as HOME, USERPROFILE and APPDATA, so every
/// platform's home-relative path lands inside the temp dir.
///
/// `agents install|remove` calls get `--no-restart` appended (T141): these tests must never
/// shell out to a real `osascript`/`pgrep`/`tasklist` to probe whether some app on the test
/// machine happens to be running, let alone quit or reopen one.
pub fn raw(args: &[&str], cfg: &Path, home: &Path) -> Output {
    raw_with_path(args, cfg, home, fake_claude_path(home))
}

/// [`raw`], but on a PATH with no `claude` at all (fake or real) — a machine that never
/// installed the Claude Code CLI, so `rtok agents install claude` falls back to the
/// settings-file surfaces instead of the plugin (T139).
pub fn raw_without_claude(args: &[&str], cfg: &Path, home: &Path) -> Output {
    let path = if cfg!(windows) {
        std::ffi::OsString::from(r"C:\Windows\System32")
    } else {
        std::ffi::OsString::from("/usr/bin:/bin")
    };
    raw_with_path(args, cfg, home, path)
}

fn raw_with_path(args: &[&str], cfg: &Path, home: &Path, path: std::ffi::OsString) -> Output {
    let mut full: Vec<&str> = args.to_vec();
    if matches!(args, ["agents", "install" | "remove", ..]) {
        full.push("--no-restart");
    }
    Command::new(bin())
        .args(["--config", cfg.to_str().unwrap()])
        .args(&full)
        .env("PATH", path)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("RTOK_HOME", home.join(".rtok"))
        .output()
        .expect("rtok")
}

/// A fake `claude` (T115) and a fake `codex` (T140) first on PATH, so no test ever runs
/// either real CLI: `claude` answers the detection probe (`--version`), appends every other
/// argv to `<home>/claude.log` and keeps `<config dir>/plugins/installed_plugins.json` the way
/// `claude plugin install` / `uninstall` do; `codex` answers `--version` and edits
/// `${CODEX_HOME:-$HOME/.codex}/config.toml`'s `[marketplaces.rtok]` / `[plugins."rtok@rtok"]`
/// tables the way `codex plugin marketplace add|remove` / `plugin add|remove` do, including the
/// real CLI's "already added from a different source" error on a second `marketplace add` with
/// a different source. A shell script on Unix; on Windows a `.cmd` shim (the same shape npm
/// installs the real CLI as), which `agents::run_cli`'s `cmd /C` wrapper (T139 windows fix)
/// resolves the way it resolves the real thing.
pub fn fake_claude_path(home: &Path) -> std::ffi::OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = home.join(".fake-bin");
        fs::create_dir_all(&dir).unwrap();
        let claude = dir.join("claude");
        if !claude.exists() {
            fs::write(
                &claude,
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
            fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let codex = dir.join("codex");
        if !codex.exists() {
            fs::write(&codex, FAKE_CODEX_SH).unwrap();
            fs::set_permissions(&codex, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut dirs = vec![dir];
        dirs.extend(std::env::split_paths(&path));
        return std::env::join_paths(dirs).unwrap();
    }
    #[cfg(windows)]
    {
        let dir = home.join(".fake-bin");
        fs::create_dir_all(&dir).unwrap();
        let claude = dir.join("claude.cmd");
        if !claude.exists() {
            fs::write(
                &claude,
                r#"@echo off
if "%~1"=="--version" (
  echo 2.0.0 Claude Code
  exit /b 0
)
set "ALLARGS=%*"
echo %ALLARGS%>>"%HOME%\claude.log"
if defined CLAUDE_CONFIG_DIR (
  set "PLUGINS=%CLAUDE_CONFIG_DIR%\plugins"
) else (
  set "PLUGINS=%HOME%\.claude\plugins"
)
if "%ALLARGS%"=="plugin install rtok@rtok" (
  mkdir "%PLUGINS%" 2>nul
  >"%PLUGINS%\installed_plugins.json" echo {"version":2,"plugins":{"rtok@rtok":[{"scope":"user"}]}}
)
if "%ALLARGS%"=="plugin uninstall rtok@rtok" (
  del /f /q "%PLUGINS%\installed_plugins.json" 2>nul
)
"#,
            )
            .unwrap();
        }
        let codex = dir.join("codex.cmd");
        if !codex.exists() {
            fs::write(&codex, FAKE_CODEX_CMD).unwrap();
        }
        let mut dirs = vec![dir];
        dirs.extend(std::env::split_paths(&path));
        return std::env::join_paths(dirs).unwrap();
    }
    #[allow(unreachable_code)]
    path
}

/// `codex`'s config: `${CODEX_HOME:-$HOME/.codex}/config.toml` (matches `agents::codex::plugin`'s
/// `CODEX_HOME` override, which is only set when `config_path` is not the default).
#[cfg(unix)]
const FAKE_CODEX_SH: &str = r#"#!/bin/sh
[ "$1" = --version ] && { echo "codex-cli 0.155.1"; exit 0; }
echo "$*" >> "$HOME/codex.log"
cfg="${CODEX_HOME:-$HOME/.codex}/config.toml"
mkdir -p "$(dirname "$cfg")"
touch "$cfg"
case "$*" in
  "plugin marketplace add listepo/rtok")
    if grep -q '^source = "https://github.com/listepo/rtok.git"$' "$cfg" 2>/dev/null; then
      exit 0
    fi
    if grep -q '^\[marketplaces.rtok\]$' "$cfg" 2>/dev/null; then
      echo "rtok: already added from a different source" >&2
      exit 1
    fi
    printf '\n[marketplaces.rtok]\nsource_type = "git"\nsource = "https://github.com/listepo/rtok.git"\n' >> "$cfg"
    ;;
  "plugin marketplace remove rtok")
    grep -q '^\[marketplaces.rtok\]$' "$cfg" 2>/dev/null || { echo "rtok: no such marketplace" >&2; exit 1; }
    awk '/^\[marketplaces\.rtok\]$/{skip=1;next} /^\[/{skip=0} !skip' "$cfg" > "$cfg.tmp" && mv "$cfg.tmp" "$cfg"
    ;;
  "plugin add rtok@rtok")
    grep -q '^\[plugins\."rtok@rtok"\]$' "$cfg" 2>/dev/null || printf '\n[plugins."rtok@rtok"]\nenabled = true\n' >> "$cfg"
    ;;
  "plugin remove rtok@rtok")
    awk '/^\[plugins\."rtok@rtok"\]$/{skip=1;next} /^\[/{skip=0} !skip' "$cfg" > "$cfg.tmp" && mv "$cfg.tmp" "$cfg"
    ;;
esac
"#;

/// [`FAKE_CODEX_SH`]'s Windows counterpart.
#[cfg(windows)]
const FAKE_CODEX_CMD: &str = r#"@echo off
if "%~1"=="--version" (
  echo codex-cli 0.155.1
  exit /b 0
)
if defined CODEX_HOME (set "CFG=%CODEX_HOME%\config.toml") else (set "CFG=%HOME%\.codex\config.toml")
for %%F in ("%CFG%") do if not exist "%%~dpF" mkdir "%%~dpF"
type nul >> "%CFG%"
set "ALLARGS=%*"
echo %ALLARGS%>>"%HOME%\codex.log"
if "%ALLARGS%"=="plugin marketplace add listepo/rtok" (
  findstr /c:"source = \"https://github.com/listepo/rtok.git\"" "%CFG%" >nul 2>&1 && exit /b 0
  findstr /c:"[marketplaces.rtok]" "%CFG%" >nul 2>&1 && (echo rtok: already added from a different source 1>&2 & exit /b 1)
  >>"%CFG%" echo([marketplaces.rtok]
  >>"%CFG%" echo source_type = "git"
  >>"%CFG%" echo source = "https://github.com/listepo/rtok.git"
)
if "%ALLARGS%"=="plugin marketplace remove rtok" (
  findstr /c:"[marketplaces.rtok]" "%CFG%" >nul 2>&1 || (echo rtok: no such marketplace 1>&2 & exit /b 1)
  findstr /v /c:"[marketplaces.rtok]" /c:"source_type = \"git\"" /c:"source = \"https://github.com/listepo/rtok.git\"" "%CFG%" > "%CFG%.tmp"
  move /y "%CFG%.tmp" "%CFG%" >nul
)
if "%ALLARGS%"=="plugin add rtok@rtok" (
  findstr /c:"[plugins.\"rtok@rtok\"]" "%CFG%" >nul 2>&1 || (
    >>"%CFG%" echo([plugins."rtok@rtok"]
    >>"%CFG%" echo enabled = true
  )
)
if "%ALLARGS%"=="plugin remove rtok@rtok" (
  findstr /v /c:"[plugins.\"rtok@rtok\"]" /c:"enabled = true" "%CFG%" > "%CFG%.tmp"
  move /y "%CFG%.tmp" "%CFG%" >nul
)
"#;

/// The fake `claude`'s calls so far, one argv per line.
pub fn claude_log(home: &Path) -> String {
    fs::read_to_string(home.join("claude.log")).unwrap_or_default()
}

/// The fake `codex`'s calls so far, one argv per line.
pub fn codex_log(home: &Path) -> String {
    fs::read_to_string(home.join("codex.log")).unwrap_or_default()
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

/// [`rtok`] over [`raw_without_claude`].
pub fn rtok_without_claude(args: &[&str], cfg: &Path, home: &Path) -> String {
    let out = raw_without_claude(args, cfg, home);
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

/// Does `text` install our hook for `event`? `rtok_command`'s Windows fallback (no bare `rtok`
/// on the sandboxed PATH) writes the absolute `…\rtok.exe` instead of the bare name, so a
/// literal `"rtok hook <event>"` search would miss it; dropping `.exe` first collapses that
/// back to the same shape the test expects on every platform.
pub fn contains_hook(text: &str, event: &str) -> bool {
    text.replace(".exe", "")
        .contains(&format!("rtok hook {event}"))
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
