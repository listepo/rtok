//! `rtok run -- <cmd>`: capture, archive, print (plan T3.1). Unfiltered in this task.

use crate::config::Config;
use crate::plugin::{Ctx, Measurement};
use crate::tokens::Class;
use anyhow::{Result, bail};
use std::process::Command;

use super::formatters;

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn script(args: &[String]) -> String {
    let inner = args
        .iter()
        .map(|a| sh_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{inner} 2>&1")
}

fn shell(cfg: &Config) -> String {
    if cfg.plugins.cmd.shell.is_empty() {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    } else {
        cfg.plugins.cmd.shell.clone()
    }
}

/// Run `args` via `$SHELL -lc`, archive stdout+stderr, print, return the exit code.
pub fn run(cfg: &Config, args: &[String]) -> Result<i32> {
    if args.is_empty() {
        bail!("rtok run: missing command");
    }
    let out = Command::new(shell(cfg))
        .arg("-lc")
        .arg(script(args))
        .output()?;
    let mut body = out.stdout;
    body.extend_from_slice(&out.stderr);
    let code = out.status.code().unwrap_or(1);
    let cx = Ctx::open(cfg.clone(), "run")?;
    let id = cx
        .store
        .put_archive(&cx.session, &body, &cfg.core.archive_dir)?;
    let before = String::from_utf8_lossy(&body);
    let family = args
        .first()
        .map(|a| a.rsplit('/').next().unwrap_or(a).to_string())
        .unwrap_or_else(|| "other".into());
    let (filtered, kind) = formatters::compress(args, &before, code, &id);
    print!("{filtered}");
    if !filtered.is_empty() && !filtered.ends_with('\n') {
        println!();
    }
    let lines = if body.is_empty() {
        0
    } else {
        before.lines().count() as u32
    };
    if lines > cfg.plugins.cmd.trailer_min_lines {
        println!("[rtok {id} · {lines} lines · expand: rtok expand {id}]");
    }
    let _ = cx.record(&Measurement {
        plugin: "cmd",
        kind,
        before_bytes: before.len() as u64,
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
    use std::fs;

    fn cfg(name: &str) -> (Config, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-run-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        c.core.log_file = dir.join("rtok.log");
        (c, dir)
    }

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
        let v: serde_json::Value =
            serde_json::from_str(&crate::measure::stats::plugin_json(&c, "cmd").unwrap()).unwrap();
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
}
