//! `rtok run -- <cmd>`: capture, archive, print (plan T3.1). Unfiltered in this task.

use crate::config::Config;
use anyhow::{Result, bail};
use rtok_plugin_sdk::{Archive, Class, Measurement};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

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
/// still picks the right flags (`-c` vs `/C` vs `-Command`).
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
        // `-c`, not `-lc` (T235.2): the host already ran its login profile in the shell that
        // runs `rtok run`, and the environment it passes down is the one the unwrapped command
        // would have had. A second login shell cost ~0.15 s per call (hyperfine: `zsh -lc true`
        // 152 ms vs `zsh -c true` 3 ms) and could reorder PATH (macOS `path_helper`).
        ShellKind::Posix => vec!["-c".into(), body.into()],
    }
}

/// Whether the printed output needs the `expand` trailer. A line count above
/// `trailer_min_lines` is the old rule. Below it the trailer is printed only when the
/// shortening dropped something a reader could miss — a line, a cut string, bytes that
/// are not valid UTF-8 — and not when it only folded whitespace or stripped ANSI
/// colour (T160): there would be nothing to expand, and the ~110-byte trailer would
/// cost more than the padding it "saved". When the output already names the archive
/// (the rules' `… N lines omitted (expand <id>)` marker) the trailer would only repeat
/// that pointer, so `named` suppresses it as well.
pub(crate) fn needs_pointer(
    lines: u32,
    trailer_min_lines: u32,
    body: &[u8],
    filtered: &str,
    named: bool,
) -> bool {
    if lines > trailer_min_lines {
        return true;
    }
    if canonical(body) == canonical(filtered.as_bytes()) {
        return false;
    }
    !named
}

/// Whether `filtered` already names the archive id (an inline `expand <id>` marker).
pub(crate) fn names_the_id(filtered: &str, id: &str) -> bool {
    filtered.contains(&format!("expand {id}"))
}

/// The pointer line appended after a filtered body: names the id, the line count and
/// how to expand it. The one spelling shared by `rtok run`, `rtok filter --archive`
/// and `rtok filter --stdin` so the three surfaces stay byte-identical.
pub(crate) fn trailer(id: &str, lines: u32) -> String {
    format!("[rtok {id} · {lines} lines · expand: rtok expand {id}]")
}

/// `body` up to the noise a filter may remove without a reader losing anything:
/// whitespace runs (padding, trailing newline) and ANSI escapes, compared line by line.
/// Every other byte stays verbatim — a `\xff` that came back U+FFFD must read as a loss.
fn canonical(body: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(body.len());
    let mut line: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i < body.len() {
        match body[i] {
            0x1b => i = skip_escape(body, i),
            b'\n' => {
                push_canonical_line(&line, &mut out);
                line.clear();
                i += 1;
            }
            b' ' | b'\t' | b'\r' => {
                while matches!(body.get(i), Some(b' ' | b'\t' | b'\r')) {
                    i += 1;
                }
                if !line.is_empty() {
                    line.push(b' ');
                }
            }
            b => {
                line.push(b);
                i += 1;
            }
        }
    }
    push_canonical_line(&line, &mut out);
    out
}

fn push_canonical_line(line: &[u8], out: &mut Vec<u8>) {
    let t = line.trim_ascii();
    if t.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(b'\n');
    }
    out.extend_from_slice(t);
}

/// Skip one ANSI escape at `i` (CSI `ESC [ … final`, OSC `ESC ] … BEL|ESC \`, else
/// `ESC` + one byte) and return the index after it.
fn skip_escape(body: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    match body.get(j) {
        Some(b'[') => {
            j += 1;
            while matches!(body.get(j), Some(c) if !(0x40..=0x7e).contains(c)) {
                j += 1;
            }
            j + 1
        }
        Some(b']') => {
            j += 1;
            while j < body.len() {
                if body[j] == 0x07 {
                    return j + 1;
                }
                if body[j] == 0x1b && body.get(j + 1) == Some(&b'\\') {
                    return j + 2;
                }
                j += 1;
            }
            j
        }
        Some(_) => j + 1,
        None => j,
    }
}

/// Run `args` via the configured/host shell, archive stdout+stderr, print, return the exit
/// code. `agent` is the sub-agent id the `PreToolUse(Bash)` rewrite embedded as `--agent`
/// (T127), or `None` for the main window / any other caller — it scopes the dedup pointer
/// so a body one context wrote is never handed to a different one as a pointer.
pub fn run(cfg: &Config, args: &[String], agent: Option<&str>) -> Result<i32> {
    if args.is_empty() {
        bail!("rtok run: missing command");
    }
    let sh = shell(cfg);
    let sh_kind = shell_kind(&sh);
    let mut cmd = Command::new(&sh);
    cmd.args(shell_args(&sh, &script_for(sh_kind, args)));
    let (body, code) = capture(cmd)?;
    emit_filtered(cfg, args, &body, code, agent);
    Ok(code)
}

/// How long [`capture`] keeps reading once the wrapped process has exited. A descendant it
/// left running (`git fsmonitor--daemon`, `gradle --daemon`, a backgrounded server) can hold
/// the pipe's write end for good, so waiting for EOF would hang the agent's call (T235.1).
const DRAIN_AFTER_EXIT: Duration = Duration::from_millis(200);

/// `cmd`'s stdout then stderr, and its exit code. Waits for the process rather than for EOF
/// (unlike `Command::output`): after it exits, what its pipes already hold is drained for up
/// to [`DRAIN_AFTER_EXIT`], and a reader still blocked by a descendant is left behind.
fn capture(mut cmd: Command) -> Result<(Vec<u8>, i32)> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let (tx, rx) = mpsc::channel();
    let out = drain(child.stdout.take(), tx.clone());
    let err = drain(child.stderr.take(), tx);
    let code = child.wait()?.code().unwrap_or(1);
    let deadline = Instant::now() + DRAIN_AFTER_EXIT;
    for _ in 0..2 {
        if rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .is_err()
        {
            break;
        }
    }
    let mut body = std::mem::take(&mut *out.lock().unwrap_or_else(|e| e.into_inner()));
    body.append(&mut err.lock().unwrap_or_else(|e| e.into_inner()));
    Ok((body, code))
}

/// Read `pipe` to EOF on its own thread into the returned buffer; signal `done` at EOF.
fn drain(pipe: Option<impl Read + Send + 'static>, done: mpsc::Sender<()>) -> Arc<Mutex<Vec<u8>>> {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&buf);
    std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            let mut chunk = [0u8; 8192];
            while let Ok(n @ 1..) = pipe.read(&mut chunk) {
                sink.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .extend_from_slice(&chunk[..n]);
            }
        }
        let _ = done.send(());
    });
    buf
}

/// Archive `body` when the shortening dropped something, print the filtered text plus
/// expand trailer, record a Measurement. Shared by `rtok run` and `rtok filter --archive`
/// (which always passes `None`: that surface has no dispatch-time context to scope by).
pub fn emit_filtered(cfg: &Config, argv: &[String], body: &[u8], exit: i32, agent: Option<&str>) {
    let mut out = std::io::stdout();
    emit_filtered_to(cfg, argv, body, exit, agent, &mut out);
}

/// [`emit_filtered`] writing to `out` instead of stdout — tests assert the printed bytes
/// (T175 verbatim pass-through) without capturing the process stdout.
fn emit_filtered_to(
    cfg: &Config,
    argv: &[String],
    body: &[u8],
    exit: i32,
    agent: Option<&str>,
    out: &mut dyn std::io::Write,
) {
    let before = String::from_utf8_lossy(body);
    let write_text = |out: &mut dyn std::io::Write, text: &str| {
        let _ = out.write_all(text.as_bytes());
        if !text.is_empty() && !text.ends_with('\n') {
            let _ = out.write_all(b"\n");
        }
    };
    let cx = match crate::plugin::Runtime::open(cfg.clone(), "run") {
        Ok(cx) => cx,
        Err(_) => {
            // Fail open on the CLI path too: never swallow the command's output (D4).
            write_text(out, &before);
            return;
        }
    };
    // Hash the raw bytes before archiving (T65.1): a same-session hit is a pointer, not
    // the body. Fail open — lookup errors and short bodies print as today. T127: `agent`
    // scopes the hit to the caller's window, so a sub-agent never sees a pointer to a body
    // written by the main window or a different sub-agent (and vice versa).
    if let Some(msg) = crate::plugin::identical_result(&cx, "cmd", body, agent) {
        write_text(out, &msg);
        return;
    }
    // The id is the body's sha256, so the filter can name it before any store write.
    let id = crate::store::hex_sha256(body);
    let lines = if body.is_empty() {
        0
    } else {
        before.lines().count() as u32
    };
    // T175: no trailer on tiny outputs. The trailer is ~170 B; when the raw body is
    // itself smaller, filtered output plus trailer can only be larger than the input.
    // Emit the raw bytes verbatim with no trailer and no archive (nothing is cut, so
    // lossless-by-default holds), and record a zero-saving row so the ledger stays honest.
    let pointer_line = trailer(&id, lines);
    if body.len() <= pointer_line.len() {
        let _ = out.write_all(body);
        if !body.is_empty() && !body.ends_with(b"\n") {
            let _ = out.write_all(b"\n");
        }
        let est = cx.estimate(&before, Class::Code);
        let _ = cx.record(&Measurement {
            plugin: "cmd",
            kind: "raw",
            before_bytes: body.len() as u64,
            after_bytes: body.len() as u64,
            est_before: est,
            est_after: est,
            ref_id: None,
            call_id: None,
        });
        return;
    }
    let settings = rules::Settings::from_config(cfg);
    let family = formatters::family(argv);
    let (filtered, kind) = formatters::compress(&settings, argv, &before, exit, &id);
    // A formatter that trims a 29-line `git log` to 20 leaves the other 9 reachable only
    // through this id. A shortening that took only whitespace/ANSI has nothing to expand
    // (T160) — and then nothing references the id, so the archive row is skipped too.
    let named = names_the_id(&filtered, &id);
    let pointer = needs_pointer(
        lines,
        cfg.plugins.cmd.trailer_min_lines,
        body,
        &filtered,
        named,
    );
    if pointer || named {
        // The archive keeps the command's bytes, not the lossy `String` used to filter
        // and print them: `expand` must return what the command wrote, including invalid UTF-8.
        if cx.put_archive_for(body, agent).is_err() {
            write_text(out, &before);
            return;
        }
    }
    // T247: measure exactly what the host receives — the padding newline and the trailer
    // line are part of the returned bytes, so `after` must count them too.
    let mut shown = filtered;
    if !shown.is_empty() && !shown.ends_with('\n') {
        shown.push('\n');
    }
    if pointer {
        shown.push_str(&pointer_line);
        shown.push('\n');
    }
    let _ = out.write_all(shown.as_bytes());
    let _ = cx.record(&Measurement {
        plugin: "cmd",
        kind,
        before_bytes: body.len() as u64,
        after_bytes: shown.len() as u64,
        est_before: cx.estimate(&before, Class::Code),
        est_after: cx.estimate(&shown, Class::Code),
        ref_id: (pointer || named).then(|| format!("{family}:{id}")),
        call_id: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::config as cfg;
    use std::fs;

    #[test]
    fn emit_filtered_archives_stdin_and_records() {
        let (c, dir) = cfg("emit-archive");
        let body = (0..80)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        emit_filtered(&c, &["cat".into()], body.as_bytes(), 0, None);
        let files: Vec<_> = fs::read_dir(&c.core.archive_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(fs::read(&files[0]).unwrap(), body.as_bytes());
        let v = crate::web::model::plugin_stats(&c, "cmd").unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{v}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_filtered_skill_records_kind() {
        let (c, dir) = cfg("emit-skill");
        let body = (0..80)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        emit_filtered(
            &c,
            &["skill".into(), "demo".into()],
            body.as_bytes(),
            0,
            None,
        );
        let v = crate::web::model::plugin_stats(&c, "cmd").unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows[0]["kind"], "skill", "{v}");
        assert_eq!(v["plugin"], "cmd", "{v}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn printf_two_lines_exit_0_no_trailer() {
        let (c, dir) = cfg("printf");
        let code = run(&c, &["printf".into(), "a\nb\n".into()], None).unwrap();
        assert_eq!(code, 0);
        // T160: the trailing newline is noise — no pointer, so no archive row either.
        let files: Vec<_> = fs::read_dir(&c.core.archive_dir)
            .map(|rd| rd.map(|e| e.unwrap().path()).collect())
            .unwrap_or_default();
        assert!(files.is_empty(), "nothing references an id: {files:?}");
        let v = crate::web::model::plugin_stats(&c, "cmd").unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{v}");
        assert!(
            rows[0]["after"].as_i64() <= rows[0]["before"].as_i64(),
            "{v}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn identical_output_from_different_commands_dedups() {
        let (c, dir) = cfg("dedup-hash");
        // A body the filter shortens (80 lines > 40): T160 stores the archive only when
        // something references its id, and a dedup pointer names exactly that id.
        let payload = format!(
            "{}\n",
            (0..80)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(
            run(&c, &["printf".into(), payload.clone()], None).unwrap(),
            0
        );
        let inner = format!("printf '%s\n' '{}'", payload.trim_end_matches('\n'));
        assert_eq!(
            run(&c, &["sh".into(), "-c".into(), inner], None).unwrap(),
            0
        );
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

    /// T127: the `--agent` a `PreToolUse(Bash)` rewrite embeds scopes the dedup pointer to
    /// that sub-agent's own window — a different sub-agent (or the main window) still sees
    /// the full body, and the same agent repeating the command still gets the pointer.
    #[test]
    fn dedup_is_scoped_to_the_dispatching_agent() {
        let (c, dir) = cfg("dedup-agent-scope");
        let payload = format!(
            "{}\n",
            (0..80)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        emit_filtered(
            &c,
            &["printf".into()],
            payload.as_bytes(),
            0,
            Some("agent-a"),
        );
        let store = crate::store::Store::open(&c.core.db_path).unwrap();
        let dedup_after_first = store
            .list_measurements("cmd")
            .unwrap()
            .iter()
            .filter(|r| r.kind == "dedup")
            .count();
        assert_eq!(dedup_after_first, 0, "nothing to dedup against yet");
        // A different sub-agent asking for the identical output still gets the body.
        emit_filtered(
            &c,
            &["printf".into()],
            payload.as_bytes(),
            0,
            Some("agent-b"),
        );
        let dedup_after_b = store
            .list_measurements("cmd")
            .unwrap()
            .iter()
            .filter(|r| r.kind == "dedup")
            .count();
        assert_eq!(dedup_after_b, 0, "agent-b never saw agent-a's body");
        // agent-a repeating its own command dedups.
        emit_filtered(
            &c,
            &["printf".into()],
            payload.as_bytes(),
            0,
            Some("agent-a"),
        );
        let dedup_after_a_again = store
            .list_measurements("cmd")
            .unwrap()
            .iter()
            .filter(|r| r.kind == "dedup")
            .count();
        assert_eq!(dedup_after_a_again, 1, "agent-a's own repeat still dedups");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn exit_3_is_preserved() {
        let (c, dir) = cfg("exit3");
        let code = run(&c, &["sh".into(), "-c".into(), "exit 3".into()], None).unwrap();
        assert_eq!(code, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    /// T235.1: a backgrounded grandchild keeps the capture pipe open after the shell exits.
    /// `capture` returns on the shell's exit with its output and code, not at the pipe's EOF.
    #[cfg(unix)]
    #[test]
    fn capture_returns_when_the_child_exits_though_a_grandchild_holds_the_pipe() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo hi; sleep 20 & exit 4"]);
        let start = Instant::now();
        let (body, code) = capture(cmd).unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{:?}",
            start.elapsed()
        );
        assert_eq!((body.as_slice(), code), (&b"hi\n"[..], 4));
    }

    #[test]
    fn three_runs_stats_plugin_cmd_json_has_rows() {
        let (c, dir) = cfg("stats3");
        for _ in 0..3 {
            assert_eq!(
                run(&c, &["printf".into(), "a\nb\n".into()], None).unwrap(),
                0
            );
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
        let code = run(&c, &["printf a; printf b".into()], None).unwrap();
        assert_eq!(code, 0);
        // "ab" lost nothing, so T160 stores no archive — but the run is still measured.
        let v = crate::web::model::plugin_stats(&c, "cmd").unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows[0]["before"], 2, "{v}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// D4 at the byte level: a command that emits invalid UTF-8 must come back whole from
    /// `expand`. The archive used to store the lossy string, so every such byte was U+FFFD.
    /// This is `emit_filtered`'s job, not the host shell's: earlier drafts padded the
    /// fixture through a real `rtok run` subprocess (a `printf` octal escape, then a file
    /// read via `cat`/`type`) and hit a different Windows shell-quoting failure each time
    /// — cmd.exe mangles backslash escapes and treats `%` as expansion, and even a plain
    /// `type <path>` came back empty on that runner. None of that exercises the archive
    /// logic under test, so call `emit_filtered` directly, like
    /// `emit_filtered_archives_stdin_and_records` above.
    #[test]
    fn archive_keeps_bytes_that_are_not_utf8() {
        let (c, dir) = cfg("bytes");
        // T175 passes bodies smaller than their own trailer through with no archive, so
        // this fixture pads past the ~170 B trailer to still exercise the archive path.
        let mut body = b"\xff\xfeok\n".to_vec();
        body.extend(std::iter::repeat_n(b'x', 300));
        body.push(b'\n');
        emit_filtered(&c, &["cat".into()], &body, 0, None);
        let files: Vec<_> = fs::read_dir(&c.core.archive_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let raw = fs::read(&files[0]).unwrap();
        assert_eq!(raw, body);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A formatter that trims a short output still has to name the archive: `git log` keeps
    /// 20 of 29 lines, and the line threshold alone would leave the other 9 unreachable.
    /// An in-place cut and destroyed bytes are losses too, however short the output was.
    #[test]
    fn a_shortened_output_needs_a_pointer_whatever_its_length() {
        let log = (0..29)
            .map(|i| format!("c{i} msg"))
            .collect::<Vec<_>>()
            .join("\n");
        let cut = (0..20)
            .map(|i| format!("c{i} msg"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            needs_pointer(29, 40, log.as_bytes(), &cut, false),
            "trimmed under the threshold"
        );
        assert!(
            !needs_pointer(29, 40, log.as_bytes(), &log, false),
            "nothing dropped"
        );
        assert!(
            needs_pointer(400, 40, log.as_bytes(), &log, false),
            "long enough on its own"
        );
        assert!(
            needs_pointer(1, 40, br#"{"blob":"xxxx"}"#, r#"blob: "xx… " (4)"#, false),
            "a string cut in place"
        );
        assert!(
            needs_pointer(1, 40, b"\xff\xfeok", "\u{FFFD}\u{FFFD}ok", false),
            "bytes the lossy string destroyed"
        );
        assert!(
            !needs_pointer(
                10,
                40,
                log.as_bytes(),
                "c0 msg\n… 27 lines omitted (expand abc)\nc28 msg",
                true,
            ),
            "the marker already names the archive"
        );
    }

    /// T160 repro: a 7-line summary whose only loss is column padding and the trailing
    /// newline got the ~110-byte trailer. Whitespace and ANSI colour are noise — a reader
    /// misses nothing, so there is nothing to expand and no trailer to pay for.
    #[test]
    fn padding_and_ansi_only_changes_need_no_pointer() {
        let padded = "NAME   STATUS   AGE\nweb-0  Running  3d\napi-1  Pending  1h\n";
        let collapsed = "NAME STATUS AGE\nweb-0 Running 3d\napi-1 Pending 1h";
        assert!(!needs_pointer(3, 40, padded.as_bytes(), collapsed, false));
        assert!(!needs_pointer(1, 40, b"\x1b[1mok\x1b[0m\n", "ok", false));
        assert!(!needs_pointer(
            1,
            40,
            b"hello-trycmd\n",
            "hello-trycmd",
            false
        ));
    }

    /// T175: bodies smaller than their own trailer pass through verbatim. A 150-byte
    /// `ps` body the `ps_aux` formatter would reshape comes back byte-identical with
    /// no trailer, no archive row, and a zero-saving `raw` Measurement row.
    #[test]
    fn tiny_body_passes_through_verbatim_with_no_negative_saving() {
        let (c, dir) = cfg("tiny-passthrough");
        let line = "root         1  0.0  00:00:01 /sbin/init worker-7 extra-flag";
        let mut rows = vec!["USER         PID  %CPU TIME     COMMAND".to_string()];
        while rows.join("\n").len() + 1 + line.len() < 150 {
            rows.push(line.to_string());
        }
        let body = rows.join("\n");
        assert!(body.len() < 200, "fixture must be tiny: {}", body.len());
        let filtered = formatters::compress(
            &rules::Settings::builtin(),
            &["ps".into(), "aux".into()],
            &body,
            0,
            "deadbeef",
        )
        .0;
        assert_ne!(filtered, body, "formatter must reshape the fixture");
        assert!(
            needs_pointer(
                body.lines().count() as u32,
                40,
                body.as_bytes(),
                &filtered,
                false
            ),
            "old path would have attached a trailer"
        );
        let mut printed = Vec::new();
        emit_filtered_to(
            &c,
            &["ps".into(), "aux".into()],
            body.as_bytes(),
            0,
            None,
            &mut printed,
        );
        let mut expect = body.as_bytes().to_vec();
        if !expect.is_empty() && !expect.ends_with(b"\n") {
            expect.push(b'\n');
        }
        assert_eq!(
            printed, expect,
            "pass-through must be verbatim (no trailer)"
        );
        assert!(
            !String::from_utf8_lossy(&printed).contains("expand"),
            "no expand trailer on tiny body: {}",
            String::from_utf8_lossy(&printed)
        );
        let files: Vec<_> = fs::read_dir(&c.core.archive_dir)
            .map(|rd| rd.map(|e| e.unwrap().path()).collect())
            .unwrap_or_default();
        assert!(files.is_empty(), "no archive on pass-through: {files:?}");
        let store = crate::store::Store::open(&c.core.db_path).unwrap();
        let rows = store.list_measurements("cmd").unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].kind, "raw", "{rows:?}");
        assert_eq!(rows[0].before_bytes, body.len() as i64, "{rows:?}");
        assert_eq!(rows[0].after_bytes, rows[0].before_bytes, "{rows:?}");
        assert_eq!(rows[0].est_after, rows[0].est_before, "{rows:?}");
        assert!(rows[0].ref_id.is_none(), "{rows:?}");
        let _ = fs::remove_dir_all(&dir);
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
            vec!["-c".to_string(), "true".to_string()]
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

    /// Configured PowerShell must not get Posix `-c` (CreateProcess would fail the flag).
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
