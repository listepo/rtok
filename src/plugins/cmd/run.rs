//! `rtok run -- <cmd>`: capture, archive, print (plan T3.1). Unfiltered in this task.

use crate::config::Config;
use anyhow::{Result, bail};
use rtok_plugin_sdk::{Archive, Class, Measurement};
use std::process::Command;

use super::{formatters, rules};

pub(crate) fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

/// Quote a command for the PreToolUse rewrite `rtok run -- <quoted>`.
/// On Windows, Cursor's beforeShellExecution often runs under PowerShell, so the
/// POSIX `'"'"'` embedding from [`sh_quote`] breaks on apostrophes. PowerShell
/// single-quoting (`''` escape) is safe for PS; commands without `'` also parse under cmd.
pub(crate) fn wrap_quote(s: &str) -> String {
    if cfg!(windows) {
        format!("'{}'", s.replace('\'', "''"))
    } else {
        sh_quote(s)
    }
}

/// Quote one argv word for `cmd.exe /C`. Double quotes; escape embedded `"` as `""`.
pub(crate) fn cmd_quote(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".into();
    }
    if !s
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '"' | '^' | '&' | '|' | '<' | '>' | '%'))
    {
        return s.to_string();
    }
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Which launcher `rtok run` should use for `shell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellKind {
    Posix,
    Cmd,
    PowerShell,
}

/// Classify by the executable basename so an explicit `[plugins.cmd] shell`
/// still picks the right flags (`-lc` vs `/C` vs `-Command`).
pub(crate) fn shell_kind(shell: &str) -> ShellKind {
    match formatters::cmd_stem(shell).to_ascii_lowercase().as_str() {
        "cmd" => ShellKind::Cmd,
        "powershell" | "pwsh" => ShellKind::PowerShell,
        _ => ShellKind::Posix,
    }
}

/// `$SHELL` is honoured only for these: `script_for` emits `{ …\n} 2>&1`, which
/// fish, nu or xonsh cannot parse. An explicit `[plugins.cmd] shell` is trusted as is.
const POSIX_SHELLS: [&str; 6] = ["sh", "bash", "zsh", "dash", "ksh", "ash"];

/// Join argv into a shell body. A single argument is already a shell snippet
/// (the PreToolUse wrap quotes the original command as one argv); several
/// arguments are a CLI argv list and must be quoted so spaces stay inside
/// one word. Posix wraps with `{ … } 2>&1`; cmd uses `(…) 2>&1`; PowerShell
/// uses a script block that merges streams.
pub(crate) fn script_for(kind: ShellKind, args: &[String]) -> String {
    match kind {
        ShellKind::Posix => {
            let inner = match args {
                [one] => one.clone(),
                many => many
                    .iter()
                    .map(|a| sh_quote(a))
                    .collect::<Vec<_>>()
                    .join(" "),
            };
            format!("{{ {inner}\n}} 2>&1")
        }
        ShellKind::Cmd => {
            let inner = match args {
                [one] => one.clone(),
                many => many
                    .iter()
                    .map(|a| cmd_quote(a))
                    .collect::<Vec<_>>()
                    .join(" "),
            };
            format!("({inner}) 2>&1")
        }
        ShellKind::PowerShell => {
            let inner = match args {
                [one] => one.clone(),
                many => many
                    .iter()
                    .map(|a| format!("'{}'", a.replace('\'', "''")))
                    .collect::<Vec<_>>()
                    .join(" "),
            };
            // `& { … }` runs a bare external; `*>&1` merges streams on PS 5+.
            format!("& {{ {inner} }} *>&1")
        }
    }
}

/// Resolve the shell binary: config override, then `$SHELL`, then a host default.
/// Native Windows has no `/bin/sh` unless Git Bash/WSL is installed; fall back
/// to `%ComSpec%` (`cmd.exe`) so `rtok run` can still spawn.
pub(crate) fn resolve_shell(
    cfg_shell: &str,
    env_shell: Option<&str>,
    windows: bool,
    comspec: Option<&str>,
) -> String {
    if !cfg_shell.is_empty() {
        return cfg_shell.to_string();
    }
    if let Some(s) = env_shell.filter(|s| {
        let stem = formatters::cmd_stem(s);
        POSIX_SHELLS.iter().any(|p| stem.eq_ignore_ascii_case(p))
    }) {
        return s.to_string();
    }
    if windows {
        comspec
            .filter(|s| !s.is_empty())
            .unwrap_or("cmd.exe")
            .to_string()
    } else {
        "/bin/sh".into()
    }
}

fn shell(cfg: &Config) -> String {
    resolve_shell(
        &cfg.plugins.cmd.shell,
        std::env::var("SHELL").ok().as_deref(),
        cfg!(windows),
        std::env::var("ComSpec").ok().as_deref(),
    )
}

/// Flags + script body for `Command::new(shell)`.
pub(crate) fn shell_args(shell: &str, body: &str) -> Vec<String> {
    match shell_kind(shell) {
        ShellKind::Cmd => vec!["/D".into(), "/C".into(), body.into()],
        ShellKind::PowerShell => vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            body.into(),
        ],
        ShellKind::Posix => vec!["-lc".into(), body.into()],
    }
}

/// Whether the printed output needs the `expand` pointer. A line count above
/// `trailer_min_lines` is the old rule; anything the filter shortened needs it too, however
/// short the command was.
fn needs_pointer(lines: u32, trailer_min_lines: u32, printed: usize, raw: usize) -> bool {
    lines > trailer_min_lines || printed < raw
}

/// Run `args` via the configured/host shell, archive stdout+stderr, print, return the exit code.
pub fn run(cfg: &Config, args: &[String]) -> Result<i32> {
    if args.is_empty() {
        bail!("rtok run: missing command");
    }
    let sh = shell(cfg);
    let sh_kind = shell_kind(&sh);
    let out = Command::new(&sh)
        .args(shell_args(&sh, &script_for(sh_kind, args)))
        .output()?;
    let mut body = out.stdout;
    body.extend_from_slice(&out.stderr);
    let code = out.status.code().unwrap_or(1);
    let before = String::from_utf8_lossy(&body);
    let cx = match crate::plugin::Runtime::open(cfg.clone(), "run") {
        Ok(cx) => cx,
        Err(_) => {
            // Fail open on the CLI path too: never swallow the command's output (D4).
            print!("{before}");
            if !before.is_empty() && !before.ends_with('\n') {
                println!();
            }
            return Ok(code);
        }
    };
    // Hash the raw bytes before archiving (T65.1): a same-session hit is a pointer, not
    // the body. Fail open — lookup errors and short bodies print as today.
    if let Some(msg) = crate::plugin::identical_result(&cx, "cmd", &body) {
        println!("{msg}");
        return Ok(code);
    }
    // The archive keeps the command's bytes, not the lossy `String` used to filter and
    // print them: `expand` must return what the command wrote, including invalid UTF-8.
    let id = match cx.put_archive(&body) {
        Ok(id) => id,
        Err(_) => {
            print!("{before}");
            if !before.is_empty() && !before.ends_with('\n') {
                println!();
            }
            return Ok(code);
        }
    };
    let settings = rules::Settings::from_config(cfg);
    let family = formatters::family(args);
    let (filtered, kind) = formatters::compress(&settings, args, &before, code, &id);
    print!("{filtered}");
    if !filtered.is_empty() && !filtered.ends_with('\n') {
        println!();
    }
    let lines = if body.is_empty() {
        0
    } else {
        before.lines().count() as u32
    };
    // Any shortening prints the pointer, not only a long one: a formatter that trims a
    // 29-line `git log` to 20 leaves the other 9 reachable only through this id.
    if needs_pointer(
        lines,
        cfg.plugins.cmd.trailer_min_lines,
        filtered.len(),
        before.len(),
    ) {
        println!("[rtok {id} · {lines} lines · expand: rtok expand {id}]");
    }
    let _ = cx.record(&Measurement {
        plugin: "cmd",
        kind,
        before_bytes: body.len() as u64,
        after_bytes: filtered.len() as u64,
        est_before: cx.estimate(&before, Class::Code),
        est_after: cx.estimate(&filtered, Class::Code),
        ref_id: Some(format!("{family}:{id}")),
        call_id: None,
    });
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::config as cfg;
    use std::fs;

    #[test]
    fn printf_two_lines_exit_0_no_trailer() {
        let (c, dir) = cfg("printf");
        let code = run(&c, &["printf".into(), "a\nb\n".into()]).unwrap();
        assert_eq!(code, 0);
        let files: Vec<_> = fs::read_dir(&c.core.archive_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        let raw = fs::read(files[0].clone()).unwrap();
        assert_eq!(raw, b"a\nb\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn identical_output_from_different_commands_dedups() {
        let (c, dir) = cfg("dedup-hash");
        let payload = format!(
            "{}
",
            "x".repeat(300)
        );
        assert_eq!(run(&c, &["printf".into(), payload.clone()]).unwrap(), 0);
        let inner = format!("printf '%s\n' '{}'", "x".repeat(300));
        assert_eq!(run(&c, &["sh".into(), "-c".into(), inner]).unwrap(), 0);
        let store = crate::store::Store::open(&c.core.db_path).unwrap();
        let rows = store.list_measurements("cmd").unwrap();
        let dedup = rows.iter().filter(|r| r.kind == "dedup").count();
        assert_eq!(dedup, 1, "{rows:?}");
        assert!(
            rows.iter()
                .any(|r| r.kind == "dedup" && r.before_bytes > r.after_bytes)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn exit_3_is_preserved() {
        let (c, dir) = cfg("exit3");
        let code = run(&c, &["sh".into(), "-c".into(), "exit 3".into()]).unwrap();
        assert_eq!(code, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn three_runs_stats_plugin_cmd_json_has_rows() {
        let (c, dir) = cfg("stats3");
        for _ in 0..3 {
            assert_eq!(run(&c, &["printf".into(), "a\nb\n".into()]).unwrap(), 0);
        }
        let v = crate::web::model::plugin_stats(&c, "cmd").unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3, "{v}");
        for r in rows {
            assert!(
                r["before"].as_i64().unwrap() >= r["after"].as_i64().unwrap(),
                "{r}"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn script_keeps_one_arg_as_a_shell_snippet() {
        assert_eq!(
            script_for(ShellKind::Posix, &["true && false".into()]),
            "{ true && false\n} 2>&1"
        );
        assert_eq!(
            script_for(ShellKind::Posix, &["git".into(), "status".into()]),
            "{ 'git' 'status'\n} 2>&1"
        );
    }

    #[test]
    fn one_arg_compound_command_runs_as_one_script() {
        let (c, dir) = cfg("compound");
        let code = run(&c, &["printf a; printf b".into()]).unwrap();
        assert_eq!(code, 0);
        let files: Vec<_> = fs::read_dir(&c.core.archive_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        let raw = fs::read(&files[0]).unwrap();
        assert_eq!(raw, b"ab");
        let _ = fs::remove_dir_all(&dir);
    }

    /// D4 at the byte level: a command that emits invalid UTF-8 must come back whole from
    /// `expand`. The archive used to store the lossy string, so every such byte was U+FFFD.
    /// Emit bytes with POSIX octal `printf` escapes (`\\377`), not bash-only `\\xHH`:
    /// dash `/bin/sh` leaves `\\xHH` literal, which made this test fail when `SHELL` is unset.
    #[test]
    fn archive_keeps_bytes_that_are_not_utf8() {
        let (c, dir) = cfg("bytes");
        let code = run(&c, &["printf".into(), r"\377\376ok\n".into()]).unwrap();
        assert_eq!(code, 0);
        let files: Vec<_> = fs::read_dir(&c.core.archive_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let raw = fs::read(&files[0]).unwrap();
        assert_eq!(raw, b"\xff\xfeok\n");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A formatter that trims a short output still has to name the archive: `git log` keeps
    /// 20 of 29 lines, and the line threshold alone would leave the other 9 unreachable.
    #[test]
    fn a_shortened_output_needs_a_pointer_whatever_its_length() {
        assert!(!needs_pointer(29, 40, 400, 400), "nothing was dropped");
        assert!(
            needs_pointer(29, 40, 300, 400),
            "trimmed under the threshold"
        );
        assert!(needs_pointer(400, 40, 400, 400), "long enough on its own");
    }

    #[test]
    fn resolve_shell_prefers_config_then_env_then_host_default() {
        assert_eq!(
            resolve_shell(
                r"C:\shell\bash.exe",
                Some("/bin/zsh"),
                true,
                Some("cmd.exe")
            ),
            r"C:\shell\bash.exe"
        );
        assert_eq!(
            resolve_shell(
                "",
                Some("/bin/zsh"),
                true,
                Some(r"C:\Windows\System32\cmd.exe")
            ),
            "/bin/zsh"
        );
        assert_eq!(
            resolve_shell("", None, true, Some(r"C:\Windows\System32\cmd.exe")),
            r"C:\Windows\System32\cmd.exe"
        );
        assert_eq!(resolve_shell("", None, true, None), "cmd.exe");
        assert_eq!(resolve_shell("", None, false, None), "/bin/sh");
        assert_eq!(resolve_shell("", Some(""), false, None), "/bin/sh");
    }

    /// `$SHELL=fish` (nu, xonsh) cannot run the `{ … } 2>&1` body: fall back to the host
    /// default. Config still wins, and `.exe` / case do not matter.
    #[test]
    fn resolve_shell_ignores_non_posix_env_shell() {
        for s in ["/opt/homebrew/bin/fish", "nu", "/usr/bin/xonsh", "fish"] {
            assert_eq!(resolve_shell("", Some(s), false, None), "/bin/sh", "{s}");
            assert_eq!(resolve_shell("", Some(s), true, None), "cmd.exe", "{s}");
        }
        for s in [
            "/bin/bash",
            "/usr/local/bin/zsh",
            "dash",
            "ksh",
            r"C:\msys64\usr\bin\BASH.EXE",
        ] {
            assert_eq!(resolve_shell("", Some(s), false, None), s, "{s}");
        }
        assert_eq!(
            resolve_shell("/usr/bin/fish", Some("/bin/zsh"), false, None),
            "/usr/bin/fish"
        );
    }

    #[test]
    fn shell_kind_and_args_match_windows_hosts() {
        assert_eq!(shell_kind("/bin/sh"), ShellKind::Posix);
        assert_eq!(shell_kind(r"C:\Windows\System32\cmd.exe"), ShellKind::Cmd);
        assert_eq!(shell_kind("powershell.exe"), ShellKind::PowerShell);
        assert_eq!(shell_kind("pwsh"), ShellKind::PowerShell);
        assert_eq!(
            shell_args("/bin/sh", "true"),
            vec!["-lc".to_string(), "true".to_string()]
        );
        assert_eq!(
            shell_args(r"C:\Windows\System32\cmd.exe", "echo hi"),
            vec!["/D".to_string(), "/C".to_string(), "echo hi".to_string()]
        );
        assert_eq!(
            shell_args("pwsh.exe", "Get-Date"),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "Get-Date".to_string()
            ]
        );
    }

    #[test]
    fn script_for_cmd_quotes_multi_argv() {
        assert_eq!(
            script_for(ShellKind::Cmd, &["echo".into(), "hello world".into()]),
            "(echo \"hello world\") 2>&1"
        );
        assert_eq!(
            script_for(ShellKind::Cmd, &["echo hi".into()]),
            "(echo hi) 2>&1"
        );
        assert_eq!(cmd_quote("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    /// Configured PowerShell must not get Posix `-lc` (CreateProcess would fail the flag).
    #[test]
    fn configured_powershell_uses_command_flag() {
        let (mut c, dir) = cfg("ps-shell");
        c.plugins.cmd.shell = "powershell.exe".into();
        let sh = shell(&c);
        assert_eq!(sh, "powershell.exe");
        assert_eq!(shell_kind(&sh), ShellKind::PowerShell);
        let args = shell_args(
            &sh,
            &script_for(ShellKind::PowerShell, &["echo".into(), "x".into()]),
        );
        assert_eq!(args[0], "-NoProfile");
        assert_eq!(args[2], "-Command");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrap_quote_is_host_safe_for_apostrophe() {
        let q = wrap_quote("echo it's");
        if cfg!(windows) {
            assert_eq!(q, "'echo it''s'");
        } else {
            assert_eq!(q, sh_quote("echo it's"));
        }
    }
}
