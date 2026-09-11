//! `rtok run -- <cmd>`: capture, archive, print (plan T3.1). Unfiltered in this task.

use crate::config::Config;
use anyhow::{Result, bail};
use rtok_plugin_sdk::{Archive, Class, Measurement};
use std::process::Command;

use super::{formatters, rules};

pub(crate) fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

/// Join argv into a `-lc` body. A single argument is already a shell snippet
/// (the PreToolUse wrap quotes the original command as one argv); several
/// arguments are a CLI argv list and must be quoted so spaces stay inside
/// one word. `{ … } 2>&1` so a trailing `&&`/`|` still merges stderr.
pub(crate) fn script(args: &[String]) -> String {
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

fn shell(cfg: &Config) -> String {
    if cfg.plugins.cmd.shell.is_empty() {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    } else {
        cfg.plugins.cmd.shell.clone()
    }
}

/// Whether the printed output needs the `expand` pointer. A line count above
/// `trailer_min_lines` is the old rule; anything the filter shortened needs it too, however
/// short the command was.
fn needs_pointer(lines: u32, trailer_min_lines: u32, printed: usize, raw: usize) -> bool {
    lines > trailer_min_lines || printed < raw
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
        assert_eq!(script(&["true && false".into()]), "{ true && false\n} 2>&1");
        assert_eq!(
            script(&["git".into(), "status".into()]),
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
    #[test]
    fn archive_keeps_bytes_that_are_not_utf8() {
        let (c, dir) = cfg("bytes");
        let code = run(&c, &["printf '\\xff\\xfeok\\n'".into()]).unwrap();
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
}
