//! Pure line filter for `rtok run` output (plan T3.2). No I/O.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Group {
    #[default]
    Off,
    Dir,
    Diag,
}

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
    pub dedupe: Dedupe,
    /// T65.3: fold runs of two or more spaces inside each line to one before the
    /// cut. `Rule::default()` — every stem without a TOML rule — turns it on: the
    /// columnar families (`docker ps`, `kubectl get`, `ps aux`) spend a third of
    /// their bytes on alignment. TOML rules keep it off unless they ask.
    pub collapse_columns: bool,
    pub group: Group,
}

const BUILTIN_KEEP: &[&str] = &["error", "warning", "panic", "fail", "traceback"];

/// How repeated lines fold before the head/tail cut (T64.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dedupe {
    Off,
    Adjacent,
    Normalized,
}

impl Default for Dedupe {
    fn default() -> Self {
        Self::Adjacent
    }
}

fn parse_dedupe(v: Option<&toml_edit::Item>, default: Dedupe) -> Result<Dedupe, String> {
    match v {
        None => Ok(default),
        Some(item) => {
            if let Some(b) = item.as_bool() {
                return Ok(if b { Dedupe::Adjacent } else { Dedupe::Off });
            }
            if let Some(s) = item.as_str() {
                return match s {
                    "normalized" => Ok(Dedupe::Normalized),
                    "true" => Ok(Dedupe::Adjacent),
                    "false" => Ok(Dedupe::Off),
                    other => Err(format!(
                        "dedupe: expected bool or \"normalized\", got \"{other}\""
                    )),
                };
            }
            Err("dedupe: expected bool or \"normalized\"".into())
        }
    }
}

/// `[plugins.cmd]` knobs shared by `pick` and `apply`.
#[derive(Clone, Debug)]
pub struct Settings {
    fail_tail_lines: usize,
    rules: Vec<Rule>,
}

impl Settings {
    pub fn from_config(cfg: &crate::config::Config) -> Self {
        Self::load(
            &cfg.plugins.cmd.rules,
            Some(&cfg.plugins.cmd.rules_dir),
            cfg.plugins.cmd.fail_tail_lines,
        )
    }

    /// Built-in defaults only (golden tests and fail-open paths without config).
    pub fn builtin() -> Self {
        Self {
            fail_tail_lines: 80,
            rules: defaults(),
        }
    }

    /// Merge builtins with in-memory rule files (D29 / T56). `rules_file` and optional
    /// `rules_dir` are Vfs paths; `*.toml` under the dir merge in name order like disk load.
    #[cfg(test)]
    pub(crate) fn from_vfs(
        vfs: &crate::testutil::Vfs,
        rules_file: &str,
        rules_dir: Option<&str>,
        fail_tail_lines: u32,
    ) -> Self {
        let mut rules = defaults();
        if let Some(s) = vfs.read_str(rules_file)
            && let Ok(user) = parse_strict(s)
        {
            rules = merge_rules(rules, user);
        }
        if let Some(dir) = rules_dir {
            for path in vfs.paths_under(dir) {
                if !path.ends_with(".toml") {
                    continue;
                }
                if let Some(s) = vfs.read_str(&path)
                    && let Ok(dropin) = parse_strict(s)
                {
                    rules = merge_rules(rules, dropin);
                }
            }
        }
        Self {
            fail_tail_lines: fail_tail_lines.max(1) as usize,
            rules,
        }
    }

    fn load(
        rules_path: &std::path::Path,
        rules_dir: Option<&std::path::Path>,
        fail_tail_lines: u32,
    ) -> Self {
        let mut rules = defaults();
        // One user file, then every `rules.d/*.toml` in name order; a later file
        // wins per `match_cmd` through the same `merge_rules`.
        if let Some(user) = read_rules_file(rules_path) {
            rules = merge_rules(rules, user);
        }
        if let Some(dir) = rules_dir
            && let Ok(rd) = std::fs::read_dir(dir)
        {
            let mut files: Vec<std::path::PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
                .collect();
            files.sort();
            for path in files {
                if let Some(dropin) = read_rules_file(&path) {
                    rules = merge_rules(rules, dropin);
                }
            }
        }
        Self {
            fail_tail_lines: fail_tail_lines.max(1) as usize,
            rules,
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

/// Read one rules file, strictly: a missing file is no rules, and a malformed
/// one is no rules too (fail open — `rtok config validate` is what reports it).
fn read_rules_file(path: &std::path::Path) -> Option<Vec<Rule>> {
    if !path.is_file() {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| parse_strict(&s).ok())
}

/// Parse one rules file strictly for `rtok config validate` (T50.2): TOML
/// syntax, table-only top level, known fields only, right types. The runtime
/// [`read_rules_file`] uses the same parser and skips the file on any error.
fn parse_group(t: &toml_edit::Table, k: &str) -> Result<Group, String> {
    match t.get("group") {
        None => Ok(Group::Off),
        Some(v) => match v.as_str() {
            Some("dir") => Ok(Group::Dir),
            Some("diag") => Ok(Group::Diag),
            _ => Err(format!("[{k}].group: expected \"dir\" or \"diag\"")),
        },
    }
}

pub fn parse_strict(s: &str) -> Result<Vec<Rule>, String> {
    let doc = s
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    let mut out = Vec::new();
    for (k, v) in doc.iter() {
        let Some(t) = v.as_table() else {
            return Err(format!("[{k}]: expected table"));
        };
        let num = |key: &str, dflt: u32| -> Result<u32, String> {
            match t.get(key) {
                None => Ok(dflt),
                Some(i) => i
                    .as_integer()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| format!("[{k}].{key}: expected integer ≥ 0")),
            }
        };
        let strs = |key: &str| -> Result<Vec<String>, String> {
            match t.get(key) {
                None => Ok(Vec::new()),
                Some(a) => a
                    .as_array()
                    .ok_or_else(|| format!("[{k}].{key}: expected array of strings"))?
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| format!("[{k}].{key}: expected array of strings"))
                    })
                    .collect(),
            }
        };
        for (field, _) in t.iter() {
            match field {
                "max_lines" | "head" | "tail" | "drop" | "keep" | "dedupe" | "collapse_columns"
                | "group" => {}
                _ => return Err(format!("[{k}].{field}: unknown field")),
            }
        }
        out.push(Rule {
            match_cmd: k.to_string(),
            max_lines: num("max_lines", 40)?,
            head: num("head", 10)?,
            tail: num("tail", 10)?,
            drop: strs("drop")?,
            keep: strs("keep")?,
            dedupe: parse_dedupe(t.get("dedupe"), Dedupe::Adjacent)
                .map_err(|e| format!("[{k}].{e}"))?,
            collapse_columns: match t.get("collapse_columns") {
                None => false,
                Some(v) => v
                    .as_bool()
                    .ok_or_else(|| format!("[{k}].collapse_columns: expected bool"))?,
            },
            group: parse_group(t, k)?,
        });
    }
    Ok(out)
}

/// Malformed rules files among the single `rules` file (when present) and every
/// `rules.d/*.toml`, as `path: reason` lines for `rtok config validate` (T50.2).
/// A missing file or dir is not an error — both default to built-ins.
pub fn issues_in(rules_path: &std::path::Path, rules_dir: &std::path::Path) -> Vec<String> {
    let mut errs = Vec::new();
    let mut files = Vec::new();
    if rules_path.is_file() {
        files.push(rules_path.to_path_buf());
    }
    if let Ok(rd) = std::fs::read_dir(rules_dir) {
        let mut dropins: Vec<std::path::PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
            .collect();
        dropins.sort();
        files.extend(dropins);
    }
    for path in files {
        match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => errs.push(format!("{}: cannot read ({e})", path.display())),
            Ok(s) => {
                if let Err(e) = parse_strict(&s) {
                    errs.push(format!("{}: {e}", path.display()));
                }
            }
        }
    }
    errs
}

/// T56.3: same validation as [`issues_in`], reading rule bodies from a [`crate::testutil::Vfs`].
#[cfg(test)]
pub(crate) fn issues_in_from_vfs(
    vfs: &crate::testutil::Vfs,
    rules_file: &str,
    rules_dir: &str,
) -> Vec<String> {
    let mut errs = Vec::new();
    let mut files = Vec::new();
    if vfs.exists(rules_file) {
        files.push(rules_file.to_string());
    }
    let mut dropins: Vec<String> = vfs
        .paths_under(rules_dir)
        .into_iter()
        .filter(|p| p.ends_with(".toml") && p != rules_dir)
        .collect();
    dropins.sort();
    files.extend(dropins);
    for path in files {
        match vfs.read_str(&path) {
            Some(s) => {
                if let Err(e) = parse_strict(s) {
                    errs.push(format!("{path}: {e}"));
                }
            }
            None => {
                // Missing / non-UTF-8: disk twin reports "cannot read"; Vfs has no IO error kind.
                if vfs.exists(&path) {
                    errs.push(format!("{path}: cannot read (not utf-8)"));
                }
            }
        }
    }
    errs
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
            dedupe: Dedupe::Adjacent,
            collapse_columns: true,
            group: Group::Off,
        }
    }
}

/// `low` is the line already lowercased: the caller does it once per line, not once per check.
fn matches_pat(pats: &[String], low: &str) -> bool {
    pats.iter().any(|p| {
        p.split('|')
            .any(|bit| !bit.is_empty() && low.contains(&bit.to_ascii_lowercase()))
    })
}

fn is_keep(low: &str, rule: &Rule) -> bool {
    BUILTIN_KEEP.iter().any(|k| low.contains(k)) || matches_pat(&rule.keep, low)
}

fn is_drop(low: &str, rule: &Rule) -> bool {
    matches_pat(&rule.drop, low)
}

/// T65.3: fold runs of two or more spaces inside a line to one; leading
/// indentation is kept (tracebacks, YAML and diffs are not columnar) and a
/// trailing run folds to nothing — no signal, pure bytes.
fn collapse_columns(line: &str) -> String {
    let trimmed = line.trim_start();
    let leading = &line[..line.len() - trimmed.len()];
    let mut out = String::with_capacity(line.len());
    out.push_str(leading);
    let mut spaces = 0usize;
    for c in trimmed.chars() {
        if c == ' ' {
            spaces += 1;
            continue;
        }
        if spaces > 0 {
            out.push(' ');
            spaces = 0;
        }
        out.push(c);
    }
    out
}

/// T65.4: which lines belong to a stack-trace block that a head/tail cut must never
/// drop — the frames under a kept `error`/`panic` line are the part the model needs.
/// One detector per language: Python's `Traceback (most recent call last):` header
/// plus its indented frames; Rust's `thread '…' panicked at` plus the indented
/// panic message and `stack backtrace:` frames; JS's `Error:` line followed by
/// `    at ` frames; Go's `goroutine N [running]:` plus its `()` / tab-indented
/// frames; Java's `Exception in thread` plus its `\tat` frames. Whole blocks are
/// kept regardless of `head`/`tail`; only the block's own length counts against
/// `max_lines`.
fn trace_blocks(lines: &[String]) -> Vec<bool> {
    let mut keep = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        let low = lines[i].to_ascii_lowercase();
        let indented_from = |keep: &mut Vec<bool>, from: usize| -> usize {
            let mut j = from;
            while j < lines.len() && (lines[j].starts_with(' ') || lines[j].starts_with('\t')) {
                keep[j] = true;
                j += 1;
            }
            j
        };
        if low.starts_with("traceback (most recent call last):") {
            keep[i] = true;
            i = indented_from(&mut keep, i + 1);
        } else if low.contains("panicked at") {
            keep[i] = true;
            let mut j = i + 1;
            // The panic message sits directly under the header; from
            // `stack backtrace:` on, the indented frames are kept until the first
            // non-indented line (the `note:` trailer ends the block unkept).
            if j < lines.len() && !lines[j].starts_with(' ') && !lines[j].starts_with('\t') {
                let l = lines[j].to_ascii_lowercase();
                if !l.starts_with("stack backtrace:")
                    && !l.starts_with("note:")
                    && !l.starts_with("error")
                    && !l.starts_with("warning")
                {
                    keep[j] = true;
                    j += 1;
                }
            }
            while j < lines.len() {
                let ll = lines[j].to_ascii_lowercase();
                if ll.starts_with("stack backtrace:") {
                    keep[j] = true;
                    j += 1;
                    j = indented_from(&mut keep, j);
                    break;
                }
                if ll.starts_with("note:") || ll.starts_with("error") || ll.starts_with("warning") {
                    break;
                }
                if lines[j].starts_with(' ') || lines[j].starts_with('\t') || j == i + 1 {
                    keep[j] = true;
                    j += 1;
                    continue;
                }
                break;
            }
            i = j;
        } else if low.starts_with("goroutine ") && low.contains('[') {
            keep[i] = true;
            let mut j = i + 1;
            while j < lines.len()
                && (lines[j].starts_with('\t')
                    || lines[j].starts_with(' ')
                    || lines[j].trim_end().ends_with("()"))
            {
                keep[j] = true;
                j += 1;
            }
            i = j;
        } else if low.contains("exception in thread") {
            keep[i] = true;
            let mut j = i + 1;
            while j < lines.len() && lines[j].starts_with('\t') {
                keep[j] = true;
                j += 1;
            }
            i = j;
        } else if low.contains("error")
            && lines
                .get(i + 1)
                .is_some_and(|next| next.starts_with("    at "))
        {
            keep[i] = true;
            let mut j = i + 1;
            while j < lines.len() && (lines[j].starts_with("    at ") || lines[j].starts_with('\t'))
            {
                keep[j] = true;
                j += 1;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    keep
}

fn normalize_line_key(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    let b = line.as_bytes();
    while i < b.len() {
        let rest = &line[i..];
        if let Some((n, rep)) = placeholder_token(rest) {
            out.push_str(rep);
            i += n;
            continue;
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

fn placeholder_token(rest: &str) -> Option<(usize, &'static str)> {
    let b = rest.as_bytes();
    if b.len() >= 19
        && b[4] == b'-'
        && b[7] == b'-'
        && (b[10] == b'T' || b[10] == b' ')
        && b[13] == b':'
        && b[16] == b':'
        && b[0..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
        && b[11..13].iter().all(|c| c.is_ascii_digit())
        && b[14..16].iter().all(|c| c.is_ascii_digit())
        && b[17..19].iter().all(|c| c.is_ascii_digit())
    {
        return Some((19, "<TS>"));
    }
    if b.len() >= 20
        && b[2] == b'/'
        && b[6] == b'/'
        && b[11] == b':'
        && b[14] == b':'
        && b[17] == b':'
        && b[0..2].iter().all(|c| c.is_ascii_digit())
        && b[7..11].iter().all(|c| c.is_ascii_digit())
        && b[12..14].iter().all(|c| c.is_ascii_digit())
        && b[15..17].iter().all(|c| c.is_ascii_digit())
        && b[18..20].iter().all(|c| c.is_ascii_digit())
        && rest[3..6].chars().all(|c| c.is_ascii_alphabetic())
    {
        return Some((20, "<TS>"));
    }
    if rest.starts_with("pid=") {
        let mut j = 4usize;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > 4 {
            return Some((j, "pid=<PID>"));
        }
    }
    if let Some((n, _)) = uuid_at(rest) {
        return Some((n, "<ID>"));
    }
    if let Some(n) = long_hex_at(rest) {
        return Some((n, "<HEX>"));
    }
    if b.first().is_some_and(|c| c.is_ascii_digit()) {
        if let Some((n, _)) = duration_at(rest) {
            return Some((n, "<DUR>"));
        }
    }
    None
}

fn uuid_at(rest: &str) -> Option<(usize, ())> {
    let need = [(8, b'-'), (4, b'-'), (4, b'-'), (4, b'-'), (12, 0)];
    let mut pos = 0usize;
    for (len, sep) in need {
        if rest.len() < pos + len {
            return None;
        }
        if !rest[pos..pos + len].bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        pos += len;
        if sep != 0 {
            if rest.len() <= pos || rest.as_bytes()[pos] != sep {
                return None;
            }
            pos += 1;
        }
    }
    Some((pos, ()))
}

fn long_hex_at(rest: &str) -> Option<usize> {
    if rest.starts_with("0x") || rest.starts_with("0X") {
        return None;
    }
    let mut n = 0usize;
    for c in rest.chars() {
        if c.is_ascii_hexdigit() {
            n += 1;
        } else {
            break;
        }
    }
    if n >= 16 { Some(n) } else { None }
}

fn duration_at(rest: &str) -> Option<(usize, ())> {
    let b = rest.as_bytes();
    let mut i = 0usize;
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    if rest[i..].starts_with("ms") {
        return Some((i + 2, ()));
    }
    if rest[i..].starts_with('s') {
        return Some((i + 1, ()));
    }
    if i >= 5 && b[1] == b':' && b[3] == b':' {
        return Some((5, ()));
    }
    None
}

fn format_also_lines(nums: &[usize]) -> String {
    nums.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
}

fn normalized_dedupe(lines: Vec<String>) -> Vec<String> {
    if lines.is_empty() {
        return lines;
    }
    let keys: Vec<String> = lines.iter().map(|l| normalize_line_key(l)).collect();
    let mut first_idx = std::collections::HashMap::<&str, usize>::new();
    let mut counts = std::collections::HashMap::<&str, u32>::new();
    let mut also = std::collections::HashMap::<&str, Vec<usize>>::new();
    for (i, key) in keys.iter().enumerate() {
        let line_no = i + 1;
        if first_idx.contains_key(key.as_str()) {
            *counts.get_mut(key.as_str()).unwrap() += 1;
            also.get_mut(key.as_str()).unwrap().push(line_no);
        } else {
            first_idx.insert(key.as_str(), i);
            counts.insert(key.as_str(), 1);
            also.insert(key.as_str(), Vec::new());
        }
    }
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let key = &keys[i];
        if first_idx[key.as_str()] != i {
            continue;
        }
        let n = counts[key.as_str()];
        if n > 1 {
            let also_s = format_also_lines(&also[key.as_str()]);
            out.push(format!("{line} (×{n}, also lines {also_s})"));
        } else {
            out.push(line.clone());
        }
    }
    out
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

fn path_line(line: &str) -> Option<(String, String)> {
    let mut s = line.trim();
    if s.is_empty() || s.contains(char::is_whitespace) || s.starts_with('-') {
        return None;
    }
    if let Some(r) = s.strip_prefix("./") {
        s = r;
    }
    let drive = s.as_bytes().get(1) == Some(&b':');
    if s.contains(':') && !drive {
        return None;
    }
    let s = s.replace('\\', "/");
    match s.rsplit_once('/') {
        Some((_, n)) if n.is_empty() || n == "." || n == ".." => None,
        Some((d, n)) => Some((d.to_string(), n.to_string())),
        None if s.contains('.') && s != "." && s != ".." => Some((String::new(), s)),
        _ => None,
    }
}

fn fold(
    lines: Vec<String>,
    parse: impl Fn(&str) -> Option<(String, String, String)>,
    fmt: impl Fn(&str, &str, &[String], usize) -> String,
) -> Vec<String> {
    let mut groups: Vec<(String, String, Vec<String>, usize)> = Vec::new();
    let mut placed = Vec::new();
    let mut out: Vec<Result<usize, String>> = Vec::new();
    for line in lines {
        let Some((key, msg, extra)) = parse(&line) else {
            out.push(Err(line));
            continue;
        };
        let i = match groups.iter().position(|(k, _, _, _)| k == &key) {
            Some(i) => i,
            None => {
                groups.push((key, msg, Vec::new(), 0));
                placed.push(false);
                groups.len() - 1
            }
        };
        groups[i].3 += 1;
        if !extra.is_empty() {
            groups[i].2.push(extra);
        }
        if !placed[i] {
            placed[i] = true;
            out.push(Ok(i));
        }
    }
    out.into_iter()
        .map(|e| match e {
            Ok(i) => {
                let (key, msg, extra, n) = &groups[i];
                fmt(key, msg, extra, *n)
            }
            Err(s) => s,
        })
        .collect()
}

fn group_dir(lines: Vec<String>) -> Vec<String> {
    fold(lines, |l| path_line(l).map(|(d, n)| (d, String::new(), n)), |dir, _, names, n| {
        let mut list = names.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        if names.len() > 3 {
            list.push_str(" …");
        }
        let head = if dir.is_empty() { "." } else { dir };
        format!("{head}/ ({n} files): {list}").replacen("./ ", ". ", 1)
    })
}

fn is_exc(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_uppercase())
        && s.chars().all(|c| c.is_ascii_alphanumeric())
        && (s.ends_with("Error") || s.ends_with("Exception") || s.ends_with("Warning"))
}

fn loc_before(t: &str, p: usize) -> String {
    let s = t[..p].trim().trim_end_matches(':').trim();
    s.split_once('(').map_or(s.to_string(), |(file, rest)| {
        let line = rest.split([',', ')']).next().unwrap_or("");
        if file.is_empty() || line.is_empty() { s.to_string() } else { format!("{file}:{line}") }
    })
}

fn coded(t: &str, needle: &str, prefix: &str) -> Option<(String, String, String)> {
    let p = t.find(needle)?;
    let rest = &t[p + needle.len()..];
    let n = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    if n == 0 {
        return None;
    }
    Some((
        format!("{prefix}{}", &rest[..n]),
        rest[n..].trim_start_matches([':', ' ']).trim().to_string(),
        loc_before(t, p),
    ))
}

fn parse_diag(line: &str) -> Option<(String, String, String)> {
    let t = line.trim();
    if let Some(p) = t.find("error[").or_else(|| t.find("warning[")) {
        let kind_end = t[p..].find('[')? + p + 1;
        let end = t[kind_end..].find(']')? + kind_end;
        let key = t[kind_end..end].to_string();
        if key.is_empty() {
            return None;
        }
        let msg = t[end + 1..].trim_start_matches([':', ' ']).trim().to_string();
        return Some((key, msg, String::new()));
    }
    if let Some(v) = coded(t, "error TS", "TS").or_else(|| coded(t, "error CS", "CS")) {
        return Some(v);
    }
    let parts: Vec<&str> = t.split_whitespace().collect();
    if parts.len() >= 4 && matches!(parts[1], "error" | "warning") {
        let loc = parts[0];
        let (a, b) = loc.split_once(':')?;
        if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) {
            return Some((parts[parts.len() - 1].to_string(), parts[2..parts.len() - 1].join(" "), loc.to_string()));
        }
    }
    if let Some(rest) = t.strip_prefix("FAILED ") {
        if let Some((loc, err)) = rest.split_once(" - ") {
            if let Some((cls, msg)) = err.split_once(": ") {
                if is_exc(cls) {
                    return Some((cls.to_string(), msg.to_string(), loc.to_string()));
                }
            }
        }
    }
    if let Some((cls, msg)) = t.split_once(": ") {
        let cls = cls.trim().trim_start_matches("E ").trim();
        if is_exc(cls) {
            return Some((cls.to_string(), msg.to_string(), String::new()));
        }
    }
    None
}

fn group_diag(lines: Vec<String>) -> Vec<String> {
    fold(lines, parse_diag, |key, msg, locs, n| {
        let mut loc_s = locs.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        if locs.len() > 3 {
            loc_s.push_str(", …");
        }
        if loc_s.is_empty() {
            format!("{key} ×{n}: {msg}")
        } else {
            format!("{key} ×{n}: {msg} ({loc_s})")
        }
    })
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
    lines.retain(|l| {
        let low = l.to_ascii_lowercase();
        is_keep(&low, rule) || !is_drop(&low, rule)
    });
    // T65.4 first: when a trace block is in the output the columnar pass stands
    // down — traceback/YAML/diff indentation and frame padding are not columnar.
    let trace_kept = trace_blocks(&lines);
    if rule.collapse_columns && !trace_kept.iter().any(|t| *t) {
        for l in &mut lines {
            *l = collapse_columns(l);
        }
    }
    lines = match rule.dedupe {
        Dedupe::Off => lines,
        Dedupe::Adjacent => dedupe(lines),
        Dedupe::Normalized => normalized_dedupe(lines),
    };
    match rule.group {
        Group::Dir => lines = group_dir(lines),
        Group::Diag => lines = group_diag(lines),
        Group::Off => {}
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
        .filter(|(_, l)| is_keep(&l.to_ascii_lowercase(), rule))
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
    let mut last_pushed_trace = false;
    // Suffix count of trace lines still ahead, so the fold-into-trailer count stays
    // true when trace blocks keep printing past a spent budget (T65.4).
    let mut trace_rest_from = vec![0usize; total + 1];
    for i in (0..total).rev() {
        trace_rest_from[i] = trace_rest_from[i + 1] + usize::from(trace_kept[i]);
    }
    for (i, line) in lines.into_iter().enumerate() {
        // T65.4: a trace block's frames are never omitted, even past the budget or
        // behind a trailer — only the block's own length counts against `max_lines`.
        if trace_kept[i] {
            if omitted > 0 {
                picked.push(format!("… {omitted} lines omitted (expand {archive_id})"));
                omitted = 0;
            }
            picked.push(line);
            last_pushed_trace = true;
            continue;
        }
        if take[i] {
            if picked.len() < max {
                if omitted > 0 {
                    let extra = if picked.len() + 1 >= max {
                        (total - i) - trace_rest_from[i]
                    } else {
                        0
                    };
                    let count = omitted + extra;
                    picked.push(format!("… {count} lines omitted (expand {archive_id})"));
                    omitted = 0;
                    last_pushed_trace = false;
                    // Nothing but trace blocks prints past a spent budget; when none
                    // remain ahead, fold everything and stop (the pinned true-count
                    // tests hold only under that exact fold).
                    if trace_rest_from[i + 1] == 0 && picked.len() >= max {
                        break;
                    }
                }
                picked.push(line);
                last_pushed_trace = false;
            } else {
                omitted += 1;
            }
        } else {
            omitted += 1;
        }
    }
    if omitted > 0 {
        // Fold the trailer into a full budget only when the last pushed line is not
        // a trace line; a trace frame is never popped for it.
        if picked.len() >= max && !last_pushed_trace {
            picked.pop();
            omitted += 1;
        }
        picked.push(format!("… {omitted} lines omitted (expand {archive_id})"));
    }
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
            dedupe: parse_dedupe(t.get("dedupe"), Dedupe::Adjacent).unwrap_or(Dedupe::Adjacent),
            collapse_columns: t
                .get("collapse_columns")
                .and_then(|i| i.as_bool())
                .unwrap_or(false),
            group: match t.get("group").and_then(|v| v.as_str()) {
                Some("dir") => Group::Dir,
                Some("diag") => Group::Diag,
                _ => Group::Off,
            },
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
            dedupe: Dedupe::Off,
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
    fn normalize_line_key_keeps_distinct_error_codes() {
        assert_ne!(
            normalize_line_key("ERR-404 file missing"),
            normalize_line_key("ERR-500 file missing")
        );
        assert_eq!(
            normalize_line_key("req id=abcd1234abcd1234 done"),
            normalize_line_key("req id=feed9999aaaa1111 done")
        );
    }

    #[test]
    fn normalized_dedupe_folds_non_adjacent_matches() {
        let lines = vec![
            "2026-09-18T10:00:01 warn: slow".into(),
            "other".into(),
            "2026-09-18T10:00:02 warn: slow".into(),
        ];
        let out = normalized_dedupe(lines);
        assert_eq!(out.len(), 2);
        assert!(out[0].contains("(×2, also lines 3)"), "{}", out[0]);
    }

    #[test]
    fn normalized_dedupe_beats_plain_on_three_k_log_fixture() {
        let mut lines = Vec::with_capacity(3000);
        for i in 0..1000 {
            lines.push(format!(
                "2026-09-18T12:{:02}:00Z 127.0.0.1 GET /x 200",
                i % 60,
            ));
            if i % 2 == 0 {
                lines.push("separator".into());
            }
        }
        let warn = "warning: unused variable `x`";
        for i in 0..1000 {
            if i % 2 == 1 {
                lines.push("separator".into());
            }
            lines.push(warn.to_string());
        }
        let rule_adj = Rule {
            max_lines: 10_000,
            dedupe: Dedupe::Adjacent,
            ..Rule::default()
        };
        let rule_norm = Rule {
            max_lines: 10_000,
            dedupe: Dedupe::Normalized,
            ..Rule::default()
        };
        let body = lines.join("\n");
        let s = settings(80);
        let plain = apply(&s, &body, 0, &rule_adj, "arc");
        let norm = apply(&s, &body, 0, &rule_norm, "arc");
        assert!(norm.len() < plain.len(), "plain={} norm={}", plain.len(), norm.len());
    }

    // --- T65.4: a stack-trace block survives the head/tail cut whole ---

    // --- T65.3: column-padding collapse ---

    /// Runs of two or more spaces fold to one inside a line; leading indentation
    /// and tabs are kept; a trailing run folds to nothing.
    #[test]
    fn collapse_columns_folds_padding_keeps_indent_and_tabs() {
        assert_eq!(
            collapse_columns("NAME   STATUS      AGE"),
            "NAME STATUS AGE"
        );
        assert_eq!(
            collapse_columns("  indented    four  spaces"),
            "  indented four spaces"
        );
        assert_eq!(collapse_columns("trailing   "), "trailing");
        assert_eq!(collapse_columns("\tgo frame\ttab"), "\tgo frame\ttab");
        assert_eq!(collapse_columns("single already"), "single already");
    }

    /// A columnar fixture shrinks under `collapse_columns = true`, and the same
    /// output beside a traceback keeps its padding — the T65.4 stand-down.
    #[test]
    fn collapse_shrinks_columnar_output_and_stands_down_for_traces() {
        let s = settings(80);
        let rows: String = (0..30)
            .map(|i| {
                format!(
                    "pod-{i:04}     Running     0         3d2h  10.0.0.{i}
"
                )
            })
            .collect();
        let plain = Rule {
            collapse_columns: false,
            ..Rule::default()
        };
        let on = Rule::default();
        let with_rule = |rule: &Rule| apply(&s, &rows, 0, rule, "arc");
        assert!(
            with_rule(&on).len() * 2 < with_rule(&plain).len() * 3,
            "collapse saves ≥ a third: {} vs {}",
            with_rule(&on).len(),
            with_rule(&plain).len()
        );
        // Trace present: the pass stands down, padding survives everywhere.
        let mut with_trace = String::from("NAME    STATUS    AGE\n");
        with_trace.push_str(&rows);
        with_trace.push_str("Traceback (most recent call last):\n  File \"m.py\", line 1\n");
        let out = apply(&s, &with_trace, 0, &on, "arc");
        assert!(out.contains("NAME    STATUS    AGE"), "{out}");
    }

    /// One fixture per language — a trace block in the middle of a long output
    /// survives the default rule's cut with every frame, while an ordinary middle
    /// line is still cut and the trailer still names the archive id.
    #[test]
    fn stack_traces_survive_the_head_tail_cut_per_language() {
        let s = settings(80);
        let rule = Rule::default();
        let traces: &[&[&str]] = &[
            // Python: header + indented frames; the bare exception line is BUILTIN_KEEP.
            &[
                "Traceback (most recent call last):",
                "  File \"m.py\", line 3, in main",
                "    run()",
                "ValueError: boom",
            ],
            // Rust: header, the message line under it, and the backtrace frames.
            &[
                "thread 'main' panicked at src/m.rs:2:5:",
                "explicit panic",
                "stack backtrace:",
                "   0: rtok::main",
                "   1: core::ops::function::FnOnce::call_once",
            ],
            // JS: an Error line and its `    at ` frames.
            &[
                "Error: cannot read properties of undefined",
                "    at main (/x/app.js:10:13)",
                "    at Object.<anonymous> (/x/app.js:3:1)",
            ],
            // Go: the goroutine header and its `()` / tab-indented frames.
            &[
                "goroutine 1 [running]:",
                "main.main()",
                "\t/src/m.go:9 +0x2c",
            ],
            // Java: the exception header and its `\tat` frames.
            &[
                "Exception in thread \"main\" java.lang.NullPointerException",
                "\tat Main.run(Main.java:8)",
                "\tat Main.main(Main.java:4)",
            ],
        ];
        for trace in traces {
            let mut lines: Vec<String> = (0..15).map(|i| format!("ok {i}")).collect();
            lines.extend(trace.iter().map(|l| l.to_string()));
            lines.extend((15..45).map(|i| format!("ok {i}")));
            let out = apply(&s, &lines.join("\n"), 0, &rule, "arc1");
            for frame in *trace {
                assert!(out.contains(frame), "`{frame}` must survive:\n{out}");
            }
            assert!(
                out.contains("lines omitted (expand arc1)"),
                "the trailer names the archive id:\n{out}"
            );
            assert!(
                !out.contains("ok 30"),
                "an ordinary middle line is still cut:\n{out}"
            );
        }
    }

    // --- disk Settings::load twins (restored; keep coverage of real path I/O) ---

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
        let s = Settings::load(&path, None, 80);
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
        let s = Settings::load(&path, None, 80);
        assert_eq!(s.pick("grep").max_lines, 5);
        let _ = fs::remove_dir_all(&dir);
    }

    fn dropin(dir: &std::path::Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    /// Drop-ins merge after the single user file in name order: `b.toml` wins
    /// over both the file and `a.toml`, and a new family is appended.
    #[test]
    fn drop_ins_merge_in_name_order_after_the_user_file() {
        let dir = std::env::temp_dir().join(format!("rtok-rules-d-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let dropins = dir.join("rules.d");
        fs::create_dir_all(&dropins).unwrap();
        let file = dir.join("rules.toml");
        fs::write(&file, "[grep]\nmax_lines = 5\n").unwrap();
        dropin(&dropins, "b.toml", "[grep]\nmax_lines = 7\n");
        dropin(
            &dropins,
            "a.toml",
            "[grep]\nmax_lines = 6\n[pytest]\nmax_lines = 9\n",
        );
        dropin(&dropins, "skip.txt", "[grep]\nmax_lines = 1\n");
        let s = Settings::load(&file, Some(&dropins), 80);
        assert_eq!(s.pick("grep").max_lines, 7, "b.toml wins in name order");
        assert_eq!(s.pick("pytest").max_lines, 9, "new families append");
        assert_eq!(s.pick("cat").max_lines, 80, "untouched defaults stay");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A malformed drop-in is skipped, fail open: the good files still apply
    /// and the builtin underneath is untouched.
    #[test]
    fn a_broken_drop_in_is_skipped_fail_open() {
        let dir = std::env::temp_dir().join(format!("rtok-rules-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let dropins = dir.join("rules.d");
        fs::create_dir_all(&dropins).unwrap();
        dropin(&dropins, "bad.toml", "[grep\nmax_lines = \n");
        dropin(&dropins, "good.toml", "[grep]\nmax_lines = 7\n");
        let missing = dir.join("no-such-rules.toml");
        let s = Settings::load(&missing, Some(&dropins), 80);
        assert_eq!(s.pick("grep").max_lines, 7);
        // A missing dir is built-ins, not an error.
        let s = Settings::load(&missing, Some(&dir.join("no-such-d")), 80);
        assert_eq!(s.pick("grep").max_lines, 40);
        let _ = fs::remove_dir_all(&dir);
    }

    // --- Vfs twins (T56.3): same assertions, no host TempDir ---

    #[test]
    fn user_rules_file_changes_output_for_match_cmd_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write(
            "rules.toml",
            "[echo]\nmax_lines = 2\nhead = 1\ntail = 1\ndedupe = false\n",
        );
        let s = Settings::from_vfs(&vfs, "rules.toml", None, 80);
        let body = (0..10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rule = s.pick("echo");
        let out = apply(&s, &body, 0, &rule, "id");
        assert_eq!(out.lines().count(), 2, "{out}");
    }

    #[test]
    fn user_rule_overrides_builtin_match_cmd_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write(
            "rules.toml",
            "[grep]\nmax_lines = 5\nhead = 2\ntail = 2\ndedupe = false\n",
        );
        let s = Settings::from_vfs(&vfs, "rules.toml", None, 80);
        assert_eq!(s.pick("grep").max_lines, 5);
    }

    #[test]
    fn drop_ins_merge_in_name_order_after_the_user_file_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("rules.toml", "[grep]\nmax_lines = 5\n");
        vfs.write("rules.d/b.toml", "[grep]\nmax_lines = 7\n");
        vfs.write(
            "rules.d/a.toml",
            "[grep]\nmax_lines = 6\n[pytest]\nmax_lines = 9\n",
        );
        vfs.write("rules.d/skip.txt", "[grep]\nmax_lines = 1\n");
        let s = Settings::from_vfs(&vfs, "rules.toml", Some("rules.d"), 80);
        assert_eq!(s.pick("grep").max_lines, 7, "b.toml wins in name order");
        assert_eq!(s.pick("pytest").max_lines, 9, "new families append");
        assert_eq!(s.pick("cat").max_lines, 80, "untouched defaults stay");
    }

    #[test]
    fn a_broken_drop_in_is_skipped_fail_open_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("rules.d/bad.toml", "[grep\nmax_lines = \n");
        vfs.write("rules.d/good.toml", "[grep]\nmax_lines = 7\n");
        let s = Settings::from_vfs(&vfs, "no-such-rules.toml", Some("rules.d"), 80);
        assert_eq!(s.pick("grep").max_lines, 7);
        let s = Settings::from_vfs(&vfs, "no-such-rules.toml", Some("no-such-d"), 80);
        assert_eq!(s.pick("grep").max_lines, 40);
    }

    /// Extra Vfs edge cases: empty rules file keeps builtins; spaced path keys work.
    #[test]
    fn empty_rules_file_from_vfs_keeps_builtins() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("rules.toml", "");
        let s = Settings::from_vfs(&vfs, "rules.toml", None, 80);
        assert_eq!(s.pick("grep").max_lines, 40);
        assert_eq!(s.pick("cat").max_lines, 80);
        assert_eq!(
            s.pick("echo").max_lines,
            40,
            "unknown cmds use Rule::default"
        );
    }

    #[test]
    fn vfs_rules_under_spaced_profile_path() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write(
            "Users/Ivan Tuhai/.config/rtok/rules.toml",
            "[rg]\nmax_lines = 3\nhead = 1\ntail = 1\ndedupe = false\n",
        );
        let s = Settings::from_vfs(&vfs, "Users/Ivan Tuhai/.config/rtok/rules.toml", None, 80);
        assert_eq!(s.pick("rg").max_lines, 3);
        let body = (0..10)
            .map(|i| format!("L{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = apply(&s, &body, 0, &s.pick("rg"), "id");
        assert_eq!(out.lines().count(), 3, "{out}");
    }

    #[test]
    fn vfs_dropin_later_file_overrides_earlier_and_user() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("rules.toml", "[grep]\nmax_lines = 1\n");
        vfs.write("rules.d/01-a.toml", "[grep]\nmax_lines = 2\n");
        vfs.write("rules.d/02-b.toml", "[grep]\nmax_lines = 9\n");
        let s = Settings::from_vfs(&vfs, "rules.toml", Some("rules.d"), 80);
        assert_eq!(s.pick("grep").max_lines, 9);
    }

    /// Unicode path segments are ordinary Vfs keys (D29 string paths).
    #[test]
    fn vfs_rules_under_unicode_path_string() {
        let mut vfs = crate::testutil::Vfs::new();
        let path = "профіль/настройки/rules.toml";
        vfs.write(
            path,
            "[cat]\nmax_lines = 4\nhead = 2\ntail = 2\ndedupe = false\n",
        );
        let s = Settings::from_vfs(&vfs, path, None, 80);
        assert_eq!(s.pick("cat").max_lines, 4);
        let drop_dir = "профіль/настройки/rules.d";
        vfs.write(format!("{drop_dir}/zz.toml"), "[cat]\nmax_lines = 11\n");
        let s = Settings::from_vfs(&vfs, path, Some(drop_dir), 80);
        assert_eq!(s.pick("cat").max_lines, 11);
    }

    /// `max_lines = 0` parses; apply clamps the budget to 1 and head/tail to 0,
    /// so multi-line output becomes a single omission trailer.
    #[test]
    fn vfs_max_lines_zero_clamps_to_one_on_apply() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write(
            "rules.toml",
            "[echo]\nmax_lines = 0\nhead = 1\ntail = 1\ndedupe = false\n",
        );
        let s = Settings::from_vfs(&vfs, "rules.toml", None, 80);
        assert_eq!(s.pick("echo").max_lines, 0);
        let body = "a\nb\nc\n";
        let out = apply(&s, body, 0, &s.pick("echo"), "id");
        assert_eq!(out.lines().count(), 1, "{out}");
        assert!(out.contains("3 lines omitted"), "{out}");
    }

    /// Several drop-ins plus a non-toml sibling: only `*.toml` merge, in name order.
    #[test]
    fn vfs_many_dropins_merge_name_order_skipping_non_toml() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("rules.toml", "[grep]\nmax_lines = 1\n");
        vfs.write(
            "rules.d/10.toml",
            "[grep]\nmax_lines = 3\n[jq]\nmax_lines = 2\n",
        );
        vfs.write("rules.d/20.toml", "[grep]\nmax_lines = 5\n");
        vfs.write(
            "rules.d/30.toml",
            "[grep]\nmax_lines = 8\n[fd]\nmax_lines = 6\n",
        );
        vfs.write("rules.d/note.md", "[grep]\nmax_lines = 99\n");
        vfs.write("rules.d/00.bak", "[grep]\nmax_lines = 99\n");
        let s = Settings::from_vfs(&vfs, "rules.toml", Some("rules.d"), 80);
        assert_eq!(s.pick("grep").max_lines, 8);
        assert_eq!(s.pick("jq").max_lines, 2);
        assert_eq!(s.pick("fd").max_lines, 6);
    }

    #[test]
    fn strict_rejects_syntax_types_and_unknown_fields() {
        assert!(parse_strict("[grep]\nmax_lines = 5\n").is_ok());
        assert!(parse_strict("").is_ok());
        let bad_syntax = parse_strict("[grep\nmax_lines = \n").unwrap_err();
        assert!(bad_syntax.contains("line"), "{bad_syntax}");
        for (body, want) in [
            ("[grep]\nmax_lines = \"many\"\n", "max_lines"),
            ("[grep]\nmax_lines = -1\n", "max_lines"),
            ("[grep]\ndrop = \"x\"\n", "drop"),
            ("[grep]\ndrop = [1]\n", "drop"),
            ("[grep]\ndedupe = \"yes\"\n", "dedupe"),
            ("[grep]\ngroup = \"nope\"\n", "group"),
            ("[grep]\nnope = 1\n", "nope"),
            ("title = \"x\"\n", "title"),
        ] {
            let err = parse_strict(body).unwrap_err();
            assert!(err.contains(want), "{body} → {err}");
        }
        assert!(parse_strict("[grep]\ndedupe = \"normalized\"\n").is_ok());
    }

    #[test]
    fn issues_in_names_every_malformed_file() {
        let dir = std::env::temp_dir().join(format!("rtok-rules-iss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let dropins = dir.join("rules.d");
        fs::create_dir_all(&dropins).unwrap();
        dropin(&dropins, "bad.toml", "[grep]\nmax_lines = \"many\"\n");
        dropin(&dropins, "good.toml", "[echo]\nmax_lines = 2\n");
        let errs = issues_in(&dir.join("no-such-rules.toml"), &dropins);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("bad.toml"), "{errs:?}");
        assert!(errs[0].contains("max_lines"), "{errs:?}");
        // Missing file and dir report nothing.
        assert!(issues_in(&dir.join("nope.toml"), &dir.join("nope-d")).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
    /// T56.3 twin: malformed drop-in named in Vfs keys (keep disk `issues_in_*`).
    #[test]
    fn issues_in_names_every_malformed_file_from_vfs() {
        let mut vfs = crate::testutil::Vfs::new();
        vfs.write("rules.d/bad.toml", "[grep]\nmax_lines = \"many\"\n");
        vfs.write("rules.d/good.toml", "[echo]\nmax_lines = 2\n");
        let errs = issues_in_from_vfs(&vfs, "no-such-rules.toml", "rules.d");
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("bad.toml"), "{errs:?}");
        assert!(errs[0].contains("max_lines"), "{errs:?}");
        assert!(issues_in_from_vfs(&vfs, "nope.toml", "nope-d").is_empty());
    }

    #[test]
    fn issues_in_from_vfs_names_spaced_path() {
        let mut vfs = crate::testutil::Vfs::new();
        let path = "Users/Ivan Tuhai/.config/rtok/rules.d/bad.toml";
        vfs.write(path, "[grep]\nmax_lines = \"many\"\n");
        let errs = issues_in_from_vfs(
            &vfs,
            "missing.toml",
            "Users/Ivan Tuhai/.config/rtok/rules.d",
        );
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("Ivan Tuhai"), "{errs:?}");
        assert!(errs[0].contains("max_lines"), "{errs:?}");
    }
}
