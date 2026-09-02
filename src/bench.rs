//! `rtok bench` A/B harness (plan T9.1).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

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
                let usage = one(&t.prompt, settings);
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

/// `(input, cache, output, cost)` from `claude -p` JSON, or zeros if it cannot run.
fn one(prompt: &str, settings: &Path) -> (u64, u64, u64, f64) {
    if !settings.exists() || std::env::var("RTOK_BENCH_LIVE").is_err() {
        return (0, 0, 0, 0.0);
    }
    let out = Command::new("claude")
        .args([
            "-p",
            prompt,
            "--output-format",
            "json",
            "--settings",
            &settings.display().to_string(),
        ])
        .output()
        .ok();
    let Some(out) = out.filter(|o| o.status.success()) else {
        return (0, 0, 0, 0.0);
    };
    parse_usage(&out.stdout)
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
}
