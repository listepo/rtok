//! Pure line filter for `rtok run` output (plan T3.2). No I/O.

/// One filter applied to captured command output.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Command argv matcher; unused by [`apply`] (T3.3 binds it).
    pub match_cmd: String,
    pub max_lines: u32,
    pub head: u32,
    pub tail: u32,
    pub drop: Vec<String>,
    pub keep: Vec<String>,
    pub dedupe: bool,
}

const BUILTIN_KEEP: &[&str] = &["error", "warning", "panic", "fail", "traceback"];

/// `[plugins.cmd]` knobs shared by `pick` and `apply`.
#[derive(Clone, Debug)]
pub struct Settings {
    fail_tail_lines: usize,
    rules: Vec<Rule>,
}

impl Settings {
    pub fn from_config(cfg: &crate::config::Config) -> Self {
        Self::load(&cfg.plugins.cmd.rules, cfg.plugins.cmd.fail_tail_lines)
    }

    /// Built-in defaults only (golden tests and fail-open paths without config).
    pub fn builtin() -> Self {
        Self {
            fail_tail_lines: 80,
            rules: defaults(),
        }
    }

    fn load(rules_path: &std::path::Path, fail_tail_lines: u32) -> Self {
        let user = if rules_path.is_file() {
            std::fs::read_to_string(rules_path)
                .map(|s| parse(&s))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            fail_tail_lines: fail_tail_lines.max(1) as usize,
            rules: merge_rules(defaults(), user),
        }
    }

    pub fn pick(&self, bin: &str) -> Rule {
        self.rules
            .iter()
            .find(|r| r.match_cmd == bin)
            .cloned()
            .unwrap_or_default()
    }
}

fn merge_rules(mut base: Vec<Rule>, user: Vec<Rule>) -> Vec<Rule> {
    for ur in user {
        if let Some(i) = base.iter().position(|r| r.match_cmd == ur.match_cmd) {
            base[i] = ur;
        } else {
            base.push(ur);
        }
    }
    base
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            match_cmd: String::new(),
            max_lines: 40,
            head: 10,
            tail: 10,
            drop: Vec::new(),
            keep: Vec::new(),
            dedupe: true,
        }
    }
}

fn matches_pat(pats: &[String], line: &str) -> bool {
    let low = line.to_ascii_lowercase();
    pats.iter().any(|p| {
        p.split('|')
            .any(|bit| !bit.is_empty() && low.contains(&bit.to_ascii_lowercase()))
    })
}

fn is_keep(line: &str, rule: &Rule) -> bool {
    let low = line.to_ascii_lowercase();
    BUILTIN_KEEP.iter().any(|k| low.contains(k)) || matches_pat(&rule.keep, line)
}

fn is_drop(line: &str, rule: &Rule) -> bool {
    matches_pat(&rule.drop, line)
}

fn dedupe(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev: Option<String> = None;
    let mut n = 0u32;
    let flush = |out: &mut Vec<String>, prev: &mut Option<String>, n: &mut u32| {
        if let Some(p) = prev.take() {
            if *n > 1 {
                out.push(format!("{p} (×{n})"));
            } else {
                out.push(p);
            }
        }
        *n = 0;
    };
    for line in lines {
        if prev.as_deref() == Some(line.as_str()) {
            n += 1;
            continue;
        }
        flush(&mut out, &mut prev, &mut n);
        prev = Some(line);
        n = 1;
    }
    flush(&mut out, &mut prev, &mut n);
    out
}

/// Apply `rule` to `output`. `exit != 0` → last `settings.fail_tail_lines` lines, untouched.
pub fn apply(
    settings: &Settings,
    output: &str,
    exit: i32,
    rule: &Rule,
    archive_id: &str,
) -> String {
    let mut lines: Vec<String> = output.lines().map(str::to_string).collect();
    if exit != 0 {
        let n = lines.len().saturating_sub(settings.fail_tail_lines);
        return lines[n..].join("\n");
    }
    lines.retain(|l| is_keep(l, rule) || !is_drop(l, rule));
    if rule.dedupe {
        lines = dedupe(lines);
    }
    let max = rule.max_lines.max(1) as usize;
    if lines.len() <= max {
        return lines.join("\n");
    }
    let head = rule.head.min(rule.max_lines) as usize;
    let tail = rule.tail.min(rule.max_lines.saturating_sub(rule.head)) as usize;
    let keep_idx: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_keep(l, rule))
        .map(|(i, _)| i)
        .collect();
    let mut take = vec![false; lines.len()];
    for slot in take.iter_mut().take(head.min(lines.len())) {
        *slot = true;
    }
    let tail_at = lines.len().saturating_sub(tail);
    for slot in take.iter_mut().skip(tail_at) {
        *slot = true;
    }
    for i in keep_idx {
        take[i] = true;
    }
    let total = lines.len();
    let mut picked: Vec<String> = Vec::new();
    let mut omitted = 0usize;
    for (i, line) in lines.into_iter().enumerate() {
        if take[i] {
            if picked.len() < max {
                if omitted > 0 {
                    let extra = if picked.len() + 1 >= max {
                        total - i
                    } else {
                        0
                    };
                    let count = omitted + extra;
                    picked.push(format!("… {count} lines omitted (expand {archive_id})"));
                    omitted = 0;
                    if picked.len() >= max {
                        break;
                    }
                }
                picked.push(line);
            } else {
                omitted += 1;
            }
        } else {
            omitted += 1;
        }
    }
    if omitted > 0 {
        if picked.len() >= max {
            picked.pop();
            omitted += 1;
        }
        picked.push(format!("… {omitted} lines omitted (expand {archive_id})"));
    }
    picked.truncate(max);
    picked.join("\n")
}

/// Built-in [`rules/default.toml`](../../../rules/default.toml).
pub fn defaults() -> Vec<Rule> {
    parse(include_str!("../../../rules/default.toml"))
}

fn parse(s: &str) -> Vec<Rule> {
    let doc = s.parse::<toml_edit::DocumentMut>().unwrap_or_default();
    let mut out = Vec::new();
    for (k, v) in doc.iter() {
        let Some(t) = v.as_table() else { continue };
        let num = |key: &str, d: u32| {
            t.get(key)
                .and_then(|i| i.as_integer())
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(d)
        };
        let strs = |key: &str| {
            t.get(key)
                .and_then(|i| i.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        out.push(Rule {
            match_cmd: k.to_string(),
            max_lines: num("max_lines", 40),
            head: num("head", 10),
            tail: num("tail", 10),
            drop: strs("drop"),
            keep: strs("keep"),
            dedupe: t.get("dedupe").and_then(|i| i.as_bool()).unwrap_or(true),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn settings(fail_tail_lines: u32) -> Settings {
        Settings {
            fail_tail_lines: fail_tail_lines.max(1) as usize,
            rules: defaults(),
        }
    }

    fn exit_nonzero_tail(
        fail_tail_lines: u32,
        expect_count: usize,
        expect_start: &str,
        expect_end: &str,
    ) {
        let body = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let s = settings(fail_tail_lines);
        let out = apply(&s, &body, 3, &Rule::default(), "id");
        assert_eq!(out.lines().count(), expect_count);
        assert!(out.starts_with(expect_start), "{out}");
        assert!(out.ends_with(expect_end), "{out}");
        assert!(!out.contains("omitted"));
    }

    #[test]
    fn three_hundred_ok_keeps_error_under_20() {
        let mut body = (0..300).map(|i| format!("ok {i}")).collect::<Vec<_>>();
        body.push("error: boom".into());
        let rule = Rule {
            max_lines: 20,
            head: 5,
            tail: 5,
            ..Rule::default()
        };
        let s = settings(80);
        let out = apply(&s, &body.join("\n"), 0, &rule, "abc");
        let n = out.lines().count();
        assert!(n <= 20, "{n} lines:\n{out}");
        assert!(out.contains("error: boom"), "{out}");
    }

    use rstest::rstest;

    #[rstest]
    #[case(80, 80, "line 20\n", "line 99")]
    #[case(3, 3, "line 97\n", "line 99")]
    fn exit_nonzero_fail_tail_lines(
        #[case] fail_tail_lines: u32,
        #[case] expect_count: usize,
        #[case] expect_start: &str,
        #[case] expect_end: &str,
    ) {
        exit_nonzero_tail(fail_tail_lines, expect_count, expect_start, expect_end);
    }

    #[rstest]
    #[case(10, 3, 3, 4, 7)]
    fn max_cap_reports_true_omitted_count(
        #[case] n_lines: usize,
        #[case] head: u32,
        #[case] tail: u32,
        #[case] max_lines: u32,
        #[case] expect_omitted: usize,
    ) {
        let body = (0..n_lines)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rule = Rule {
            max_lines,
            head,
            tail,
            dedupe: false,
            ..Rule::default()
        };
        let s = settings(80);
        let out = apply(&s, &body, 0, &rule, "arc");
        assert!(
            out.contains(&format!("{expect_omitted} lines omitted")),
            "{out}"
        );
        let content = out.lines().filter(|l| !l.contains("lines omitted")).count();
        assert_eq!(n_lines - content, expect_omitted, "{out}");
    }

    #[test]
    fn user_rules_file_changes_output_for_match_cmd() {
        let dir = std::env::temp_dir().join(format!("rtok-rules-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rules.toml");
        fs::write(
            &path,
            "[echo]\nmax_lines = 2\nhead = 1\ntail = 1\ndedupe = false\n",
        )
        .unwrap();
        let s = Settings::load(&path, 80);
        let body = (0..10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rule = s.pick("echo");
        let out = apply(&s, &body, 0, &rule, "id");
        assert_eq!(out.lines().count(), 2, "{out}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_rule_overrides_builtin_match_cmd() {
        let dir = std::env::temp_dir().join(format!("rtok-rules-ovr-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rules.toml");
        fs::write(
            &path,
            "[grep]\nmax_lines = 5\nhead = 2\ntail = 2\ndedupe = false\n",
        )
        .unwrap();
        let s = Settings::load(&path, 80);
        assert_eq!(s.pick("grep").max_lines, 5);
        let _ = fs::remove_dir_all(&dir);
    }
}
