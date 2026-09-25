//! Per-family stdout compactors (plan T3.3). `None` → fall back to rules.

use super::rules;

/// Compact `output`. Kind is `formatter`, `rule`, `raw` (below the size gate, or a T176
/// bounded passthrough — both by design), or `unmatched` (a rule/formatter ran and
/// shrank nothing — T177's actionable share).
pub fn compress(
    settings: &rules::Settings,
    argv: &[String],
    output: &str,
    exit: i32,
    archive_id: &str,
) -> (String, &'static str) {
    // T176: `sed -n 1,620p`, `| tail -300`, `grep -A3` already printed what was asked
    // for; a second cut here only sent the agent to `expand` for the same bytes.
    if output.len() <= super::bounded::MAX_BYTES && super::bounded::is_bounded(&argv.join(" ")) {
        return (output.to_string(), "raw");
    }
    // T177: a single Bash string chaining 2+ DISTINCT programs (`&&`, `|`, `;`, or a newline)
    // is not one family's output; matching only argv[0] picks the wrong rule, or none at
    // all. A same-program chain (`cargo build && cargo test`, `cd x && cargo test`) keeps
    // its own family's formatter/rule — only a genuine mix routes to `[script]`.
    let multi = matches!(argv, [one] if super::bounded::mixed_chain(one));
    let split = family_argv(argv);
    let stem = if multi { "script" } else { bin(&split) };
    let rule = settings.pick(stem);
    // T65.2: JSON bodies skip table formatters so kubectl -o json / gh --json
    // reach the compact pass instead of a NAME/STATUS parser.
    if rules::is_json_body(output) {
        let s = rules::apply(settings, output, exit, &rule, archive_id);
        let kind = if s.len() < output.len() {
            "rule"
        } else {
            "unmatched"
        };
        return (s, kind);
    }
    if !multi && let Some(s) = format(&split, output) {
        return (s, "formatter");
    }
    let s = rules::apply(settings, output, exit, &rule, archive_id);
    let kind = if stem == "skill" {
        "skill"
    } else if s.len() < output.len() {
        "rule"
    } else {
        "unmatched"
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
    "cargo", "git", "pytest", "jest", "vitest", "tree", "go", "docker", "kubectl", "ps",
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
        ("tree", _) => Some(output.lines().take(40).collect::<Vec<_>>().join("\n")),
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
    // The PID column is not always first: BSD `ps aux` prints `USER PID …`,
    // the macOS `ps aux` shape in the goldens prints `PID …` first.
    let pid_at = header
        .split_whitespace()
        .position(|h| h.eq_ignore_ascii_case("PID"))
        .unwrap_or(0);
    for line in lines {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // One bad line (a repeated header, a localized string, a wrapped row)
        // skips itself — returning None here would drop every good row with it.
        let pid_ok = fields
            .get(pid_at)
            .is_some_and(|pid| pid.bytes().all(|b| b.is_ascii_digit()));
        if !pid_ok {
            continue;
        }
        let pid = fields[pid_at];
        let cmd = fields
            .iter()
            .copied()
            .find(|t| t.starts_with('/') || t.starts_with("./"))
            .or_else(|| fields.last().copied())
            .unwrap_or("");
        let base = cmd.rsplit('/').next().unwrap_or(cmd);
        let tail = fields.last().copied().unwrap_or("");
        if tail != base && tail != pid {
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

    /// T176 audit repro: `cargo nextest run … | tail -300` came back as 4.7 KB of 20 KB and
    /// lost the failing test's panic. Bounded, the failure block survives whole; the same
    /// log unbounded is still cut.
    #[test]
    fn a_bounded_failing_nextest_log_keeps_its_failure_block() {
        let settings = rules::Settings::builtin();
        let mut log: Vec<String> = (0..150)
            .map(|i| format!("        PASS [   0.01s] t{i}"))
            .collect();
        log.splice(
            70..70,
            [
                "        FAIL [   0.02s] rtok store::tests::lock",
                "--- STDERR:              rtok store::tests::lock ---",
                "thread 'store::tests::lock' panicked at src/store/mod.rs:88:5:",
                "assertion `left == right` failed",
            ]
            .map(String::from),
        );
        let log = log.join("\n");
        let bounded = ["cargo nextest run --workspace 2>&1 | tail -300".to_string()];
        assert_eq!(
            compress(&settings, &bounded, &log, 0, "id"),
            (log.clone(), "raw")
        );
        let (cut, _) = compress(&settings, &["cargo nextest run".into()], &log, 0, "id");
        assert!(cut.len() < log.len(), "unbounded output is still filtered");
    }

    fn goldens() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cmd_golden")
    }

    /// `min_saving: <percent>` (T238): the floor a family's saving must clear, checked in
    /// `ten_families_and_aws_key_unredacted`. `None` when the header line is absent — the
    /// golden test then fails so every `.in` must declare one.
    fn parse_in(s: &str) -> (Vec<String>, i32, Option<u32>, String) {
        let mut argv = Vec::new();
        let mut exit = 0;
        let mut min_saving = None;
        let mut rest = s;
        for line in s.lines() {
            if let Some(a) = line.strip_prefix("argv: ") {
                argv = a.split_whitespace().map(str::to_string).collect();
            } else if let Some(e) = line.strip_prefix("exit: ") {
                exit = e.parse().unwrap_or(0);
            } else if let Some(m) = line.strip_prefix("min_saving: ") {
                min_saving = m.trim().parse().ok();
            } else if line == "---" {
                rest = s.split_once("---\n").map(|(_, r)| r).unwrap_or("");
                break;
            }
        }
        (argv, exit, min_saving, rest.to_string())
    }

    /// Fixed rates (T238), matching `tests/mode_bench.rs` and `tokens::tests::RATES` — not
    /// `Estimator::default()`, so a config default change can't silently shift the goldens'
    /// floors.
    /// AWS's documented example access key id, planted in the `cat.in` golden. It must reach the
    /// output untouched: the formatters compress, they do not redact.
    const AWS_EXAMPLE_KEY_ID: &str = "AKIAIOSFODNN7EXAMPLE";

    const SAVING_RATES: crate::config::Estimator = crate::config::Estimator {
        code: 3.5,
        prose: 4.2,
        json: 3.0,
        cjk: 1.0,
    };

    /// Saving % of shrinking `before` to `after`, both estimated as `Class::Code` — the same
    /// class `cmd/run.rs` and `cmd/filter.rs` record every `Measurement` with, so the floor
    /// tracks what a real run actually reports instead of a duplicated estimate path.
    fn saving_pct(before: &str, after: &str) -> (u32, u32, i64) {
        let b = crate::tokens::estimate(before, rtok_plugin_sdk::Class::Code, &SAVING_RATES);
        let a = crate::tokens::estimate(after, rtok_plugin_sdk::Class::Code, &SAVING_RATES);
        let pct = if b == 0 {
            0
        } else {
            (b as i64 - a as i64) * 100 / b as i64
        };
        (b, a, pct)
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
            let (argv, exit, min_saving, output) = parse_in(&raw);
            let (got, _) = compress(&settings, &argv, &output, exit, "deadbeef");
            let outp = p.with_extension("out");
            let want = fs::read_to_string(&outp).unwrap();
            assert_eq!(got.trim_end(), want.trim_end(), "{}", p.display());
            // T238: guard the size of the saving, not only the bytes — a re-blessed `.out`
            // that keeps twice as much must fail even though it still matches itself.
            let floor = min_saving
                .unwrap_or_else(|| panic!("{}: missing `min_saving:` header", p.display()));
            let (before, after, pct) = saving_pct(&output, &got);
            assert!(
                after <= before,
                "{}: output ({after} tok) is larger than input ({before} tok)",
                p.display()
            );
            assert!(
                pct >= floor as i64,
                "{}: saving {pct}% below floor {floor}% (before {before} tok, after {after} tok)",
                p.display()
            );
        }
        assert!(n >= 10, "need 10 families, got {n}");
        let secret = fs::read_to_string(dir.join("cat.in")).unwrap();
        assert!(secret.contains(AWS_EXAMPLE_KEY_ID));
        let (got, _) = compress(
            &settings,
            &["cat".into(), "secrets.env".into()],
            &parse_in(&secret).3,
            0,
            "id",
        );
        // Never echo `got` here: it carries the credential-shaped fixture, and a failing assert
        // would print it to the test log.
        assert!(
            got.contains(AWS_EXAMPLE_KEY_ID),
            "compressed `cat secrets.env` output dropped or altered the AWS key line"
        );
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

    /// T177: a single Bash string joining 2+ commands is filtered by `[script]`, not by
    /// the first command's family — `git`'s formatter would otherwise run on `cargo`'s
    /// mixed-in output too.
    #[test]
    fn newline_joined_script_uses_the_script_rule_not_the_first_command() {
        let settings = rules::Settings::builtin();
        let body = (0..40)
            .map(|i| format!("line {i} of mixed output"))
            .collect::<Vec<_>>()
            .join("\n");
        let script = format!("git status\ncat file.txt\n{body}");
        let (got, kind) = compress(&settings, std::slice::from_ref(&script), &script, 0, "id");
        assert_eq!(kind, "rule", "{got}");
        assert!(got.len() < script.len(), "{got}");
        assert!(got.contains("omitted (expand id)"), "{got}");
        // A single command with no `&&`/`;`/newline chain keeps using its own family.
        let (_, single_kind) = compress(&settings, &["git status".into()], "clean\n", 0, "id");
        assert_ne!(
            single_kind, "rule",
            "one-line git status should not hit [script]"
        );
    }

    /// T177 (revised): a chain of the SAME program (`cargo build && cargo test`, or with a
    /// leading `cd`) must keep using `cargo`'s own formatter/rule, not the generic
    /// `[script]` — only a genuinely mixed chain loses the specialised path.
    #[test]
    fn same_program_chain_keeps_its_own_family_not_script() {
        let settings = rules::Settings::builtin();
        let diag = (0..40)
            .map(|i| format!("warning: unused variable `x{i}`"))
            .collect::<Vec<_>>()
            .join("\n");
        for cmd in [
            "cargo build && cargo test".to_string(),
            "cd crates/rtok && cargo test".to_string(),
        ] {
            let (got, kind) = compress(&settings, std::slice::from_ref(&cmd), &diag, 0, "id");
            assert_ne!(kind, "rule", "{cmd}: {got}");
        }
    }

    /// T177: `unmatched` (not `raw`) once a family rule ran and shrank nothing — `raw`
    /// stays reserved for a tiny body below the trailer gate or a T176 bounded passthrough.
    #[test]
    fn a_picked_rule_that_shrinks_nothing_is_unmatched_not_raw() {
        let settings = rules::Settings::builtin();
        // `cat` has a named rule (`max_lines = 80`); 5 lines never reach the cut.
        let body = "one\ntwo\nthree\nfour\nfive";
        let (got, kind) = compress(&settings, &["cat".into(), "f".into()], body, 0, "id");
        assert_eq!(got, body);
        assert_eq!(kind, "unmatched");
    }

    /// T177: `cat -n` of several named files is not one bounded read (T176) — it must
    /// still hit `[cat]`'s head/tail cut once it is large.
    #[test]
    fn unbounded_multi_file_cat_gets_the_cat_rule() {
        let settings = rules::Settings::builtin();
        let body = (0..200)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let cmd = "cat -n a.rs b.rs c.rs".to_string();
        let (got, kind) = compress(&settings, &[cmd], &body, 0, "id");
        assert_eq!(kind, "rule", "{got}");
        assert!(got.len() < body.len(), "{got}");
    }

    /// T177: a recursive grep/rg with a context flag but no `-m`/`--max-count` can return
    /// an unbounded hit list — it must still hit `[grep]`'s cut once it is large.
    #[test]
    fn large_recursive_grep_hit_list_gets_the_grep_rule() {
        let settings = rules::Settings::builtin();
        let body = (0..200)
            .map(|i| format!("src/f{i}.rs:{i}:needle found here"))
            .collect::<Vec<_>>()
            .join("\n");
        let cmd = "grep -rn -A3 needle src".to_string();
        let (got, kind) = compress(&settings, &[cmd], &body, 0, "id");
        assert_eq!(kind, "rule", "{got}");
        assert!(got.len() < body.len(), "{got}");
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
            let (argv, exit, _, output) = parse_in(&raw);
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
            let (_, exit, _, output) = parse_in(&raw);
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

    #[test]
    fn group_goldens_beat_the_same_rule_without_group() {
        use super::rules::Group;
        let settings = rules::Settings::builtin();
        let dir = goldens();
        for file in [
            "ls.in",
            "find.in",
            "rg.in",
            "tsc_dup.in",
            "eslint_dup.in",
            "cargo_check.in",
            "dotnet_dup.in",
        ] {
            let raw = fs::read_to_string(dir.join(file)).unwrap();
            let (argv, exit, _, output) = parse_in(&raw);
            let (got, _) = compress(&settings, &argv, &output, exit, "deadbeef");
            let mut off = settings.pick(bin(&family_argv(&argv)));
            off.group = Group::Off;
            let ungrouped = rules::apply(&settings, &output, exit, &off, "deadbeef");
            assert!(
                got.len() < ungrouped.len(),
                "{file}: grouped {} B vs ungrouped {} B\n{got}",
                got.len(),
                ungrouped.len()
            );
        }
    }

    #[test]
    fn ps_aux_skips_a_bad_line_instead_of_dropping_every_row() {
        let output = "USER PID %CPU COMMAND\n\
            root 1 0.0 /sbin/init\n\
            USER PID %CPU COMMAND\n\
            root 42 0.1 /usr/bin/worker-7\n";
        let got = ps_aux(output).expect("good rows survive a repeated header");
        assert!(got.contains("1 init"), "{got}");
        assert!(got.contains("42 worker-7"), "{got}");
        assert!(!got.contains("PID"), "{got}");
    }

    /// T240: every family a builtin rule names in `rules/default.toml` must have a golden
    /// `.in` whose argv actually picks that rule via `family_argv`/`bin` — the same path
    /// `compress` uses in production. `[script]` is reached by mixed-chain detection, not
    /// by an argv[0] match, so it is exempt (covered by the chain tests above instead).
    #[test]
    fn every_builtin_rule_family_has_a_golden() {
        let in_files: Vec<String> = fs::read_dir(goldens())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("in"))
            .map(|p| fs::read_to_string(&p).unwrap())
            .collect();
        let covered = |family: &str| {
            in_files.iter().any(|raw| {
                let (argv, ..) = parse_in(raw);
                !argv.is_empty() && bin(&family_argv(&argv)) == family
            })
        };
        let missing: Vec<String> = rules::defaults()
            .into_iter()
            .map(|r| r.match_cmd)
            .filter(|family| family != "script" && !covered(family))
            .collect();
        assert!(missing.is_empty(), "families without a golden: {missing:?}");
    }
}
