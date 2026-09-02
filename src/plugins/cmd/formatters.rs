//! Per-family stdout compactors (plan T3.3). `None` → fall back to rules.

use super::rules::{self, Rule};

/// Compact `output`. Kind is `formatter`, `rule`, or `raw`.
pub fn compress(
    argv: &[String],
    output: &str,
    exit: i32,
    archive_id: &str,
) -> (String, &'static str) {
    if let Some(s) = format(argv, output) {
        return (s, "formatter");
    }
    let rule = pick(argv);
    let s = rules::apply(output, exit, &rule, archive_id);
    let kind = if s.len() < output.len() {
        "rule"
    } else {
        "raw"
    };
    (s, kind)
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

fn pick(argv: &[String]) -> Rule {
    let rules = rules::defaults();
    let joined = argv.join(" ");
    rules
        .into_iter()
        .find(|r| {
            !r.match_cmd.is_empty()
                && (joined
                    .split_whitespace()
                    .any(|w| w == r.match_cmd || w.ends_with(&format!("/{}", r.match_cmd))))
        })
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
}
