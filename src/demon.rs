//! `rtok demon` (plan T20.1, decision D22) — keeps rtok's long-running surfaces up.
//!
//! One supervisor process per service. `start` detaches `rtok demon supervise <name>`; that
//! process re-spawns `rtok <name>` every time the child exits, and stops only when `stop` drops
//! a `<name>.stop` marker beside the state file. Nothing here runs on the hook path: `rtok hook`
//! never reads this state and fails open whether a supervisor is up or not (D1).

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::ValueEnum;
use rustix::process::{Pid, Signal};
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
}

impl Service {
    /// The subcommand the supervisor spawns, and the stem of its files.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::Mcp => "mcp",
            Self::Web => "web",
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
    /// The binary this supervisor runs; `update` compares it with the one on disk now.
    exe: PathBuf,
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

fn write(cfg: &Config, st: &State) -> Result<()> {
    let path = file(cfg, st.service, "json");
    fs::write(&path, serde_json::to_vec_pretty(st)?).with_context(|| path.display().to_string())
}

/// One process, never a group. `kill(2)` reads 0 as "my whole process group" and a negative
/// number as "that group", so a state file with either — corrupt, hand-edited, half-written —
/// must address nothing rather than signal every process rtok happens to share a group with.
fn one(pid: i32) -> Option<Pid> {
    (pid > 0).then(|| Pid::from_raw(pid)).flatten()
}

/// True while `pid` is a live process. Signal 0 asks the kernel, so a state file left behind by
/// a supervisor that was killed from outside reads as stopped instead of as whatever it said.
fn alive(pid: i32) -> bool {
    one(pid).is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
}

fn signal(pid: i32, sig: Signal) {
    if let Some(p) = one(pid) {
        let _ = rustix::process::kill_process(p, sig);
    }
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
    let exe = std::env::current_exe()?;
    for service in targets(cfg, named, false)? {
        // The supervisor's own lock is the truth; the state file can lag it or name a reused pid.
        if claim(cfg, service)?.is_none() {
            println!("{service} already running");
            continue;
        }
        if let Some(st) = read(cfg, service) {
            if alive(st.supervisor) {
                println!("{service} already running (supervisor {})", st.supervisor);
                continue;
            }
            // A supervisor that died from outside can leave its child up. Starting another
            // supervisor would put two owners on the same port — retire the orphan first.
            if alive(st.child) {
                signal(st.child, Signal::TERM);
                for _ in 0..40 {
                    if !alive(st.child) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if alive(st.child) {
                    signal(st.child, Signal::KILL);
                }
            }
            let _ = fs::remove_file(file(cfg, service, "json"));
        }
        let _ = fs::remove_file(file(cfg, service, "stop"));
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
    let sig = if force { Signal::KILL } else { Signal::TERM };
    for service in targets(cfg, named, true)? {
        let Some(st) = read(cfg, service) else {
            println!("{service} not running");
            continue;
        };
        fs::write(file(cfg, service, "stop"), b"")?;
        signal(st.supervisor, sig);
        signal(st.child, sig);
        for _ in 0..40 {
            if !alive(st.supervisor) && !alive(st.child) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // Last resort: a supervisor that ignored SIGTERM would otherwise outlive its own state.
        if alive(st.supervisor) || alive(st.child) {
            signal(st.supervisor, Signal::KILL);
            signal(st.child, Signal::KILL);
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

/// One row of the `rtok demon status` page (T15.11): state asked of the kernel, never
/// copied out of the state file.
#[derive(Debug, Serialize)]
pub struct Row {
    pub service: Service,
    pub running: bool,
    /// `None` when the service is down: the columns render `-`.
    pub supervisor: Option<i32>,
    pub child: Option<i32>,
    pub uptime_secs: Option<u64>,
    pub restarts: Option<u32>,
    pub log: PathBuf,
}

/// The query behind `rtok demon status` / `list`: one row per service the verb targets,
/// liveness from `kill(2)` so a state file left behind by a killed supervisor reads as
/// stopped instead of as whatever it said.
pub fn rows(cfg: &Config, named: &[Service]) -> Result<Vec<Row>> {
    let mut out = Vec::new();
    for service in targets(cfg, named, true)? {
        let st = read(cfg, service);
        let up = st.as_ref().is_some_and(|s| alive(s.supervisor));
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
        out.push(Row {
            service,
            running: up,
            supervisor,
            child,
            uptime_secs,
            restarts,
            log: file(cfg, service, "log"),
        });
    }
    Ok(out)
}

/// The rendering: header plus one coloured row per service. The word is padded before it
/// is coloured: ANSI bytes would otherwise count as width.
pub fn table(rows: &[Row]) -> String {
    let mut out = format!(
        "{:<11}{:<10}{:<12}{:<8}{:<9}{:<10}log\n",
        "service", "state", "supervisor", "child", "uptime", "restarts"
    );
    let dash = || "-".to_string();
    for r in rows {
        let word = format!("{:<10}", if r.running { "running" } else { "stopped" });
        out.push_str(&format!(
            "{:<11}{}{:<12}{:<8}{:<9}{:<10}{}\n",
            r.service,
            crate::render::state(&word, r.running),
            r.supervisor.map(|v| v.to_string()).unwrap_or_else(dash),
            r.child.map(|v| v.to_string()).unwrap_or_else(dash),
            r.uptime_secs.map(|s| format!("{s}s")).unwrap_or_else(dash),
            r.restarts.map(|v| v.to_string()).unwrap_or_else(dash),
            r.log.display()
        ));
    }
    out
}

/// Restart under the binary that is on disk now — what to run after `ketch install listepo/rtok`
/// replaced it, since a running supervisor keeps holding the old inode.
pub fn update(cfg: &Config, config_file: Option<&Path>, named: &[Service]) -> Result<()> {
    let exe = std::env::current_exe()?;
    let services = targets(cfg, named, true)?;
    for service in &services {
        match read(cfg, *service) {
            Some(st) if st.exe == exe => println!("{service} already runs {}", exe.display()),
            Some(st) => println!("{service} {} -> {}", st.exe.display(), exe.display()),
            None => println!("{service} not running"),
        }
    }
    restart(cfg, config_file, &services)
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
    let _ = rustix::process::setsid();
    let exe = std::env::current_exe()?;
    let stop = file(cfg, service, "stop");
    let mut st = State {
        service,
        exe: exe.clone(),
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
            .arg(service.as_str())
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
    match rustix::fs::flock(&f, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Some(f)),
        Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
        Err(e) => Err(e.into()),
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
        assert!(
            claim(&cfg, Service::Web).unwrap().is_some(),
            "released on drop"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_group_pid_is_never_signalled() {
        assert!(alive(std::process::id() as i32));
        // Both of these mean "a process group" to kill(2), and `stop` must never reach one.
        assert!(one(0).is_none());
        assert!(one(-1).is_none());
        assert!(!alive(0) && !alive(-1));
    }
}
