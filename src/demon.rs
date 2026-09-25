//! `rtok demon` (plan T20.1, decision D22) — keeps rtok's long-running surfaces up.
//!
//! One supervisor process per service. `start` detaches `rtok demon supervise <name>`; that
//! process re-spawns `rtok <name>` every time the child exits, and stops only when `stop` drops
//! a `<name>.stop` marker beside the state file. `rtok hook` never reads this state and fails open
//! whether a supervisor is up or not (D1); the `hook` service only keeps the optional resident
//! `rtok hook --serve` up, which `rtok-hook` uses when it answers and bypasses when not (D32).

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// The only services a supervisor may run — rtok's own long-running surfaces. Clap validates the
/// CLI side from this enum (D14 derive API: help, completions and the error message come free),
/// so neither an argument nor a config file can turn `start` into "run this command".
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Service {
    Proxy,
    Mcp,
    Web,
    Hook,
}

impl Service {
    /// The subcommand the supervisor spawns, and the stem of its files.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::Mcp => "mcp",
            Self::Web => "web",
            Self::Hook => "hook",
        }
    }

    /// What the supervisor runs: the subcommand, plus `--serve` for the resident hook (D32).
    fn args(self) -> &'static [&'static str] {
        match self {
            Self::Proxy => &["proxy"],
            Self::Mcp => &["mcp"],
            Self::Web => &["web"],
            Self::Hook => &["hook", "--serve"],
        }
    }

    /// A name out of `[demon] services`. Clap owns the same list, so there is one source of truth.
    fn parse(name: &str) -> Result<Self> {
        <Self as ValueEnum>::from_str(name, true)
            .map_err(|e| anyhow::anyhow!("[demon] services: {e}"))
    }
}

impl std::fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What `status` and `list` read. The supervisor is the only writer.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    service: Service,
    supervisor: i32,
    child: i32,
    /// Unix seconds the current child started.
    since: u64,
    restarts: u32,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn file(cfg: &Config, service: Service, ext: &str) -> PathBuf {
    cfg.demon.state_dir.join(format!("{service}.{ext}"))
}

fn read(cfg: &Config, service: Service) -> Option<State> {
    serde_json::from_str(&fs::read_to_string(file(cfg, service, "json")).ok()?).ok()
}

/// Atomic swap, never truncate-then-write: `status`, `upgrade` and `quiesce` read this file while
/// the supervisor rewrites it on every restart, and a half-written file parses as "not running".
fn write(cfg: &Config, st: &State) -> Result<()> {
    let path = file(cfg, st.service, "json");
    rtok_agent_sdk::write_atomic(&path, &serde_json::to_string_pretty(st)?)
        .with_context(|| path.display().to_string())
}

/// True while `pid` is a live process. A state file left behind by a supervisor that was
/// killed from outside reads as stopped instead of as whatever it said.
fn alive(pid: i32) -> bool {
    rtok_sys::process_alive(pid)
}

/// The services a verb acts on: the ones named, else — for the verbs that act on what is
/// already up — every service with a state file, else `[demon] services`.
fn targets(cfg: &Config, named: &[Service], running_first: bool) -> Result<Vec<Service>> {
    if !named.is_empty() {
        return Ok(named.to_vec());
    }
    if running_first {
        let up: Vec<Service> = Service::value_variants()
            .iter()
            .copied()
            .filter(|s| read(cfg, *s).is_some())
            .collect();
        if !up.is_empty() {
            return Ok(up);
        }
    }
    cfg.demon
        .services
        .iter()
        .map(|n| Service::parse(n))
        .collect()
}

/// Detach one supervisor per service. A service that is already up is left alone rather than
/// given a second supervisor, which would give the same port two owners.
pub fn start(cfg: &Config, config_file: Option<&Path>, named: &[Service]) -> Result<()> {
    fs::create_dir_all(&cfg.demon.state_dir)?;
    let exe = on_disk_exe()?;
    // The supervisor below outlives this command; on Windows it would otherwise inherit
    // whatever piped our own stdout/stderr (a test harness, a captured parent) and hold that
    // pipe open forever, so the piper's read to EOF never returns (T83.3). No-op on Unix.
    rtok_sys::stop_inheriting_own_stdio();
    for service in targets(cfg, named, false)? {
        // The supervisor's own lock is the truth; the state file can lag it or name a reused pid.
        if claim(cfg, service)?.is_none() {
            println!("{service} already running");
            continue;
        }
        if let Some(st) = read(cfg, service) {
            // The lock just proved no supervisor is up, so the state file is stale; its pids
            // are only worth signalling if they date from this boot — after a reboot the
            // kernel hands the same numbers to unrelated processes.
            let this_boot = boot_time().is_some_and(|boot| st.since >= boot);
            if !this_boot {
                println!(
                    "{service}: stale state from before this boot, not signalling pid {}",
                    st.child
                );
            }
            // A supervisor that died from outside can leave its child up. Starting another
            // supervisor would put two owners on the same port — retire the orphan first.
            if this_boot && alive(st.child) {
                rtok_sys::process_term(st.child);
                for _ in 0..40 {
                    if !alive(st.child) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if alive(st.child) {
                    rtok_sys::process_kill(st.child);
                }
            }
            let _ = fs::remove_file(file(cfg, service, "json"));
        }
        let _ = fs::remove_file(file(cfg, service, "stop"));
        if service == Service::Mcp {
            eprintln!(
                "{service}: no stdin client under the demon — `rtok mcp` exits at EOF and is restarted"
            );
        }
        let mut cmd = Command::new(&exe);
        if let Some(c) = config_file {
            cmd.arg("--config").arg(c);
        }
        let child = cmd
            .args(["demon", "supervise", service.as_str()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawn supervisor for {service}"))?;
        println!("{service} started (supervisor {})", child.id());
    }
    Ok(())
}

/// Ask each supervisor to go away, then make sure it did. The marker is written *before* the
/// signals: a supervisor that wakes up between them must not start one more child.
pub fn stop(cfg: &Config, named: &[Service], force: bool) -> Result<()> {
    for service in targets(cfg, named, true)? {
        let Some(st) = read(cfg, service) else {
            println!("{service} not running");
            continue;
        };
        fs::write(file(cfg, service, "stop"), b"")?;
        if force {
            rtok_sys::process_kill(st.supervisor);
            rtok_sys::process_kill(st.child);
        } else {
            rtok_sys::process_term(st.supervisor);
            rtok_sys::process_term(st.child);
        }
        for _ in 0..40 {
            if !alive(st.supervisor) && !alive(st.child) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // Last resort: a supervisor that ignored SIGTERM would otherwise outlive its own state.
        if alive(st.supervisor) || alive(st.child) {
            rtok_sys::process_kill(st.supervisor);
            rtok_sys::process_kill(st.child);
        }
        let _ = fs::remove_file(file(cfg, service, "json"));
        println!("{service} stopped");
    }
    Ok(())
}

pub fn restart(cfg: &Config, config_file: Option<&Path>, named: &[Service]) -> Result<()> {
    let services = targets(cfg, named, true)?;
    stop(cfg, &services, false)?;
    start(cfg, config_file, &services)
}

/// Keep what [`stop`] stopped down until the replace runs. A supervisor that lost the race
/// between its child's death and its own signal can still have respawned the surface — the
/// respawn rewrites its state file and re-locks the store the replace is about to touch
/// (T73). Retire the respawn with the same this-boot guard as [`start`], drop the state
/// file, and return only once every stopped service's file is gone and stays gone; a
/// surface that will not stay down fails the upgrade instead of racing the replace.
fn quiesce(cfg: &Config, services: &[Service]) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let mut rogue = false;
        for service in services {
            let Some(st) = read(cfg, *service) else {
                continue;
            };
            rogue = true;
            if boot_time().is_some_and(|boot| st.since >= boot) {
                println!(
                    "{service}: respawned during the replace window, retiring pid {}",
                    st.child
                );
                rtok_sys::process_kill(st.supervisor);
                rtok_sys::process_kill(st.child);
            }
            let _ = fs::remove_file(file(cfg, *service, "json"));
        }
        if !rogue {
            return Ok(());
        }
        if Instant::now() > deadline {
            anyhow::bail!("demon surfaces did not stay down for the replace");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Stop every live surface (HTTP/WS `web`, `mcp`, `proxy` — SQLite drops with them),
/// replace the on-disk binary, then start the same set. A failed replace still starts.
pub fn upgrade(cfg: &Config, config_file: Option<&Path>) -> Result<()> {
    let up: Vec<Service> = rows(cfg, &[])?
        .into_iter()
        .filter(|r| r.running)
        .map(|r| r.service)
        .collect();
    // The stop folds into `replaced` so a surface that will not stay down still gets its
    // restart: returning before `start` would leave the machine without what was up.
    let replaced = (|| -> Result<()> {
        if !up.is_empty() {
            stop(cfg, &up, false)?;
            quiesce(cfg, &up)?;
        }
        replace_binary()
    })();
    let started = if up.is_empty() {
        Ok(())
    } else {
        start(cfg, config_file, &up)
    };
    match (replaced, started) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), Ok(())) => Err(e),
        (Ok(()), Err(e)) => Err(e),
        (Err(e), Err(s)) => Err(e).with_context(|| format!("and restart failed: {s}")),
    }
}

/// The path `start` should spawn: `current_exe` after an in-place replace can be the
/// deleted inode, while the same path on disk already holds the new file.
fn on_disk_exe() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    if exe.is_file() {
        Ok(exe)
    } else {
        Ok(PathBuf::from("rtok"))
    }
}

fn replace_binary() -> Result<()> {
    if let Some(cmd) = std::env::var_os("RTOK_UPDATE_CMD") {
        return run_replace(Command::new(cmd));
    }
    match Command::new("ketch")
        .args(["upgrade", "rtok", "--yes"])
        .status()
    {
        Ok(st) if st.success() => return Ok(()),
        Ok(st) => anyhow::bail!("ketch upgrade rtok failed ({st})"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    match Command::new("rtok-update").status() {
        Ok(st) if st.success() => Ok(()),
        Ok(st) => anyhow::bail!("rtok-update failed ({st})"),
        Err(_) => anyhow::bail!(
            "no ketch or rtok-update — install with ketch (`ketch install listepo/rtok`)"
        ),
    }
}

fn run_replace(mut cmd: Command) -> Result<()> {
    let st = cmd.status()?;
    if st.success() {
        Ok(())
    } else {
        anyhow::bail!("update command failed ({st})")
    }
}

/// One row of the `rtok demon status` page (T15.11): state asked of the kernel, never
/// copied out of the state file.
#[derive(Debug, Serialize)]
pub struct Row {
    pub service: Service,
    pub running: bool,
    /// The address a service listens on: the proxy's configured `[proxy] bind:port`, named
    /// whether it is running or not; `None` for a non-listening surface (`mcp`). Read from
    /// this process's config — a supervisor started with `--config` may sit elsewhere.
    pub endpoint: Option<String>,
    /// `None` when the service is down: the columns render `-`.
    pub supervisor: Option<i32>,
    pub child: Option<i32>,
    pub uptime_secs: Option<u64>,
    pub restarts: Option<u32>,
    pub log: PathBuf,
}

/// The query behind `rtok demon status`: one row per named service, or every service when none
/// is named, so a stopped one reads as stopped rather than going missing. Liveness is the
/// supervisor's `flock`, which dies with it, so a state file left behind by a killed
/// supervisor reads as stopped instead of as whatever it said — even when its pid was reused.
pub fn rows(cfg: &Config, named: &[Service]) -> Result<Vec<Row>> {
    let services = if named.is_empty() {
        Service::value_variants()
    } else {
        named
    };
    let mut out = Vec::new();
    for &service in services {
        let st = read(cfg, service);
        // `Ok(None)` is "held elsewhere"; the probe's own lock drops at once.
        let up = st.is_some() && matches!(claim(cfg, service), Ok(None));
        let live = |s: &State| {
            (
                Some(s.supervisor),
                Some(s.child),
                Some(now().saturating_sub(s.since)),
                Some(s.restarts),
            )
        };
        let (supervisor, child, uptime_secs, restarts) = match st.as_ref().filter(|_| up) {
            Some(s) => live(s),
            None => (None, None, None, None),
        };
        let endpoint =
            (service == Service::Proxy).then(|| format!("{}:{}", cfg.proxy.bind, cfg.proxy.port));
        out.push(Row {
            service,
            running: up,
            endpoint,
            supervisor,
            child,
            uptime_secs,
            restarts,
            log: file(cfg, service, "log"),
        });
    }
    Ok(out)
}

/// The rendering: header plus one coloured row per service.
///
/// Colour the bare state word first, then pad with plain spaces. Colouring a
/// pre-padded string strips the trailing pad (owo-colors), so `proxy` and
/// `stopped` used to glue into `proxystopped`. Pad width is the visible word
/// length, never the ANSI byte length.
pub fn table(rows: &[Row]) -> String {
    // Fixed floors keep the header readable; content can grow (pids, paths).
    const SERVICE: usize = 9;
    const STATE: usize = 9;
    const ENDPOINT: usize = 16;
    const SUPERVISOR: usize = 10;
    const CHILD: usize = 8;
    const UPTIME: usize = 8;
    const RESTARTS: usize = 8;

    fn pad_left(cell: &str, width: usize) -> String {
        format!("{cell:<width$}")
    }

    /// Colour, then pad with plain spaces so trailing pad survives ANSI.
    fn state_cell(running: bool, width: usize) -> String {
        let word = if running { "running" } else { "stopped" };
        let colored = crate::render::state(word, running);
        let pad = " ".repeat(width.saturating_sub(word.len()));
        format!("{colored}{pad}")
    }

    let dash = || "-".to_string();
    let sep = "  ";
    let mut out = format!(
        "{1}{0}{2}{0}{3}{0}{4}{0}{5}{0}{6}{0}{7}{0}log\n",
        sep,
        pad_left("service", SERVICE),
        pad_left("state", STATE),
        pad_left("endpoint", ENDPOINT),
        pad_left("supervisor", SUPERVISOR),
        pad_left("child", CHILD),
        pad_left("uptime", UPTIME),
        pad_left("restarts", RESTARTS),
    );
    for r in rows {
        out.push_str(&format!(
            "{1}{0}{2}{0}{3}{0}{4}{0}{5}{0}{6}{0}{7}{0}{8}\n",
            sep,
            pad_left(&r.service.to_string(), SERVICE),
            state_cell(r.running, STATE),
            pad_left(r.endpoint.as_deref().unwrap_or("-"), ENDPOINT),
            pad_left(
                &r.supervisor.map(|v| v.to_string()).unwrap_or_else(dash),
                SUPERVISOR
            ),
            pad_left(&r.child.map(|v| v.to_string()).unwrap_or_else(dash), CHILD),
            pad_left(
                &r.uptime_secs.map(|s| format!("{s}s")).unwrap_or_else(dash),
                UPTIME
            ),
            pad_left(
                &r.restarts.map(|v| v.to_string()).unwrap_or_else(dash),
                RESTARTS
            ),
            r.log.display()
        ));
    }
    out
}

/// Read `stream` line by line, forwarding each as `(level, line)`. Runs on its own thread so
/// the supervisor's poll loop never blocks on a child's pipe; a send failure only means the
/// receiving end already went away, which happens once the drain loop is done.
fn pump(stream: impl Read, level: &'static str, tx: &Sender<(&'static str, String)>) {
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        let _ = tx.send((level, line));
    }
}

/// Write every line waiting on `rx` through the T24.0 sink, without blocking for more.
fn drain(rx: &Receiver<(&'static str, String)>, log_cfg: &Config, service: Service) {
    while let Ok((level, line)) = rx.try_recv() {
        crate::log::append(log_cfg, level, "demon", service.as_str(), &line);
    }
}

/// The detached half: spawn the service, wait, spawn it again. Runs until the stop marker.
/// `config_file` is passed straight down, so the service reads the file the operator started the
/// supervisor with rather than whatever the default layers resolve to.
pub fn supervise(cfg: &Config, config_file: Option<&Path>, service: Service) -> Result<()> {
    fs::create_dir_all(&cfg.demon.state_dir)?;
    // One supervisor per service, however it was started: two `demon start` runs could both
    // pass the state-file check before either supervisor wrote its state, and put two owners
    // on one port. A second supervisor leaves here without touching the state file.
    let Some(_lock) = claim(cfg, service)? else {
        return Ok(());
    };
    // Its own session, so closing the terminal that ran `start` does not take the tree down.
    rtok_sys::setsid();
    let exe = std::env::current_exe()?;
    let stop = file(cfg, service, "stop");
    let mut st = State {
        service,
        supervisor: std::process::id() as i32,
        child: 0,
        since: now(),
        restarts: 0,
    };
    let mut backoff = cfg.demon.backoff_ms;
    while !stop.exists() {
        // The child's own log config: same rotation settings, but pointed at its file rather
        // than rtok's own. A raw appending fd (the old approach) is a file the child holds
        // open, which nothing can ever rotate out from under it — piping stdout/stderr through
        // the T24.0 sink instead is what makes rotation possible at all.
        let mut log_cfg = cfg.clone();
        log_cfg.log.path = file(cfg, service, "log");
        let mut cmd = Command::new(&exe);
        if let Some(c) = config_file {
            cmd.arg("--config").arg(c);
        }
        let mut child = cmd
            .args(service.args())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn {service}"))?;
        // Two reader threads only forward lines over a channel; the main thread is the sole
        // writer, so two streams landing on the same file are never a rotate/append race.
        let (tx, rx) = mpsc::channel();
        let out_tx = tx.clone();
        let out = child.stdout.take().expect("piped stdout");
        let out_handle = std::thread::spawn(move || pump(out, "info", &out_tx));
        let err = child.stderr.take().expect("piped stderr");
        let err_handle = std::thread::spawn(move || pump(err, "warn", &tx));
        st.child = child.id() as i32;
        st.since = now();
        write(cfg, &st)?;
        let started = Instant::now();
        loop {
            drain(&rx, &log_cfg, service);
            if stop.exists() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_handle.join();
                let _ = err_handle.join();
                drain(&rx, &log_cfg, service);
                let _ = fs::remove_file(file(cfg, service, "json"));
                return Ok(());
            }
            if child.try_wait()?.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(cfg.demon.poll_ms));
        }
        // The pipes close when the child exits, so both readers reach EOF and finish on their
        // own; joining just makes sure every line they already read is drained before restart.
        let _ = out_handle.join();
        let _ = err_handle.join();
        drain(&rx, &log_cfg, service);
        let healthy = started.elapsed() >= Duration::from_millis(cfg.demon.healthy_ms);
        let wait;
        (wait, backoff) = next_backoff(backoff, healthy, &cfg.demon);
        st.restarts += 1;
        write(cfg, &st)?;
        std::thread::sleep(Duration::from_millis(wait));
    }
    let _ = fs::remove_file(file(cfg, service, "json"));
    Ok(())
}

/// An exclusive non-blocking `flock` on `<service>.lock`, held until the file drops; `None`
/// while another supervisor holds it. std opens files close-on-exec, so the child service
/// never inherits the lock and cannot keep it after its supervisor is gone.
fn claim(cfg: &Config, service: Service) -> Result<Option<fs::File>> {
    let path = file(cfg, service, "lock");
    let f = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .with_context(|| path.display().to_string())?;
    match rtok_sys::try_lock_exclusive(&f)? {
        true => Ok(Some(f)),
        false => Ok(None),
    }
}

/// Unix seconds the machine booted, where a file read tells: `/proc/stat` on Linux,
/// `sysctl kern.boottime` on macOS. `None` elsewhere, and a stale pid is then never signalled.
fn boot_time() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        fs::read_to_string("/proc/stat")
            .ok()?
            .lines()
            .find_map(|l| l.strip_prefix("btime ")?.trim().parse().ok())
    }
    #[cfg(target_os = "macos")]
    {
        // `{ sec = 1700000000, usec = 0 } Tue Nov 14 ...`
        let out = Command::new("sysctl")
            .args(["-n", "kern.boottime"])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let rest = s.split_once("sec = ")?.1;
        rest.split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// `(wait now, wait after the next fast crash)`. A child that stayed up was healthy and resets
/// to `backoff_ms`; only a crash loop doubles, capped at `max_backoff_ms`. The doubling used to
/// run before the first sleep, so the documented 1 s first wait was 2 s.
fn next_backoff(cur: u64, healthy: bool, d: &crate::config::Demon) -> (u64, u64) {
    let wait = if healthy { d.backoff_ms } else { cur };
    (wait, wait.saturating_mul(2).min(d.max_backoff_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiesce_retires_a_late_respawn_state_file() {
        let mut cfg = Config::default();
        cfg.demon.state_dir = crate::testutil::tmp_dir("demon-quiesce");
        // A respawned state from this boot with already-dead pids: quiesce must take the
        // rogue path (kill is a no-op on a reaped pid), drop the file, and return on the
        // next pass once nothing rewrites it.
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let dead = child.id() as i32;
        child.wait().unwrap();
        fs::write(
            file(&cfg, Service::Mcp, "json"),
            serde_json::to_vec(&State {
                service: Service::Mcp,
                supervisor: dead,
                child: dead,
                since: now(),
                restarts: 1,
            })
            .unwrap(),
        )
        .unwrap();
        quiesce(&cfg, &[Service::Mcp]).unwrap();
        assert!(!file(&cfg, Service::Mcp, "json").exists());
    }

    #[test]
    fn quiesce_is_ok_when_nothing_respawned() {
        let mut cfg = Config::default();
        cfg.demon.state_dir = crate::testutil::tmp_dir("demon-quiesce-empty");
        quiesce(&cfg, &[Service::Mcp, Service::Web]).unwrap();
    }

    #[test]
    fn the_first_crash_waits_backoff_ms_then_doubles_to_the_cap() {
        let d = crate::config::Demon {
            backoff_ms: 1000,
            max_backoff_ms: 3000,
            ..Config::default().demon
        };
        let mut cur = d.backoff_ms;
        let mut waits = Vec::new();
        for _ in 0..4 {
            let wait;
            (wait, cur) = next_backoff(cur, false, &d);
            waits.push(wait);
        }
        assert_eq!(waits, [1000, 2000, 3000, 3000]);
        assert_eq!(next_backoff(cur, true, &d).0, 1000, "a healthy run resets");
    }

    #[test]
    fn a_service_name_is_never_an_argv() {
        assert_eq!(Service::parse("PROXY").unwrap(), Service::Proxy);
        let e = Service::parse("rm -rf ~").unwrap_err().to_string();
        assert!(e.contains("[demon] services"), "{e}");
    }

    /// Colouring must not eat the state column's trailing pad — that was how
    /// `proxy` and `stopped` glued into `proxystopped` on a colouring terminal.
    #[test]
    fn status_table_keeps_a_gap_between_service_and_state() {
        let rows = [Row {
            service: Service::Proxy,
            running: false,
            endpoint: None,
            supervisor: None,
            child: None,
            uptime_secs: None,
            restarts: None,
            log: PathBuf::from("/tmp/proxy.log"),
        }];
        let text = table(&rows);
        let body = text.lines().nth(1).expect("row");
        // Strip ANSI so the assertion is about visible columns, not escape bytes.
        let mut visible = String::new();
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for x in chars.by_ref() {
                        if x.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                continue;
            }
            visible.push(c);
        }
        assert!(
            !visible.contains("proxystopped"),
            "service and state must not glue: {visible:?}"
        );
        let after_proxy = &visible[visible.find("proxy").expect("proxy") + "proxy".len()..];
        let stopped_at = after_proxy.find("stopped").expect("stopped");
        assert!(
            after_proxy[..stopped_at].chars().all(|c| c.is_whitespace()),
            "gap between proxy and stopped must be whitespace: {visible:?}"
        );
    }

    /// The proxy row names its `[proxy] bind:port` even while stopped — the address is
    /// config, not state, so it is exactly what a stopped row should still tell you.
    #[test]
    fn the_proxy_row_names_its_configured_endpoint_stopped_or_not() {
        let mut cfg = Config::default();
        cfg.demon.state_dir = crate::testutil::tmp_dir("demon-endpoint");
        cfg.proxy.bind = "127.0.0.2".into();
        cfg.proxy.port = 8123;
        let rows = rows(&cfg, &[]).unwrap();
        let proxy = rows.iter().find(|r| r.service == Service::Proxy).unwrap();
        assert!(!proxy.running);
        assert_eq!(proxy.endpoint.as_deref(), Some("127.0.0.2:8123"));
        assert!(
            rows.iter()
                .find(|r| r.service == Service::Mcp)
                .unwrap()
                .endpoint
                .is_none(),
            "mcp is stdio, it has no address"
        );
        let text = table(&rows);
        assert!(text.contains("endpoint"), "{text}");
        assert!(text.contains("127.0.0.2:8123"), "{text}");
    }

    /// The plumbing `supervise` wires up: a child's output, pumped through the channel and
    /// drained through the T24.0 sink, rotates the same as any other log once it passes
    /// `max_bytes` — a real child's own stdout volume isn't something a test can dial in, so
    /// this drives `pump`/`drain` directly with a synthetic stream instead of a subprocess.
    #[test]
    fn a_service_that_writes_past_max_bytes_gets_a_rotated_log() {
        let dir = std::env::temp_dir().join(format!("rtok-demon-rotate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut cfg = Config::default();
        cfg.demon.state_dir = dir.clone();
        cfg.log.max_bytes = 200;
        cfg.log.files = 2;
        cfg.log.level = "debug".into();
        let mut log_cfg = cfg.clone();
        log_cfg.log.path = file(&cfg, Service::Mcp, "log");

        let (tx, rx) = mpsc::channel();
        let body: String = (0..50).map(|i| format!("line {i}\n")).collect();
        pump(std::io::Cursor::new(body), "info", &tx);
        drop(tx);
        drain(&rx, &log_cfg, Service::Mcp);

        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().any(|n| n == "mcp.log"), "{names:?}");
        assert!(names.iter().any(|n| n == "mcp.log.1"), "{names:?}");
        let live = fs::metadata(file(&cfg, Service::Mcp, "log")).unwrap().len();
        assert!(live <= 200, "the live file is bounded: {live}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A second supervisor for a service that already has one returns before its first
    /// spawn and writes no state.
    #[test]
    fn a_second_supervisor_for_one_service_leaves_at_once() {
        let dir = std::env::temp_dir().join(format!("rtok-demon-claim-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut cfg = Config::default();
        cfg.demon.state_dir = dir.clone();
        fs::create_dir_all(&dir).unwrap();
        let held = claim(&cfg, Service::Web).unwrap().expect("first claim");
        assert!(claim(&cfg, Service::Web).unwrap().is_none());
        supervise(&cfg, None, Service::Web).unwrap();
        assert!(read(&cfg, Service::Web).is_none());
        drop(held);
        // A child spawned by a parallel test can hold the inherited lock fd for the instant
        // between fork and exec, so the release is polled, briefly.
        let mut released = false;
        for _ in 0..50 {
            released = claim(&cfg, Service::Web).unwrap().is_some();
            if released {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(released, "released on drop");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_group_pid_is_never_signalled() {
        assert!(alive(std::process::id() as i32));
        // Both of these mean "a process group" to kill(2), and `stop` must never reach one.
        assert!(!alive(0) && !alive(-1));
    }
}
