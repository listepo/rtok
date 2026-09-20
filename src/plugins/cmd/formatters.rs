//! Per-family stdout compactors (plan T3.3). `None` → fall back to rules.

use super::rules;

/// Compact `output`. Kind is `formatter`, `rule`, or `raw`.
pub fn compress(
    settings: &rules::Settings,
    argv: &[String],
    output: &str,
    exit: i32,
    archive_id: &str,
) -> (String, &'static str) {
    let argv = family_argv(argv);
    if let Some(s) = format(&argv, output) {
        return (s, "formatter");
    }
    let rule = settings.pick(bin(&argv));
    let s = rules::apply(settings, output, exit, &rule, archive_id);
    let kind = if bin(&argv) == "skill" {
        "skill"
    } else if s.len() < output.len() {
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

/// Basename of argv[0], splitting on `/` and `\` and dropping a trailing `.exe`
/// (case-insensitive). Re-export of [`crate::agents::cmd_stem`] — one definition,
/// shared with `measure::stats::bash_family` and `agents::is_rtok_bin` (T55.10).
pub(crate) use crate::agents::cmd_stem;

/// Stems with a Rust formatter (any subcommand). `rtok stats` labels the whole stem.
const FORMATTER_STEMS: &[&str] = &[
    "cargo", "git", "pytest", "jest", "vitest", "ls", "find", "tree", "go", "docker", "kubectl",
    "ps",
];

/// T50.1: how `rtok stats` labels a Bash family — `formatter`, named `rule`, or `default`.
pub fn filter_kind(settings: &rules::Settings, stem: &str) -> &'static str {
    if FORMATTER_STEMS.contains(&stem) {
        return "formatter";
    }
    if settings.pick(stem).match_cmd == stem {
        return "rule";
    }
    "default"
}

fn bin(argv: &[String]) -> &str {
    argv.first().map(|a| cmd_stem(a)).unwrap_or("")
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
        ("git", "diff") => Some(git_diff(output)),
        ("git", "log") => Some(output.lines().take(20).collect::<Vec<_>>().join("\n")),
        ("pytest", _) => Some(keep(
            output,
            &["FAILED", "ERROR", "passed", "failed", "error"],
        )),
        ("jest", _) | ("vitest", _) => Some(keep(output, &["FAIL", "PASS", "Tests:", "● "])),
        ("go", "test") => Some(keep(output, &["FAIL", "PASS", "ok  ", "--- FAIL"])),
        ("ls", _) => Some(output.lines().take(40).collect::<Vec<_>>().join("\n")),
        ("find", _) | ("tree", _) => Some(output.lines().take(40).collect::<Vec<_>>().join("\n")),
        ("docker", "ps") => docker_ps(output),
        ("kubectl", "get") => kubectl_get(output),
        ("ps", "aux") => ps_aux(output),
        _ => None,
    }
}

/// `git diff`: the changed lines *are* the answer, so only the blob-hash bookkeeping goes.
/// A needle list here (`"+++"`, `"--- a/"`) matched the file headers and dropped every
/// `+`/`-` line, i.e. the whole change.
fn git_diff(output: &str) -> String {
    output
        .lines()
        .filter(|l| !l.starts_with("index "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `git status`: the branch line, what changed, and the untracked paths listed under their
/// header. Porcelain codes are read on the raw line — `trim_start` used to eat the first
/// column, so `" M x"` could never match — and an indented path belongs to the section
/// header above it. Action hints and section boilerplate are dropped; empty output falls
/// back to the first 15 lines rather than to nothing.
fn git_status(output: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut untracked = false;
    for line in output.lines() {
        let head = line.trim_start();
        if head.starts_with("Untracked files:") {
            untracked = true;
            out.push(head.to_string());
        } else if is_status_entry(head) {
            untracked = false;
            out.push(head.to_string());
        } else if is_porcelain(line) {
            untracked = false;
            out.push(line.to_string());
        } else if untracked && line.starts_with(['\t', ' ']) && !head.starts_with("(use ") {
            // An untracked path, indented under its header in the long format.
            out.push(head.to_string());
        }
    }
    if out.is_empty() {
        output.lines().take(15).collect::<Vec<_>>().join("\n")
    } else {
        out.join("\n")
    }
}

/// Long-format entries and the one-line summaries worth keeping.
fn is_status_entry(head: &str) -> bool {
    const VERBS: &[&str] = &[
        "modified:",
        "new file:",
        "deleted:",
        "renamed:",
        "copied:",
        "both modified:",
        "both added:",
        "both deleted:",
        "added by us:",
        "unmerged:",
    ];
    const LINES: &[&str] = &[
        "On branch ",
        "Your branch ",
        "HEAD detached ",
        "nothing to commit",
        "no changes added to commit",
        "nothing added to commit",
    ];
    VERBS.iter().any(|v| head.starts_with(v)) || LINES.iter().any(|l| head.starts_with(l))
}

/// `XY path` in the short/porcelain format: two status columns, then a space. Both columns
/// are meaningful (` M` modified in the worktree, `??` untracked), so the line is never
/// trimmed before this test.
fn is_porcelain(line: &str) -> bool {
    let b = line.as_bytes();
    let column = |c: u8| {
        matches!(
            c,
            b' ' | b'M' | b'A' | b'D' | b'R' | b'C' | b'U' | b'?' | b'!'
        )
    };
    b.len() > 3 && column(b[0]) && column(b[1]) && b[2] == b' '
}

/// `docker ps`: one compact row per container. Padding, bind-all, container-side
/// port/proto, registry host and `N days`/`N hours` units are alignment noise; the
/// name, status, host port and image tag are the objects the rule's head/tail cut drops.
fn docker_ps(output: &str) -> Option<String> {
    let mut lines = output.lines().filter(|l| !l.is_empty());
    let header = lines.next()?;
    let u = header.to_ascii_uppercase();
    if !u.contains("CONTAINER ID") && !(u.contains("IMAGE") && u.contains("NAMES")) {
        return None;
    }
    let rows: Vec<String> = lines
        .map(compact_docker_row)
        .filter(|r| !r.is_empty())
        .collect();
    if rows.is_empty() {
        None
    } else {
        Some(rows.join("\n"))
    }
}

fn compact_docker_row(line: &str) -> String {
    let joined = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let no_bind = joined.replace("0.0.0.0:", "");
    shorten_ago(&drop_registry_host(&drop_arrow_port(&no_bind)))
}

fn drop_arrow_port(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'>') {
            chars.next();
            while matches!(chars.peek(), Some(d) if d.is_ascii_digit()) {
                chars.next();
            }
            if chars.peek() == Some(&'/') {
                chars.next();
                while matches!(chars.peek(), Some(d) if d.is_ascii_alphabetic()) {
                    chars.next();
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn drop_registry_host(s: &str) -> String {
    s.split_whitespace()
        .map(|tok| match tok.split_once('/') {
            Some((host, rest)) if host.contains('.') && !host.contains(':') => rest,
            _ => tok,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn shorten_ago(s: &str) -> String {
    let mut out = s.to_string();
    for (from, to) in [
        (" days", "d"),
        (" day", "d"),
        (" hours", "h"),
        (" hour", "h"),
        (" minutes", "m"),
        (" minute", "m"),
        (" seconds", "s"),
        (" second", "s"),
        (" weeks", "w"),
        (" week", "w"),
    ] {
        out = out.replace(from, to);
    }
    out
}

/// `kubectl get`: one row per object, dropping wide columns (AGE, NODE, RESTARTS)
/// that the default rule keeps in the head/tail while omitting the middle objects.
fn kubectl_get(output: &str) -> Option<String> {
    let mut lines = output.lines().filter(|l| !l.is_empty());
    let header = lines.next()?;
    let cols: Vec<&str> = header.split_whitespace().collect();
    if cols.first().copied() != Some("NAME") {
        return None;
    }
    if !cols
        .iter()
        .any(|c| matches!(*c, "STATUS" | "READY" | "AGE"))
    {
        return None;
    }
    const KEEP: &[&str] = &["NAME", "READY", "STATUS", "IP"];
    let idx: Vec<usize> = cols
        .iter()
        .enumerate()
        .filter(|(_, c)| KEEP.contains(c))
        .map(|(i, _)| i)
        .collect();
    if idx.is_empty() {
        return None;
    }
    let mut out = vec![idx.iter().map(|&i| cols[i]).collect::<Vec<_>>().join(" ")];
    for line in lines {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 {
            continue;
        }
        let picked: Vec<&str> = idx.iter().filter_map(|&i| fields.get(i).copied()).collect();
        if !picked.is_empty() {
            out.push(picked.join(" "));
        }
    }
    if out.len() < 2 {
        None
    } else {
        Some(out.join("\n"))
    }
}

/// `ps aux`: one row per process. TTY/TIME padding is noise; PID plus the
/// command basename (and its last arg when that is a distinct worker id) is the object.
fn ps_aux(output: &str) -> Option<String> {
    let mut lines = output.lines().filter(|l| !l.is_empty());
    let header = lines.next()?;
    let u = header.to_ascii_uppercase();
    if !u.contains("PID") || !(u.contains("CMD") || u.contains("COMMAND")) {
        return None;
    }
    let mut rows = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let pid = fields.first()?;
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let cmd = fields
            .iter()
            .copied()
            .find(|t| t.starts_with('/') || t.starts_with("./"))
            .or_else(|| fields.last().copied())
            .unwrap_or("");
        let base = cmd.rsplit('/').next().unwrap_or(cmd);
        let tail = fields.last().copied().unwrap_or("");
        if tail != base && tail != *pid {
            rows.push(format!("{pid} {base} {tail}"));
        } else {
            rows.push(format!("{pid} {base}"));
        }
    }
    if rows.is_empty() {
        None
    } else {
        Some(rows.join("\n"))
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
        let settings = rules::Settings::builtin();
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
            let (got, _) = compress(&settings, &argv, &output, exit, "deadbeef");
            let outp = p.with_extension("out");
            let want = fs::read_to_string(&outp).unwrap();
            assert_eq!(got.trim_end(), want.trim_end(), "{}", p.display());
        }
        assert!(n >= 10, "need 10 families, got {n}");
        let secret = fs::read_to_string(dir.join("cat.in")).unwrap();
        assert!(secret.contains("AKIAIOSFODNN7EXAMPLE"));
        let (got, _) = compress(
            &settings,
            &["cat".into(), "secrets.env".into()],
            &parse_in(&secret).2,
            0,
            "id",
        );
        assert!(got.contains("AKIAIOSFODNN7EXAMPLE"), "{got}");
    }

    #[test]
    fn rule_is_picked_by_the_command_not_an_argument() {
        let settings = rules::Settings::builtin();
        let argv = |s: &[&str]| s.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(
            settings
                .pick(bin(&family_argv(&argv(&["/usr/bin/grep", "-rn", "x"]))))
                .match_cmd,
            "grep"
        );
        assert_eq!(
            settings
                .pick(bin(&family_argv(&argv(&[
                    "git", "commit", "-m", "fix grep"
                ]))))
                .match_cmd,
            ""
        );
        assert_eq!(
            settings
                .pick(bin(&family_argv(&argv(&["docker", "run", "node"]))))
                .match_cmd,
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
            let settings = rules::Settings::builtin();
            assert_eq!(
                compress(&settings, &joined, &output, exit, "id"),
                compress(&settings, &argv, &output, exit, "id"),
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

    #[test]
    fn family_names_windows_argv() {
        let argv = |s: &[&str]| s.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(family(&argv(&[r"C:\tools\git.exe", "status"])), "git");
        assert_eq!(family(&argv(&["cargo.exe", "test"])), "cargo");
    }

    #[test]
    fn table_formatter_beats_default_rule_and_keeps_every_object() {
        let settings = rules::Settings::builtin();
        let dir = goldens();
        for (file, argv0, prefix) in [
            ("docker_ps.in", ["docker", "ps"], "web-"),
            ("kubectl_get.in", ["kubectl", "get"], "web-deploy-"),
            ("ps_aux.in", ["ps", "aux"], "worker-"),
        ] {
            let raw = fs::read_to_string(dir.join(file)).unwrap();
            let (_, exit, output) = parse_in(&raw);
            let argv = argv0.iter().map(|w| w.to_string()).collect::<Vec<_>>();
            let (got, kind) = compress(&settings, &argv, &output, exit, "deadbeef");
            assert_eq!(kind, "formatter", "{file}");
            let rule_out = rules::apply(
                &settings,
                &output,
                exit,
                &rules::Rule::default(),
                "deadbeef",
            );
            assert!(
                got.len() < rule_out.len(),
                "{file}: formatter {} vs rule {}",
                got.len(),
                rule_out.len()
            );
            for i in 0..40 {
                let name = match prefix {
                    "web-" => format!("web-{i:02}"),
                    "web-deploy-" => format!("web-deploy-{i:02}-abcd"),
                    _ => format!("worker-{i}"),
                };
                assert!(got.contains(&name), "{file} dropped {name}");
            }
        }
    }

    #[test]
    fn table_formatter_stands_down_on_unrecognized() {
        let settings = rules::Settings::builtin();
        let argv = |s: &[&str]| s.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        let (got, kind) = compress(
            &settings,
            &argv(&["docker", "ps"]),
            "Cannot connect to the Docker daemon\n",
            0,
            "deadbeef",
        );
        assert_ne!(kind, "formatter", "{got}");
        assert!(got.contains("Cannot connect"), "{got}");
        let (got, kind) = compress(
            &settings,
            &argv(&["kubectl", "get"]),
            "error: the server doesn't have a resource type \"pods\"\n",
            1,
            "deadbeef",
        );
        assert_ne!(kind, "formatter", "{got}");
        assert!(got.contains("doesn't have a resource type"), "{got}");
        let (got, kind) = compress(
            &settings,
            &argv(&["ps", "aux"]),
            "ps: invalid option -- z\n",
            1,
            "deadbeef",
        );
        assert_ne!(kind, "formatter", "{got}");
        assert!(got.contains("invalid option"), "{got}");
    }
}
