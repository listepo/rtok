//! `rtok bench` A/B harness (plan T9.1, T68.9).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use figment::Figment;
use figment::providers::{Format, Toml};
use serde::Deserialize;
use serde_json::Value;

use crate::config::Config;

#[derive(Deserialize)]
struct TaskFile {
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
struct Task {
    id: String,
    prompt: String,
    check: String,
}

#[derive(Default)]
struct Acc {
    input: u64,
    cache: u64,
    output: u64,
    cost: f64,
    pass: u32,
    n: u32,
}

pub fn run(cfg: &Config) -> Result<String> {
    if cfg.bench.suite == "graph" {
        return run_graph(cfg);
    }
    if !cfg.bench.suite.is_empty() {
        bail!("unknown bench suite {:?}", cfg.bench.suite);
    }
    let tasks = load_tasks(&cfg.bench.tasks)?;
    if tasks.len() != 6 {
        bail!("expected 6 tasks, got {}", tasks.len());
    }
    if cfg.bench.dry_run {
        return Ok(dry_list(&tasks, cfg));
    }
    Ok(table(&tasks, cfg))
}

fn load_tasks(path: &Path) -> Result<Vec<Task>> {
    Figment::new()
        .merge(Toml::file(path))
        .extract::<TaskFile>()
        .with_context(|| path.display().to_string())
        .map(|f| f.tasks)
}

fn dry_list(tasks: &[Task], cfg: &Config) -> String {
    let mut out = String::new();
    for t in tasks {
        for name in cfg.bench.configs.keys() {
            for n in 1..=cfg.bench.runs {
                out.push_str(&format!("{} {name} {n}\n", t.id));
            }
        }
    }
    out
}

fn table(tasks: &[Task], cfg: &Config) -> String {
    let mut by: BTreeMap<String, Acc> = BTreeMap::new();
    for t in tasks {
        for (name, settings) in &cfg.bench.configs {
            let acc = by.entry(name.clone()).or_default();
            for _ in 0..cfg.bench.runs {
                let usage = one(&t.prompt, settings, cfg.bench.timeout_s);
                acc.input += usage.0;
                acc.cache += usage.1;
                acc.output += usage.2;
                acc.cost += usage.3;
                acc.n += 1;
            }
            if Command::new("sh")
                .arg("-c")
                .arg(&t.check)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                acc.pass += 1;
            }
        }
    }
    write_results(&by, tasks.len(), cfg);
    let mut s = format!(
        "{:<8} {:>10} {:>10} {:>10} {:>10} {:>8}\n",
        "config", "input", "cache", "output", "cost", "pass"
    );
    for (name, a) in &by {
        let n = a.n.max(1) as f64;
        s.push_str(&format!(
            "{name:<8} {:>10.0} {:>10.0} {:>10.0} {:>10.4} {:>3}/{:<3}\n",
            a.input as f64 / n,
            a.cache as f64 / n,
            a.output as f64 / n,
            a.cost / n,
            a.pass,
            tasks.len()
        ));
    }
    s
}

const GRAPH_ARMS: [&str; 2] = ["mcp", "native"];

#[derive(Deserialize)]
struct GraphFile {
    repos: Vec<GraphRepo>,
    questions: Vec<GraphQ>,
}
#[derive(Deserialize)]
struct GraphRepo {
    id: String,
    path: String,
}
#[derive(Deserialize)]
struct GraphQ {
    id: String,
    repo: String,
    prompt: String,
    expect: String,
}

fn graph_toml(cfg: &Config) -> PathBuf {
    let t = &cfg.bench.tasks;
    if t.file_name().is_some_and(|n| n == "graph.toml") {
        t.clone()
    } else {
        t.parent().unwrap_or(Path::new("bench")).join("graph.toml")
    }
}

fn load_graph(path: &Path) -> Result<GraphFile> {
    let f = Figment::new()
        .merge(Toml::file(path))
        .extract::<GraphFile>()
        .with_context(|| path.display().to_string())?;
    if f.questions.len() < 10 {
        bail!(
            "graph suite needs ≥ 10 questions, got {}",
            f.questions.len()
        );
    }
    if f.repos.len() < 3 {
        bail!("graph suite needs ≥ 3 repos, got {}", f.repos.len());
    }
    let ids: Vec<&str> = f.repos.iter().map(|r| r.id.as_str()).collect();
    for q in &f.questions {
        if !ids.contains(&q.repo.as_str()) {
            bail!("question {} repo {} is not in [[repos]]", q.id, q.repo);
        }
        regex::Regex::new(&q.expect).with_context(|| format!("{} expect", q.id))?;
    }
    Ok(f)
}

fn run_graph(cfg: &Config) -> Result<String> {
    let file = load_graph(&graph_toml(cfg))?;
    if cfg.bench.dry_run {
        let mut out = String::new();
        for q in &file.questions {
            for arm in GRAPH_ARMS {
                for n in 1..=cfg.bench.runs {
                    out.push_str(&format!("{} {} {arm} {n}\n", q.id, q.repo));
                }
            }
        }
        return Ok(out);
    }
    Ok(graph_table(&file, cfg))
}

fn graph_table(file: &GraphFile, cfg: &Config) -> String {
    let mut s = String::from("id repo arm tools in out cache wall_ms cost resident pass\n");
    let runs = cfg.bench.runs;
    let mut tot = [(0u64, 0u64, 0u64, 0u64, 0u64, 0.0, 0u64, 0u32); 2];
    for q in &file.questions {
        let repo = file.repos.iter().find(|r| r.id == q.repo).unwrap();
        let re = regex::Regex::new(&q.expect).expect("checked at load");
        for (i, arm) in GRAPH_ARMS.iter().enumerate() {
            let mut a = (0u64, 0u64, 0u64, 0u64, 0u64, 0.0, 0u64, 0u32);
            for _ in 0..runs {
                let u = graph_one(&q.prompt, Path::new(&repo.path), arm, cfg);
                if re.is_match(&u.7) {
                    a.7 += 1;
                }
                a.0 += u.0;
                a.1 += u.1;
                a.2 += u.2;
                a.3 += u.3;
                a.4 += u.4;
                a.5 += u.5;
                a.6 += u.6;
            }
            let n = runs.max(1) as f64;
            s.push_str(&format!(
                "{} {} {arm} {:.0} {:.0} {:.0} {:.0} {:.0} {:.4} {:.0} {}/{runs}\n",
                q.id,
                q.repo,
                a.0 as f64 / n,
                a.1 as f64 / n,
                a.2 as f64 / n,
                a.3 as f64 / n,
                a.4 as f64 / n,
                a.5 / n,
                a.6 as f64 / n,
                a.7
            ));
            tot[i].0 += a.0;
            tot[i].1 += a.1;
            tot[i].2 += a.2;
            tot[i].3 += a.3;
            tot[i].4 += a.4;
            tot[i].5 += a.5;
            tot[i].6 += a.6;
            tot[i].7 += a.7;
        }
    }
    let n = (file.questions.len() as u32 * runs).max(1) as f64;
    for (i, arm) in GRAPH_ARMS.iter().enumerate() {
        let a = tot[i];
        s.push_str(&format!(
            "TOTAL * {arm} {:.0} {:.0} {:.0} {:.0} {:.0} {:.4} {:.0} {}/{n:.0}\n",
            a.0 as f64 / n,
            a.1 as f64 / n,
            a.2 as f64 / n,
            a.3 as f64 / n,
            a.4 as f64 / n,
            a.5 / n,
            a.6 as f64 / n,
            a.7
        ));
    }
    s
}

fn graph_one(
    prompt: &str,
    repo: &Path,
    arm: &str,
    cfg: &Config,
) -> (u64, u64, u64, u64, u64, f64, u64, String) {
    let _ = (prompt, repo, arm, cfg);
    (0, 0, 0, 0, 0, 0.0, 0, String::new())
}

fn write_results(by: &BTreeMap<String, Acc>, tasks: usize, cfg: &Config) {
    if cfg!(test) {
        return;
    }
    let dir = cfg
        .bench
        .tasks
        .parent()
        .unwrap_or(Path::new("bench"))
        .join("results");
    let _ = std::fs::create_dir_all(&dir);
    let live = std::env::var("RTOK_BENCH_LIVE").is_ok();
    for (name, a) in by {
        let n = a.n.max(1) as f64;
        let v = serde_json::json!({
            "config": name,
            "runs": a.n,
            "mean_input": a.input as f64 / n,
            "mean_cache": a.cache as f64 / n,
            "mean_output": a.output as f64 / n,
            "mean_cost_usd": a.cost / n,
            "pass": a.pass,
            "tasks": tasks,
            "live": live,
        });
        let path = dir.join(format!("{name}.json"));
        let _ = std::fs::write(
            &path,
            serde_json::to_string_pretty(&v).unwrap_or_default() + "\n",
        );
    }
}

/// `(input, cache, output, cost)` from `claude -p` JSON, or zeros if it cannot run or runs
/// past `timeout_s` — a host that hangs is one zero row, not a bench that never returns.
fn one(prompt: &str, settings: &Path, timeout_s: u64) -> (u64, u64, u64, f64) {
    if !settings.exists() || std::env::var("RTOK_BENCH_LIVE").is_err() {
        return (0, 0, 0, 0.0);
    }
    let mut cmd = Command::new("claude");
    cmd.args([
        "-p",
        prompt,
        "--output-format",
        "json",
        "--settings",
        &settings.display().to_string(),
    ]);
    let Some(out) = run_bounded(&mut cmd, Duration::from_secs(timeout_s.max(1))) else {
        return (0, 0, 0, 0.0);
    };
    parse_usage(&out)
}

/// How often [`run_bounded`] checks whether the child has exited.
const POLL: Duration = Duration::from_millis(20);

/// Run `cmd` and collect its stdout, killing it once `timeout` has passed. `None` when it
/// could not be spawned, exited badly, or was killed.
fn run_bounded(cmd: &mut Command, timeout: Duration) -> Option<Vec<u8>> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    // Drain while polling: a child that fills the pipe (64 KiB) before exiting would
    // otherwise block on write, never exit, and be scored as a timeout.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut buf);
        buf
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => break None,
        }
    };
    let out = reader.join().ok()?;
    status?.success().then_some(out)
}

fn parse_usage(bytes: &[u8]) -> (u64, u64, u64, f64) {
    let v: Value = serde_json::from_slice(bytes).unwrap_or(Value::Null);
    let u = v.get("usage").cloned().unwrap_or(Value::Null);
    (
        num(&u, "input_tokens"),
        num(&u, "cache_read_input_tokens") + num(&u, "cache_creation_input_tokens"),
        num(&u, "output_tokens"),
        v.get("total_cost_usd")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    )
}

fn num(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg(dry: bool) -> Config {
        let mut c = Config::default();
        c.bench.tasks = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bench/tasks.toml");
        c.bench.dry_run = dry;
        c
    }

    #[test]
    fn dry_run_lists_six_by_two_by_three() {
        let s = run(&cfg(true)).unwrap();
        let lines: Vec<_> = s.lines().collect();
        assert_eq!(lines.len(), 6 * 2 * 3, "{s}");
        assert!(
            s.contains("add-fn a 1") && s.contains("run-tests b 3"),
            "{s}"
        );
    }

    #[test]
    fn real_run_prints_a_table() {
        let s = run(&cfg(false)).unwrap();
        assert!(s.contains("config"), "{s}");
        assert!(s.contains("input") && s.contains("pass"), "{s}");
        assert!(s.lines().any(|l| l.starts_with('a')), "{s}");
        assert!(s.lines().any(|l| l.starts_with('b')), "{s}");
    }

    /// `[bench] timeout_s` was read by nothing, so a hung host held `rtok bench` forever.
    #[test]
    fn a_hung_run_is_killed_instead_of_hanging_the_bench() {
        // Spawn `sleep` directly: `sh -c 'sleep 30'` leaves the sleep child holding
        // the pipe after `kill` on the shell, so the reader thread waited the full 30s.
        let mut slow = Command::new("sleep");
        slow.arg("30");
        let start = Instant::now();
        assert!(run_bounded(&mut slow, Duration::from_millis(150)).is_none());
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "killed, not waited out"
        );
    }

    #[test]
    fn a_finished_run_hands_back_its_output() {
        let mut quick = Command::new("sh");
        quick.arg("-c").arg("printf hello");
        let out = run_bounded(&mut quick, Duration::from_secs(30)).expect("runs");
        assert_eq!(out, b"hello");
    }

    /// More than a pipe buffer of output used to deadlock the child and score as a timeout.
    #[test]
    fn a_run_past_the_pipe_buffer_is_drained_not_timed_out() {
        let mut big = Command::new("sh");
        big.arg("-c").arg("head -c 300000 /dev/zero");
        let out = run_bounded(&mut big, Duration::from_secs(10)).expect("runs");
        assert_eq!(out.len(), 300_000);
    }
}
