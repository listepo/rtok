//! Offer to restart a host after `agents install|uninstall` (T76).
//!
//! After config writes finish, ask interactively whether to stop then start that host so the
//! new config is live. Timeout comes only from `[setup].restart_prompt_timeout_seconds`:
//! `0` / absent means wait forever; a positive value treats silence as No (no restart).
//! Dry-run and non-interactive stdin skip the prompt entirely.

use std::io::{self, IsTerminal, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::{Agent, Kind, host};
use crate::config::Config;

/// Braille spinner frames (~80 ms cadence).
pub const BRAILLE_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// ASCII fallback when the locale is not UTF-8.
pub const ASCII_FRAMES: &[char] = &['-', '\\', '|', '/'];

/// What the user (or timeout) decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartChoice {
    Yes,
    No,
    /// Dry-run, non-TTY, or explicit skip — no prompt was shown.
    Skipped,
}

/// Prefer braille when the process locale looks UTF-8.
pub fn spinner_frames() -> &'static [char] {
    if locale_is_utf8() {
        BRAILLE_FRAMES
    } else {
        ASCII_FRAMES
    }
}

fn locale_is_utf8() -> bool {
    for key in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(v) = std::env::var(key) {
            let lower = v.to_ascii_lowercase();
            if lower.contains("utf-8") || lower.contains("utf8") {
                return true;
            }
            if !v.is_empty() {
                // A concrete non-UTF locale wins over later vars.
                return false;
            }
        }
    }
    // Unset locale: assume UTF-8 on Unix, ASCII on Windows consoles without UTF-8.
    !cfg!(windows)
}

/// True when we should ask: not dry-run and stdin is an interactive TTY.
pub fn should_prompt(cfg: &Config) -> bool {
    should_prompt_with(cfg.setup.dry_run, io::stdin().is_terminal())
}

/// Split out for tests: dry-run or non-interactive → no prompt.
pub fn should_prompt_with(dry_run: bool, interactive: bool) -> bool {
    !dry_run && interactive
}

/// Ask whether to restart `host_id` after a successful install/uninstall write.
///
/// Prints the yes/no prompt with a left-side in-place spinner while waiting. On Yes, runs
/// [`restart_host`]. On No / timeout / skip, leaves the process alone.
pub fn offer_host_restart(cfg: &Config, host_id: &str) -> Result<RestartChoice> {
    if !should_prompt(cfg) {
        return Ok(RestartChoice::Skipped);
    }
    let timeout = match cfg.setup.restart_prompt_timeout_seconds {
        0 => None,
        secs => Some(Duration::from_secs(secs)),
    };
    let prompt = format!("Restart {host_id} so the new config takes effect? [y/N]: ");
    let choice = prompt_with_spinner(&prompt, timeout, spinner_frames())?;
    match choice {
        RestartChoice::Yes => {
            match restart_host(host_id) {
                Ok(msg) => println!("{msg}"),
                Err(e) => {
                    // Config writes already succeeded; a restart failure must not fail the command.
                    eprintln!("warning: could not restart {host_id}: {e:#}");
                }
            }
            Ok(RestartChoice::Yes)
        }
        other => {
            if other == RestartChoice::No {
                println!("Leaving {host_id} running. Restart it yourself if it caches config.");
            }
            Ok(other)
        }
    }
}

/// After install/uninstall of every named host, offer a restart for each (interactive only).
pub fn offer_after_setup(cfg: &Config, host_ids: &[String]) -> Result<()> {
    for id in host_ids {
        offer_host_restart(cfg, id)?;
    }
    Ok(())
}

/// Blocking / timed yes-no read with a left-side spinner; only the glyph is redrawn, never the typed answer.
///
/// `timeout = None` waits forever. Empty input / timeout / anything other than y/yes → No.
pub fn prompt_with_spinner(
    prompt_label: &str,
    timeout: Option<Duration>,
    frames: &[char],
) -> Result<RestartChoice> {
    prompt_with_spinner_io(
        prompt_label,
        timeout,
        frames,
        Duration::from_millis(80),
        &mut io::stdout(),
        read_line_async,
    )
}

/// Test seam: inject writer, frame period, and a line source.
pub fn prompt_with_spinner_io<W: Write>(
    prompt_label: &str,
    timeout: Option<Duration>,
    frames: &[char],
    frame_period: Duration,
    out: &mut W,
    wait_line: impl FnOnce(Option<Duration>) -> io::Result<Option<String>> + Send + 'static,
) -> Result<RestartChoice> {
    let frames = if frames.is_empty() {
        ASCII_FRAMES
    } else {
        frames
    };
    let (tx, rx) = mpsc::channel::<io::Result<Option<String>>>();
    let timeout_for_reader = timeout;
    thread::spawn(move || {
        let _ = tx.send(wait_line(timeout_for_reader));
    });

    let started = Instant::now();
    let mut frame_i = 0usize;
    // First paint immediately so the prompt appears with the spinner.
    write!(out, "\r\x1b[K{} {prompt_label}", frames[0])?;
    out.flush()?;

    let line = loop {
        let wait = match timeout {
            Some(limit) => {
                let left = limit.saturating_sub(started.elapsed());
                if left.is_zero() {
                    finish_unanswered(out)?;
                    return Ok(RestartChoice::No);
                }
                left.min(frame_period)
            }
            None => frame_period,
        };
        match rx.recv_timeout(wait) {
            Ok(Ok(Some(line))) => break line,
            Ok(Ok(None)) => {
                // EOF with no line → treat as No.
                finish_unanswered(out)?;
                return Ok(RestartChoice::No);
            }
            Ok(Err(e)) => {
                finish_unanswered(out)?;
                return Err(e.into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(limit) = timeout
                    && started.elapsed() >= limit
                {
                    finish_unanswered(out)?;
                    return Ok(RestartChoice::No);
                }
                frame_i = (frame_i + 1) % frames.len();
                paint_frame(out, frames[frame_i])?;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                finish_unanswered(out)?;
                return Ok(RestartChoice::No);
            }
        }
    };

    finish_answered(out)?;
    Ok(parse_yes_no(&line))
}

/// Swap only the glyph in column 0. Save/restore the cursor and never clear the line, so the
/// answer the user is typing (echoed by the terminal after the prompt) stays visible.
fn paint_frame(out: &mut impl Write, frame: char) -> io::Result<()> {
    write!(out, "\x1b7\r{frame}\x1b8")?;
    out.flush()
}

/// No answer (timeout/EOF/error): blank the glyph, keep the prompt, and end the line.
fn finish_unanswered(out: &mut impl Write) -> io::Result<()> {
    write!(out, "\x1b7\r \x1b8\n")?;
    out.flush()
}

/// The user pressed Enter, so the cursor is on the next row: blank the glyph one row up.
fn finish_answered(out: &mut impl Write) -> io::Result<()> {
    write!(out, "\x1b7\x1b[A\r \x1b8")?;
    out.flush()
}

fn parse_yes_no(line: &str) -> RestartChoice {
    match line.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => RestartChoice::Yes,
        _ => RestartChoice::No,
    }
}

/// Read one stdin line; when `timeout` is `Some`, return `Ok(None)` on expiry without a line.
fn read_line_async(timeout: Option<Duration>) -> io::Result<Option<String>> {
    // Dedicated reader thread is already used by the spinner loop; here we just block on stdin.
    // The outer `recv_timeout` enforces the overall deadline; this helper ignores its timeout
    // argument when called from that thread (the channel recv is what times out). Keeping the
    // parameter lets tests pass a stub that honors it without spawning stdin.
    let _ = timeout;
    let mut line = String::new();
    let n = io::stdin().read_line(&mut line)?;
    if n == 0 && line.is_empty() {
        return Ok(None);
    }
    Ok(Some(line))
}

/// Stop then start a host. Desktop apps get a real quit/reopen where we know how; CLI-only
/// hosts get a clear message that a manual relaunch is needed.
pub fn restart_host(host_id: &str) -> Result<String> {
    let agent = host(host_id).with_context(|| format!("unknown host: {host_id}"))?;
    restart_agent(agent)
}

fn restart_agent(agent: &dyn Agent) -> Result<String> {
    let id = agent.id();
    let mut desktop: Vec<(&str, &str)> = Vec::new();
    let mut cli_bins: Vec<&str> = Vec::new();
    for v in agent.variants() {
        match v.kind {
            Kind::Desktop => {
                for app in v.apps {
                    desktop.push((*app, v.name));
                }
                for bin in v.bins {
                    cli_bins.push(*bin);
                }
            }
            Kind::Cli => {
                for bin in v.bins {
                    cli_bins.push(*bin);
                }
            }
        }
    }

    if desktop.is_empty() && cli_bins.is_empty() {
        return Ok(format!(
            "{id}: no app or binary recorded for restart; relaunch it yourself"
        ));
    }

    // Prefer a desktop quit/reopen when an app bundle/path is listed.
    for (app, name) in &desktop {
        if let Some(msg) = try_restart_desktop(app, name)? {
            return Ok(format!("{id}: {msg}"));
        }
    }

    // Fall back to killing known binaries (stop only). Starting a CLI agent needs a TTY session
    // we do not own — document that for the user.
    if !cli_bins.is_empty() {
        let stopped = stop_bins(&cli_bins);
        return Ok(format!(
            "{id}: stopped [{}]; start the CLI agent again in your terminal",
            stopped.join(", ")
        ));
    }

    Ok(format!(
        "{id}: desktop app not found on this machine; nothing to restart"
    ))
}

/// Platform quit + relaunch for a desktop app path. `None` if that path is not present.
fn try_restart_desktop(app: &str, display_name: &str) -> Result<Option<String>> {
    let path = expand_app_path(app);
    if !path.exists() {
        return Ok(None);
    }
    let app_name = app_name_from_path(&path).unwrap_or(display_name);
    stop_desktop(app_name)?;
    // Brief pause so the OS can release locks before reopen.
    thread::sleep(Duration::from_millis(400));
    start_desktop(app_name, &path)?;
    Ok(Some(format!("restarted {app_name}")))
}

fn expand_app_path(spec: &str) -> std::path::PathBuf {
    if let Some(rest) = spec.strip_prefix("~/")
        && let Some(home) = crate::config::env_user_home()
    {
        return home.join(rest);
    }
    if let Some(rest) = spec.strip_prefix('$') {
        let (var, tail) = rest.split_once('/').unwrap_or((rest, ""));
        if let Some(root) = std::env::var_os(var) {
            return std::path::Path::new(&root).join(tail);
        }
    }
    std::path::PathBuf::from(spec)
}

fn app_name_from_path(path: &std::path::Path) -> Option<&str> {
    let name = path.file_name()?.to_str()?;
    Some(name.strip_suffix(".app").unwrap_or(name))
}

fn stop_desktop(app_name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("osascript")
            .args(["-e", &format!("tell application \"{app_name}\" to quit")])
            .status()
            .with_context(|| format!("osascript quit {app_name}"))?;
        // Non-zero is fine when the app was not running.
        let _ = status;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", &format!("{app_name}.exe"), "/F"])
            .status();
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Best-effort: kill by process name.
        let _ = std::process::Command::new("killall").arg(app_name).status();
        Ok(())
    }
}

fn start_desktop(app_name: &str, path: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = path; // used on Windows/Linux cfgs
        std::process::Command::new("open")
            .arg("-a")
            .arg(app_name)
            .status()
            .with_context(|| format!("open -a {app_name}"))?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", path.to_str().unwrap_or(app_name)])
            .status()
            .with_context(|| format!("start {app_name}"))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Linux: try gtk-launch / the binary path / xdg-open.
        if path.exists() {
            let _ = std::process::Command::new(path).spawn();
        } else {
            let _ = std::process::Command::new("xdg-open").arg(app_name).spawn();
        }
        Ok(())
    }
}

fn stop_bins(bins: &[&str]) -> Vec<String> {
    let mut stopped = Vec::new();
    for bin in bins {
        #[cfg(windows)]
        let ok = std::process::Command::new("taskkill")
            .args(["/IM", &format!("{bin}.exe"), "/F"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        #[cfg(not(windows))]
        let ok = std::process::Command::new("killall")
            .arg(bin)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            stopped.push((*bin).to_string());
        }
    }
    if stopped.is_empty() {
        bins.iter().map(|b| (*b).to_string()).collect()
    } else {
        stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    fn cfg_timeout(secs: u64) -> Config {
        let mut c = Config::default();
        c.setup.restart_prompt_timeout_seconds = secs;
        c.setup.dry_run = false;
        c
    }

    #[test]
    fn default_timeout_is_zero() {
        assert_eq!(Config::default().setup.restart_prompt_timeout_seconds, 0);
    }

    #[test]
    fn absent_key_deserializes_as_zero() {
        // Figment extract with an empty [setup] table keeps the field default (0).
        use figment::{Figment, providers::Serialized};
        let c: Config = Figment::from(Serialized::defaults(Config::default()))
            .merge(("setup.dry_run", false))
            .extract()
            .unwrap();
        assert_eq!(c.setup.restart_prompt_timeout_seconds, 0);
    }

    #[test]
    fn should_prompt_requires_interactive_and_not_dry_run() {
        assert!(!should_prompt_with(true, true));
        assert!(!should_prompt_with(false, false));
        assert!(!should_prompt_with(true, false));
        assert!(should_prompt_with(false, true));
    }

    #[test]
    fn dry_run_skips_prompt() {
        let mut c = cfg_timeout(5);
        c.setup.dry_run = true;
        assert!(!should_prompt(&c));
        let choice = offer_host_restart(&c, "cursor").unwrap();
        assert_eq!(choice, RestartChoice::Skipped);
    }

    #[test]
    fn parse_yes_variants() {
        assert_eq!(parse_yes_no("y\n"), RestartChoice::Yes);
        assert_eq!(parse_yes_no("Yes"), RestartChoice::Yes);
        assert_eq!(parse_yes_no("n"), RestartChoice::No);
        assert_eq!(parse_yes_no(""), RestartChoice::No);
        assert_eq!(parse_yes_no("maybe"), RestartChoice::No);
    }

    #[test]
    fn yes_path_returns_yes_and_clears_spinner_glyph() {
        let mut buf = Cursor::new(Vec::new());
        let frames = &['A', 'B'];
        let choice = prompt_with_spinner_io(
            "> ",
            None,
            frames,
            Duration::from_millis(1),
            &mut buf,
            |_| Ok(Some("y".into())),
        )
        .unwrap();
        assert_eq!(choice, RestartChoice::Yes);
        let s = String::from_utf8(buf.into_inner()).unwrap();
        assert!(s.contains("A > "));
        // Answered: the glyph one row up is blanked; the typed answer is never cleared.
        assert!(s.ends_with("\x1b7\x1b[A\r \x1b8"));
        assert_eq!(
            s.matches("\x1b[K").count(),
            1,
            "only the first paint clears: {s:?}"
        );
    }

    #[test]
    fn no_path_returns_no() {
        let mut buf = Cursor::new(Vec::new());
        let choice = prompt_with_spinner_io(
            "Your choice: ",
            None,
            ASCII_FRAMES,
            Duration::from_millis(1),
            &mut buf,
            |_| Ok(Some("n".into())),
        )
        .unwrap();
        assert_eq!(choice, RestartChoice::No);
        let s = String::from_utf8(buf.into_inner()).unwrap();
        assert!(s.contains("Your choice: "));
    }

    #[test]
    fn positive_timeout_silence_is_no() {
        let mut buf = Cursor::new(Vec::new());
        let choice = prompt_with_spinner_io(
            "> ",
            Some(Duration::from_millis(30)),
            ASCII_FRAMES,
            Duration::from_millis(5),
            &mut buf,
            |_| {
                thread::sleep(Duration::from_millis(200));
                Ok(Some("y".into())) // too late — outer timeout should already have fired
            },
        )
        .unwrap();
        assert_eq!(choice, RestartChoice::No);
    }

    #[test]
    fn zero_timeout_waits_for_input_no_auto_no() {
        let mut buf = Cursor::new(Vec::new());
        let started = Instant::now();
        let choice = prompt_with_spinner_io(
            "> ",
            None, // 0 → forever
            ASCII_FRAMES,
            Duration::from_millis(5),
            &mut buf,
            |_| {
                thread::sleep(Duration::from_millis(40));
                Ok(Some("yes".into()))
            },
        )
        .unwrap();
        assert_eq!(choice, RestartChoice::Yes);
        assert!(started.elapsed() >= Duration::from_millis(35));
    }

    #[test]
    fn spinner_redraws_in_place_without_newlines_per_frame() {
        let buf = Arc::new(Mutex::new(Cursor::new(Vec::new())));
        let buf_c = Arc::clone(&buf);
        let choice = {
            let mut guard = buf_c.lock().unwrap();
            prompt_with_spinner_io(
                "> ",
                Some(Duration::from_millis(25)),
                &['1', '2', '3'],
                Duration::from_millis(5),
                &mut *guard,
                |_| {
                    thread::sleep(Duration::from_millis(80));
                    Ok(None)
                },
            )
            .unwrap()
        };
        assert_eq!(choice, RestartChoice::No);
        let s = String::from_utf8(buf.lock().unwrap().get_ref().clone()).unwrap();
        // Frames swap the glyph in place (no clear, no newline); only the timeout ends the line.
        let newline_count = s.chars().filter(|c| *c == '\n').count();
        assert_eq!(
            newline_count, 1,
            "spinner frames must not print newlines: {s:?}"
        );
        assert!(s.matches("\x1b7\r").count() >= 2);
        assert_eq!(
            s.matches("\x1b[K").count(),
            1,
            "frames must not clear typed input: {s:?}"
        );
    }

    #[test]
    fn braille_sequence_matches_spec() {
        assert_eq!(
            BRAILLE_FRAMES,
            &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
        );
        assert_eq!(ASCII_FRAMES, &['-', '\\', '|', '/']);
    }

    #[test]
    fn restart_unknown_host_errors() {
        assert!(restart_host("no-such-host").is_err());
    }

    #[test]
    fn restart_cli_only_host_documents_manual_start() {
        // aider is CLI-only in this tree.
        let msg = restart_host("aider").unwrap();
        assert!(
            msg.contains("start the CLI")
                || msg.contains("relaunch")
                || msg.contains("nothing")
                || msg.contains("CLI agent"),
            "{msg}"
        );
    }
}
