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
const FAIL_TAIL: usize = 80;

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

/// Apply `rule` to `output`. `exit != 0` → last 80 lines, untouched.
pub fn apply(output: &str, exit: i32, rule: &Rule, archive_id: &str) -> String {
    let mut lines: Vec<String> = output.lines().map(str::to_string).collect();
    if exit != 0 {
        let n = lines.len().saturating_sub(FAIL_TAIL);
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
    let mut picked: Vec<String> = Vec::new();
    let mut omitted = 0usize;
    for (i, line) in lines.into_iter().enumerate() {
        if take[i] && picked.len() < max {
            if omitted > 0 {
                picked.push(format!("… {omitted} lines omitted (expand {archive_id})"));
                omitted = 0;
                if picked.len() >= max {
                    break;
                }
            }
            picked.push(line);
        } else if !take[i] {
            omitted += 1;
        }
    }
    if omitted > 0 && picked.len() < max {
        picked.push(format!("… {omitted} lines omitted (expand {archive_id})"));
    }
    picked.truncate(max);
    picked.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let out = apply(&body.join("\n"), 0, &rule, "abc");
        let n = out.lines().count();
        assert!(n <= 20, "{n} lines:\n{out}");
        assert!(out.contains("error: boom"), "{out}");
    }

    #[test]
    fn exit_3_returns_last_80_untouched() {
        let body = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = apply(&body, 3, &Rule::default(), "id");
        assert_eq!(out.lines().count(), 80);
        assert!(out.starts_with("line 20\n"), "{out}");
        assert!(out.ends_with("line 99"), "{out}");
        assert!(!out.contains("omitted"));
    }
}
