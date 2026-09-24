//! T176: a command the agent already bounded — `sed -n 1,80p`, `head`/`tail -n`,
//! `grep -A/-B/-C/-m`, `cat -n` of named files — printed what was asked for. Cutting it
//! again only buys an `expand` round trip. The lexer reads quotes and the `|`, `|&`,
//! `&&`, `||`, `;` separators and nothing else of the shell grammar (`cmd/AGENTS.md`).

/// Past this the host truncates a Bash result on its own (Claude Code: 30 000 chars),
/// and a cut that names the archive beats one that does not.
pub const MAX_BYTES: usize = 30_000;

/// Whether every command in `snippet` ends in a bounding stage. A leading `cd`/`export`
/// prints nothing and does not unbound the rest; at least one stage must bound.
pub fn is_bounded(snippet: &str) -> bool {
    let mut any = false;
    for pipeline in lex(snippet) {
        let Some(last) = pipeline.last() else {
            continue;
        };
        let silent = pipeline.len() == 1
            && matches!(last.first().map(String::as_str), Some("cd" | "export"));
        if silent {
            continue;
        }
        if !bounds(last) {
            return false;
        }
        any = true;
    }
    any
}

/// Pipelines of stages of words, quotes removed.
fn lex(s: &str) -> Vec<Vec<Vec<String>>> {
    let mut lists = vec![vec![Vec::new()]];
    let mut word: Option<String> = None;
    let mut quote: Option<char> = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                word.get_or_insert_default().push(c);
            }
            continue;
        }
        let sep = match c {
            '\'' | '"' => {
                quote = Some(c);
                word.get_or_insert_default();
                continue;
            }
            '|' if chars.peek() == Some(&'|') => Some(true),
            '&' if chars.peek() == Some(&'&') => Some(true),
            ';' | '\n' => Some(true),
            '|' => Some(false),
            c if c.is_whitespace() => None,
            c => {
                word.get_or_insert_default().push(c);
                continue;
            }
        };
        let stages = lists.last_mut().expect("never empty");
        if let Some(w) = word.take() {
            stages.last_mut().expect("never empty").push(w);
        }
        if let Some(new_list) = sep {
            if matches!(chars.peek(), Some('|' | '&')) {
                chars.next();
            }
            if new_list {
                lists.push(vec![Vec::new()]);
            } else {
                stages.push(Vec::new());
            }
        }
    }
    if let Some(w) = word {
        lists
            .last_mut()
            .and_then(|s| s.last_mut())
            .expect("never empty")
            .push(w);
    }
    lists
        .into_iter()
        .filter(|p| p.iter().any(|s| !s.is_empty()))
        .collect()
}

fn bounds(stage: &[String]) -> bool {
    let Some((cmd, args)) = stage.split_first() else {
        return false;
    };
    match super::formatters::cmd_stem(cmd) {
        "head" => counts(args, |n| n.bytes().all(|b| b.is_ascii_digit())),
        "tail" => {
            !args
                .iter()
                .any(|a| matches!(a.as_str(), "-f" | "-F" | "--follow"))
                && counts(args, |n| !n.starts_with('+'))
        }
        "sed" => sed_range(args),
        "grep" | "rg" => args.iter().any(|a| {
            let short = a.strip_prefix('-').filter(|s| !s.starts_with('-'));
            short.is_some_and(|s| {
                s.trim_end_matches(|c: char| c.is_ascii_digit())
                    .ends_with(['A', 'B', 'C', 'm'])
            }) || [
                "--context",
                "--after-context",
                "--before-context",
                "--max-count",
            ]
            .iter()
            .any(|l| a.split('=').next() == Some(l))
        }),
        "cat" => args.iter().any(|a| a == "-n") && args.iter().any(|a| !a.starts_with('-')),
        _ => false,
    }
}

/// `head`/`tail` counts (`-n N`, `-nN`, `--lines=N`, `-c N`, `-N`) all pass `ok`.
/// No count is the default ten lines — bounded too.
fn counts(args: &[String], ok: impl Fn(&str) -> bool) -> bool {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let val = match a.as_str() {
            "-n" | "-c" | "--lines" | "--bytes" => it.next().map(String::as_str),
            _ => a
                .strip_prefix("--lines=")
                .or_else(|| a.strip_prefix("--bytes="))
                .or_else(|| a.strip_prefix("-n"))
                .or_else(|| a.strip_prefix("-c"))
                .or_else(|| {
                    a.strip_prefix('-')
                        .filter(|n| n.starts_with(|c: char| c.is_ascii_digit()))
                }),
        };
        if let Some(v) = val
            && (v.is_empty() || !ok(v))
        {
            return false;
        }
    }
    true
}

/// `sed -n` whose every script is numeric `a[,b]p` addresses, `;`-joined.
fn sed_range(args: &[String]) -> bool {
    let quiet = args
        .iter()
        .any(|a| matches!(a.as_str(), "-n" | "--quiet" | "--silent"));
    let mut scripts = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-e" | "--expression" => scripts.extend(it.next()),
            s if s.starts_with('-') => {}
            _ if scripts.is_empty() => scripts.push(a),
            _ => {}
        }
    }
    let range = |s: &str| {
        let Some(addr) = s.trim().strip_suffix('p') else {
            return false;
        };
        let mut ends = addr.split(',');
        let num = |e: Option<&str>| {
            e.is_some_and(|e| !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit()))
        };
        num(ends.next()) && ends.next().is_none_or(|e| num(Some(e))) && ends.next().is_none()
    };
    quiet && !scripts.is_empty() && scripts.iter().all(|s| s.split(';').all(range))
}

#[cfg(test)]
mod tests {
    use super::is_bounded;
    use rstest::rstest;

    #[rstest]
    #[case("sed -n '1,620p' src/hooks/types.rs")]
    #[case("sed -n 10p a; sed -n -e 1,5p -e 9p b")]
    #[case("cargo nextest run --workspace 2>&1 | tail -300")]
    #[case("cargo nextest run |& tail -n 50")]
    #[case("git log --oneline | head")]
    #[case("head -n 40 a.rs b.rs")]
    #[case("grep -rn -A3 'a|b' src")]
    #[case("rg -C 2 needle")]
    #[case("grep -m 5 x f")]
    #[case("cd /repo && sed -n '1,80p' Cargo.toml")]
    #[case("cat -n src/main.rs")]
    fn bounded_forms(#[case] cmd: &str) {
        assert!(is_bounded(cmd), "{cmd}");
    }

    #[rstest]
    #[case("cargo nextest run")]
    #[case("sed -n '/fn main/,$p' a.rs")]
    #[case("sed 's/a/b/' a.rs")]
    #[case("tail -n +5 log")]
    #[case("tail -f log")]
    #[case("head -n -5 log")]
    #[case("grep -rn needle src")]
    #[case("sed -n 1,5p a | grep x")]
    #[case("make && tail -5 build.log")]
    #[case("cat a.rs")]
    #[case("cd /repo")]
    #[case("echo 'x | head'")]
    fn unbounded_forms(#[case] cmd: &str) {
        assert!(!is_bounded(cmd), "{cmd}");
    }
}
