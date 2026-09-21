//! Offer to restart a host after `agents install|uninstall` (T76).
//!
//! After config writes finish, ask interactively whether to stop then start that host so the
//! new config is live. The question is an `inquire::Confirm` (T138): default No, Esc / Ctrl-C
//! also mean No, and it waits for an answer. Dry-run and non-interactive stdin skip the prompt
//! entirely.

use std::io::{self, IsTerminal};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use inquire::error::InquireError;
use inquire::ui::RenderConfig;
use owo_colors::{OwoColorize, Stream};

use super::{Agent, Kind, host};
use crate::config::Config;

/// What the user decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartChoice {
    Yes,
    No,
    /// Dry-run, non-TTY, or explicit skip — no prompt was shown.
    Skipped,
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
/// On Yes, runs [`restart_host`]. On No / skip, leaves the process alone.
pub fn offer_host_restart(cfg: &Config, host_id: &str) -> Result<RestartChoice> {
    if !should_prompt(cfg) {
        return Ok(RestartChoice::Skipped);
    }
    let answer = inquire::Confirm::new(&format!(
        "Restart {host_id} so the new config takes effect?"
    ))
    .with_default(false)
    .with_render_config(render_config())
    .prompt();
    match confirm_choice(answer)? {
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

/// Map an inquire answer: Esc / Ctrl-C decline; a TTY that vanished mid-prompt skips; any
/// other error surfaces.
fn confirm_choice(answer: Result<bool, InquireError>) -> Result<RestartChoice> {
    match answer {
        Ok(true) => Ok(RestartChoice::Yes),
        Ok(false) | Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
            Ok(RestartChoice::No)
        }
        Err(InquireError::NotTTY) => Ok(RestartChoice::Skipped),
        Err(e) => Err(e).context("restart prompt"),
    }
}

/// inquire's colours on a colour terminal; plain text under the same rule the rest of rtok
/// follows (owo-colors: tty, `NO_COLOR`, `CLICOLOR`, `TERM=dumb`). inquire draws on stderr.
fn render_config() -> RenderConfig<'static> {
    let probe = "x"
        .if_supports_color(Stream::Stderr, |t| t.bold())
        .to_string();
    if probe == "x" {
        RenderConfig::empty()
    } else {
        RenderConfig::default()
    }
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

    #[test]
    fn should_prompt_requires_interactive_and_not_dry_run() {
        assert!(!should_prompt_with(true, true));
        assert!(!should_prompt_with(false, false));
        assert!(!should_prompt_with(true, false));
        assert!(should_prompt_with(false, true));
    }

    #[test]
    fn dry_run_skips_prompt() {
        let mut c = Config::default();
        c.setup.dry_run = true;
        assert!(!should_prompt(&c));
        let choice = offer_host_restart(&c, "cursor").unwrap();
        assert_eq!(choice, RestartChoice::Skipped);
    }

    #[test]
    fn confirm_answers_map_to_choices() {
        assert_eq!(confirm_choice(Ok(true)).unwrap(), RestartChoice::Yes);
        assert_eq!(confirm_choice(Ok(false)).unwrap(), RestartChoice::No);
        assert_eq!(
            confirm_choice(Err(InquireError::OperationCanceled)).unwrap(),
            RestartChoice::No
        );
        assert_eq!(
            confirm_choice(Err(InquireError::OperationInterrupted)).unwrap(),
            RestartChoice::No
        );
        assert_eq!(
            confirm_choice(Err(InquireError::NotTTY)).unwrap(),
            RestartChoice::Skipped
        );
        assert!(confirm_choice(Err(InquireError::InvalidConfiguration("x".into()))).is_err());
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
