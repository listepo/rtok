//! Close a running desktop app before `agents install|uninstall` writes its config, reopen it
//! after (T141; replaces T76's interactive y/N prompt with an automatic, no-prompt flow).
//!
//! Only [`Kind::Desktop`] apps are ever quit/reopened, and only when both are true: the app is
//! currently running, and the write would actually change something for that host (checked with
//! a dry-run via [`super::would_change`]). CLI-only hosts are never killed — a running CLI
//! binary just gets a one-line reminder to restart its session.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use super::{Agent, Kind, Request, Variant};
use crate::config::Config;

/// Process control seam (T141): the real impl shells out; tests fake it and record calls.
pub trait Procs {
    /// True if a desktop app bundle/window named `name` is currently running. Must not launch
    /// it as a side effect. Not for CLI binaries — on macOS `osascript`'s app-name lookup is
    /// case-insensitive and can match the wrong app (or pop a "Choose Application" dialog for
    /// an unknown name), so a CLI binary is asked about through [`Self::bin_running`] instead.
    fn app_running(&self, name: &str) -> bool;
    /// True if a CLI binary named `bin` is currently running (`pgrep -x` / `tasklist`, never
    /// `osascript`'s app-name check).
    fn bin_running(&self, bin: &str) -> bool;
    /// Ask a running desktop app to quit.
    fn quit(&self, name: &str) -> Result<()>;
    /// Launch a desktop app again.
    fn open(&self, name: &str, path: &Path) -> Result<()>;
}

/// `tasklist /FI "IMAGENAME eq <name>.exe"` (Windows): used for both an app's exe and a CLI
/// binary — unlike `osascript`'s app-name lookup, it matches an exact image name only.
#[cfg(target_os = "windows")]
fn tasklist_running(name: &str) -> bool {
    let exe = if name.ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    };
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {exe}")])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .to_ascii_lowercase()
                .contains(&exe.to_ascii_lowercase())
        })
        .unwrap_or(false)
}

/// `pgrep -x <name>` (macOS/Linux): exact process-name match, used for CLI binaries everywhere
/// and for desktop apps outside macOS (where there is no app-bundle concept to ask `osascript`
/// about).
#[cfg(not(target_os = "windows"))]
fn pgrep_running(name: &str) -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Real OS process control: macOS uses `osascript` (the same name resolution `tell application
/// … to quit` and `open -a` already rely on) for [`Procs::app_running`] only; Windows uses
/// `tasklist`/`taskkill` for both; everything else uses `pgrep`/`killall` for both.
pub struct RealProcs;

impl Procs for RealProcs {
    fn app_running(&self, name: &str) -> bool {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("osascript")
                .args(["-e", &format!("application \"{name}\" is running")])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
                .unwrap_or(false)
        }
        #[cfg(target_os = "windows")]
        {
            tasklist_running(name)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            pgrep_running(name)
        }
    }

    fn bin_running(&self, bin: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            tasklist_running(bin)
        }
        #[cfg(not(target_os = "windows"))]
        {
            pgrep_running(bin)
        }
    }

    fn quit(&self, name: &str) -> Result<()> {
        // Defense in depth: the desktop `apps` lists we act on never name rtok itself, but
        // never let this path kill the process it runs under.
        if name.eq_ignore_ascii_case("rtok") {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("osascript")
                .args(["-e", &format!("tell application \"{name}\" to quit")])
                .status()
                .with_context(|| format!("osascript quit {name}"))?;
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("taskkill")
                .args(["/IM", &format!("{name}.exe"), "/F"])
                .status()
                .with_context(|| format!("taskkill {name}"))?;
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            std::process::Command::new("killall")
                .arg(name)
                .status()
                .with_context(|| format!("killall {name}"))?;
            Ok(())
        }
    }

    fn open(&self, name: &str, path: &Path) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let _ = path;
            std::process::Command::new("open")
                .args(["-a", name])
                .status()
                .with_context(|| format!("open -a {name}"))?;
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", path.to_str().unwrap_or(name)])
                .status()
                .with_context(|| format!("start {name}"))?;
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            if path.exists() {
                std::process::Command::new(path).spawn()?;
            } else {
                std::process::Command::new("xdg-open").arg(name).spawn()?;
            }
            Ok(())
        }
    }
}

/// `~/x` and `$VAR/x` as a path on this machine; anything else unchanged.
fn expand_app_path(spec: &str) -> PathBuf {
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
    PathBuf::from(spec)
}

/// The display/process name our path parsing reads from one `apps[]` entry — the same string
/// asked of [`Procs::app_running`] and handed to [`Procs::quit`]/[`Procs::open`].
fn desktop_app_name(spec: &str, fallback: &str) -> (String, PathBuf) {
    let path = expand_app_path(spec);
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .map(|n| n.strip_suffix(".app").unwrap_or(n).to_string())
        .unwrap_or_else(|| fallback.to_string());
    (name, path)
}

/// The first `apps[]` entry of a desktop variant that `procs` reports as currently running.
fn running_desktop_app(v: &Variant, procs: &dyn Procs) -> Option<(String, PathBuf)> {
    v.apps
        .iter()
        .map(|spec| desktop_app_name(spec, v.name))
        .find(|(name, _)| procs.app_running(name))
}

/// The first CLI binary of a host that `procs` reports as currently running.
fn running_cli_bin(agent: &dyn Agent, procs: &dyn Procs) -> Option<&'static str> {
    agent
        .variants()
        .iter()
        .filter(|v| v.kind == Kind::Cli)
        .find_map(|v| v.bins.iter().copied().find(|b| procs.bin_running(b)))
}

/// Poll `procs.app_running(name)` until it goes false or `timeout` elapses.
fn wait_until_not_running(procs: &dyn Procs, name: &str, timeout: Duration) {
    let started = std::time::Instant::now();
    while procs.app_running(name) && started.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// `rtok agents install|uninstall`: quit a running desktop app before the write if it would
/// actually change that host's config, write, then reopen it. Never touches a CLI-only host's
/// process; if one is running it just prints a reminder. Fails open — a quit/reopen failure is
/// a warning, never a command failure.
pub fn run(cfg: &mut Config, req: &Request, no_restart: bool) -> Result<String> {
    with_restart(
        cfg,
        req,
        no_restart,
        &RealProcs,
        |c, a| super::would_change(c, a, req),
        |c| super::run(c, req),
    )
}

/// Test seam for [`run`]: `procs`, `would_change` and `write` are injected so orchestration
/// tests never shell out or touch a real host's files.
fn with_restart(
    cfg: &mut Config,
    req: &Request,
    no_restart: bool,
    procs: &dyn Procs,
    would_change: impl Fn(&Config, &'static dyn Agent) -> Result<bool>,
    write: impl FnOnce(&mut Config) -> Result<String>,
) -> Result<String> {
    let mut out = String::new();
    let mut to_reopen: Vec<(String, PathBuf)> = Vec::new();
    if !no_restart {
        let agents = super::resolve(&req.hosts)?;
        for &agent in &agents {
            let target = agent
                .variants()
                .iter()
                .filter(|v| v.kind == Kind::Desktop)
                .find_map(|v| running_desktop_app(v, procs));
            let bin = running_cli_bin(agent, procs);
            // Nothing of this host's is running: skip the dry-run entirely (T141 review).
            if target.is_none() && bin.is_none() {
                continue;
            }
            let changed = would_change(cfg, agent)?;
            if let Some((name, path)) = target
                && changed
            {
                if cfg.setup.dry_run {
                    out.push_str(&format!("{}: would close/reopen {name}\n", agent.id()));
                } else if let Err(e) = procs.quit(&name) {
                    eprintln!(
                        "warning: could not quit {name}: {e:#}; restart it manually to load the new config"
                    );
                } else {
                    wait_until_not_running(procs, &name, Duration::from_secs(5));
                    to_reopen.push((name, path));
                }
            }
            if changed && let Some(bin) = bin {
                out.push_str(&format!(
                    "{}: restart your {bin} session to load the new config\n",
                    agent.id()
                ));
            }
        }
    }
    // Always reopen what was quit — even when the write itself fails — before propagating
    // the write's error (T141 review): a write failure must never leave a closed app closed.
    let written = write(cfg);
    for (name, path) in to_reopen {
        if let Err(e) = procs.open(&name, &path) {
            eprintln!("warning: could not reopen {name}: {e:#}; open it manually");
        }
    }
    out.push_str(&written?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Mode;
    use std::cell::RefCell;
    use std::collections::HashSet;

    #[derive(Default)]
    struct FakeProcs {
        running: RefCell<HashSet<String>>,
        calls: RefCell<Vec<String>>,
        fail_quit: bool,
    }

    impl FakeProcs {
        fn running_names(names: &[&str]) -> Self {
            Self {
                running: RefCell::new(names.iter().map(|s| s.to_string()).collect()),
                ..Default::default()
            }
        }
    }

    impl Procs for FakeProcs {
        fn app_running(&self, name: &str) -> bool {
            self.running.borrow().contains(name)
        }
        fn bin_running(&self, bin: &str) -> bool {
            self.running.borrow().contains(bin)
        }
        fn quit(&self, name: &str) -> Result<()> {
            self.calls.borrow_mut().push(format!("quit:{name}"));
            if self.fail_quit {
                anyhow::bail!("boom");
            }
            self.running.borrow_mut().remove(name);
            Ok(())
        }
        fn open(&self, name: &str, _path: &Path) -> Result<()> {
            self.calls.borrow_mut().push(format!("open:{name}"));
            Ok(())
        }
    }

    fn req(host: &str) -> Request {
        Request {
            hosts: vec![host.to_string()],
            mode: Mode::Install,
            cli: false,
            desktop: false,
            all: true,
        }
    }

    /// [`with_restart`] for the "windsurf" host with a fixed `would_change` answer and a
    /// no-op write — every test below that does not need to observe the write's position in
    /// the call order goes through here instead of repeating the same call shape.
    fn call(
        cfg: &mut Config,
        procs: &FakeProcs,
        no_restart: bool,
        changed: bool,
    ) -> Result<String> {
        with_restart(
            cfg,
            &req("windsurf"),
            no_restart,
            procs,
            |_, _| Ok(changed),
            |_| Ok(String::new()),
        )
    }

    /// The write closure pushes into the same call log as quit/open, so one assertion proves
    /// the order: quit before write, write before reopen.
    #[test]
    fn running_and_changed_quits_writes_then_reopens_in_order() {
        let procs = FakeProcs::running_names(&["Windsurf"]);
        let mut cfg = Config::default();
        with_restart(
            &mut cfg,
            &req("windsurf"),
            false,
            &procs,
            |_, _| Ok(true),
            |_| {
                procs.calls.borrow_mut().push("write".into());
                Ok(String::new())
            },
        )
        .unwrap();
        assert_eq!(
            procs.calls.borrow().as_slice(),
            [
                "quit:Windsurf".to_string(),
                "write".to_string(),
                "open:Windsurf".to_string()
            ]
        );
    }

    #[test]
    fn running_but_unchanged_does_nothing() {
        let procs = FakeProcs::running_names(&["Windsurf"]);
        call(&mut Config::default(), &procs, false, false).unwrap();
        assert!(procs.calls.borrow().is_empty());
    }

    #[test]
    fn not_running_writes_only() {
        let procs = FakeProcs::default();
        call(&mut Config::default(), &procs, false, true).unwrap();
        assert!(procs.calls.borrow().is_empty());
    }

    #[test]
    fn dry_run_prints_would_close_and_makes_no_calls() {
        let procs = FakeProcs::running_names(&["Windsurf"]);
        let mut cfg = Config::default();
        cfg.setup.dry_run = true;
        let out = call(&mut cfg, &procs, false, true).unwrap();
        assert!(procs.calls.borrow().is_empty());
        assert!(out.contains("would close/reopen Windsurf"), "{out}");
    }

    #[test]
    fn no_restart_flag_writes_only() {
        let procs = FakeProcs::running_names(&["Windsurf"]);
        call(&mut Config::default(), &procs, true, true).unwrap();
        assert!(procs.calls.borrow().is_empty());
    }

    #[test]
    fn quit_failure_still_writes_and_warns_instead_of_erroring() {
        let procs = FakeProcs {
            fail_quit: true,
            ..FakeProcs::running_names(&["Windsurf"])
        };
        let mut cfg = Config::default();
        let out = with_restart(
            &mut cfg,
            &req("windsurf"),
            false,
            &procs,
            |_, _| Ok(true),
            |_| {
                procs.calls.borrow_mut().push("write".into());
                Ok(String::new())
            },
        )
        .unwrap();
        // No reopen call: a failed quit still writes, but nothing was closed to reopen.
        assert_eq!(
            procs.calls.borrow().as_slice(),
            ["quit:Windsurf".to_string(), "write".to_string()]
        );
        assert!(out.is_empty());
    }

    #[test]
    fn write_failure_still_reopens_then_propagates_the_error() {
        let procs = FakeProcs::running_names(&["Windsurf"]);
        let mut cfg = Config::default();
        let err = with_restart(
            &mut cfg,
            &req("windsurf"),
            false,
            &procs,
            |_, _| Ok(true),
            |_| anyhow::bail!("disk full"),
        )
        .unwrap_err();
        // Quit, then reopen despite the write error, then the error propagates — a write
        // failure must never leave the app closed (T141 review).
        assert_eq!(
            procs.calls.borrow().as_slice(),
            ["quit:Windsurf".to_string(), "open:Windsurf".to_string()]
        );
        assert!(err.to_string().contains("disk full"), "{err:#}");
    }

    #[test]
    fn cli_only_host_running_prints_message_and_is_never_killed() {
        let procs = FakeProcs::running_names(&["aider"]);
        let mut cfg = Config::default();
        let out = with_restart(
            &mut cfg,
            &req("aider"),
            false,
            &procs,
            |_, _| Ok(true),
            |_| Ok(String::new()),
        )
        .unwrap();
        assert!(procs.calls.borrow().is_empty());
        assert!(out.contains("aider: restart your aider session"), "{out}");
    }

    /// One row per host with a `Kind::Desktop` variant, naming what `apps[0]` (always the
    /// macOS entry in this tree) should resolve to. A host added here without a row panics
    /// instead of silently skipping restart coverage.
    fn expected_desktop_name(variant: &str) -> &'static str {
        match variant {
            "Antigravity" => "Antigravity",
            "Claude Desktop" => "Claude",
            "GitHub Copilot" => "GitHub Copilot",
            "Cursor" => "Cursor",
            "Cline for VS Code" => "Visual Studio Code",
            "Kilo Code for VS Code" => "Visual Studio Code",
            "Kimi Code Desktop" => "Kimi Code",
            "OpenCode Desktop" => "OpenCode",
            "VS Code" => "Visual Studio Code",
            "VS Code - Insiders" => "Visual Studio Code - Insiders",
            "Windsurf" => "Windsurf",
            "ZCode" => "ZCode",
            "Zed" => "Zed",
            other => panic!("desktop variant `{other}` has no expected-name row (T141) — add one"),
        }
    }

    #[test]
    fn every_desktop_variant_of_every_host_resolves_its_known_app_name() {
        for &id in crate::agents::HOSTS {
            let agent = crate::agents::host(id).unwrap();
            let mut had_desktop = false;
            for v in agent.variants().iter().filter(|v| v.kind == Kind::Desktop) {
                had_desktop = true;
                assert!(!v.apps.is_empty(), "{id}/{} has no apps[]", v.name);
                let (name, _) = desktop_app_name(v.apps[0], v.name);
                assert_eq!(name, expected_desktop_name(v.name), "host {id}");
            }
            if !had_desktop {
                // CLI-only host: pretend every bin is running and confirm it never yields a
                // quit/open call — running_desktop_app always returns None for it.
                let procs = FakeProcs::running_names(
                    &agent
                        .variants()
                        .iter()
                        .flat_map(|v| v.bins.iter().copied())
                        .collect::<Vec<_>>(),
                );
                assert!(
                    agent
                        .variants()
                        .iter()
                        .filter(|v| v.kind == Kind::Desktop)
                        .find_map(|v| running_desktop_app(v, &procs))
                        .is_none(),
                    "CLI-only host {id} must never produce a desktop restart target"
                );
            }
        }
    }
}
