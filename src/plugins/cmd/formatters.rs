//! Per-family stdout compactors (plan T3.3). `None` → fall back to rules.

use super::rules::{self, Rule};

/// Compact `output`. Kind is `formatter`, `rule`, or `raw`.
pub fn compress(
    argv: &[String],
    output: &str,
    exit: i32,
    archive_id: &str,
) -> (String, &'static str) {
    let argv = family_argv(argv);
    if let Some(s) = format(&argv, output) {
        return (s, "formatter");
    }
    let rule = pick(&argv);
    let s = rules::apply(output, exit, &rule, archive_id);
    let kind = if s.len() < output.len() {
        "rule"
    } else {
        "raw"
    };
    (s, kind)
}

/// The argv a family is matched against. The hook quotes the whole command into one argv
/// (`rtok run -- 'git status'`, `cmd/hook.rs`), while a hand-typed `rtok run -- git status`
/// arrives split; both must read as `["git", "status"]`, or no formatter and no `[rule]`
/// matches the path that actually runs. Only the first word of a snippet is considered —
/// `cmd/AGENTS.md` forbids parsing shell syntax beyond it.
pub fn family_argv(argv: &[String]) -> Vec<String> {
    match argv {
        [one] => one.split_whitespace().map(str::to_string).collect(),
        many => many.to_vec(),
    }
}

/// `argv[0]`'s basename — the family a `Measurement` names. `other` when there is none.
pub fn family(argv: &[String]) -> String {
    let argv = family_argv(argv);
    match bin(&argv) {
        "" => "other".to_string(),
        found => found.to_string(),
    }
}

fn bin(argv: &[String]) -> &str {
    argv.first()
        .map(|a| a.rsplit('/').next().unwrap_or(a.as_str()))
        .unwrap_or("")
}

fn sub(argv: &[String]) -> &str {
    argv.get(1).map(String::as_str).unwrap_or("")
}

fn format(argv: &[String], output: &str) -> Option<String> {
    match (bin(argv), sub(argv)) {
        ("cargo", "test") => Some(keep(
            output,
            &["FAILED", "test result:", "error[", "panicked"],
        )),
        ("cargo", "build") | ("cargo", "clippy") => Some(keep(
            output,
            &["error[", "error:", "-->", "Finished", "warning:"],
        )),
        ("git", "status") => Some(git_status(output)),
        ("git", "diff") => Some(keep(
            output,
            &["diff --git", "@@", "file changed", "+++", "--- a/"],
        )),
        ("git", "log") => Some(output.lines().take(20).collect::<Vec<_>>().join("\n")),
        ("pytest", _) => Some(keep(
            output,
            &["FAILED", "ERROR", "passed", "failed", "error"],
        )),
        ("jest", _) | ("vitest", _) => Some(keep(output, &["FAIL", "PASS", "Tests:", "● "])),
        ("go", "test") => Some(keep(output, &["FAIL", "PASS", "ok  ", "--- FAIL"])),
        ("ls", _) => Some(output.lines().take(40).collect::<Vec<_>>().join("\n")),
        ("find", _) | ("tree", _) => Some(output.lines().take(40).collect::<Vec<_>>().join("\n")),
        _ => None,
    }
}

fn keep(output: &str, needles: &[&str]) -> String {
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| needles.iter().any(|n| l.contains(n)))
        .collect();
    if lines.is_empty() {
        return output
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
    }
    lines.join("\n")
}

fn git_status(output: &str) -> String {
    let mut out = Vec::new();
    for line in output.lines() {
        let t = line.trim_start();
        if t.starts_with("On branch ")
            || t.starts_with("modified:")
            || t.starts_with("new file:")
            || t.starts_with("deleted:")
            || t.starts_with("renamed:")
            || t.starts_with("Untracked")
            || t.starts_with("?? ")
            || t.starts_with(" M ")
            || t.starts_with("M  ")
        {
            out.push(t);
        }
    }
    if out.is_empty() {
        output.lines().take(15).collect::<Vec<_>>().join("\n")
    } else {
        out.join("\n")
    }
}

/// The `rules/default.toml` rule for the command itself (`argv[0]`'s basename, as [`format`]
/// keys on) — not for any word of its arguments: `git commit -m "fix grep"` is not a `grep`.
fn pick(argv: &[String]) -> Rule {
    let bin = bin(argv);
    rules::defaults()
        .into_iter()
        .find(|r| r.match_cmd == bin)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn goldens() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cmd_golden")
    }

    fn parse_in(s: &str) -> (Vec<String>, i32, String) {
        let mut argv = Vec::new();
        let mut exit = 0;
        let mut rest = s;
        for line in s.lines() {
            if let Some(a) = line.strip_prefix("argv: ") {
                argv = a.split_whitespace().map(str::to_string).collect();
            } else if let Some(e) = line.strip_prefix("exit: ") {
                exit = e.parse().unwrap_or(0);
            } else if line == "---" {
                rest = s.split_once("---\n").map(|(_, r)| r).unwrap_or("");
                break;
            }
        }
        (argv, exit, rest.to_string())
    }

    #[test]
    fn ten_families_and_aws_key_unredacted() {
        let dir = goldens();
        let mut n = 0u32;
        let mut files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        for p in files {
            if p.extension().and_then(|e| e.to_str()) != Some("in") {
                continue;
            }
            n += 1;
            let raw = fs::read_to_string(&p).unwrap();
            let (argv, exit, output) = parse_in(&raw);
            let (got, _) = compress(&argv, &output, exit, "deadbeef");
            let outp = p.with_extension("out");
            let want = fs::read_to_string(&outp).unwrap();
            assert_eq!(got.trim_end(), want.trim_end(), "{}", p.display());
        }
        assert!(n >= 10, "need 10 families, got {n}");
        let secret = fs::read_to_string(dir.join("cat.in")).unwrap();
        assert!(secret.contains("AKIAIOSFODNN7EXAMPLE"));
        let (got, _) = compress(
            &["cat".into(), "secrets.env".into()],
            &parse_in(&secret).2,
            0,
            "id",
        );
        assert!(got.contains("AKIAIOSFODNN7EXAMPLE"), "{got}");
    }

    #[test]
    fn rule_is_picked_by_the_command_not_an_argument() {
        let argv = |s: &[&str]| s.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(
            pick(&family_argv(&argv(&["/usr/bin/grep", "-rn", "x"]))).match_cmd,
            "grep"
        );
        assert_eq!(
            pick(&family_argv(&argv(&["git", "commit", "-m", "fix grep"]))).match_cmd,
            ""
        );
        assert_eq!(
            pick(&family_argv(&argv(&["docker", "run", "node"]))).match_cmd,
            ""
        );
    }

    /// The hook wraps Bash as `rtok run -- '<cmd>'`, so the command reaches `compress` as
    /// one argv. Every golden must filter identically in that shape — this is the shape that
    /// runs in production, and the split-argv one only in this test file.
    #[test]
    fn one_quoted_argv_filters_like_a_split_argv() {
        let mut checked = 0;
        for p in fs::read_dir(goldens())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("in"))
        {
            let raw = fs::read_to_string(&p).unwrap();
            let (argv, exit, output) = parse_in(&raw);
            if argv.is_empty() {
                continue;
            }
            checked += 1;
            let joined = vec![argv.join(" ")];
            assert_eq!(
                compress(&joined, &output, exit, "id"),
                compress(&argv, &output, exit, "id"),
                "{}: one quoted argv filtered differently",
                p.display()
            );
        }
        assert!(checked >= 10, "expected the golden families, saw {checked}");
    }

    #[test]
    fn family_names_the_command_not_the_whole_snippet() {
        let argv = |s: &[&str]| s.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(family(&argv(&["git status | head"])), "git");
        assert_eq!(family(&argv(&["/usr/bin/git", "status"])), "git");
        assert_eq!(family(&argv(&[])), "other");
    }
}
