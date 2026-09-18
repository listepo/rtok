//! Generic grouping pass for `cmd` rules (T64.1): `group = "dir"` or `"diag"`.

use std::collections::BTreeMap;

/// Rewrite `lines` when `group` is `"dir"` or `"diag"`; otherwise return unchanged.
pub fn apply(group: Option<&str>, lines: Vec<String>) -> Vec<String> {
    match group {
        Some("dir") => group_dir(lines),
        Some("diag") => group_diag(lines),
        _ => lines,
    }
}

fn group_dir(lines: Vec<String>) -> Vec<String> {
    let mut passthrough = Vec::new();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in lines {
        let trimmed = line.trim();
        if let Some(path) = path_token(trimmed) {
            let (dir, file) = split_dir(path);
            groups.entry(dir).or_default().push(file);
        } else {
            passthrough.push(line);
        }
    }
    if groups.is_empty() {
        return passthrough;
    }
    let mut out = passthrough;
    for (dir, mut files) in groups {
        files.sort();
        files.dedup();
        let n = files.len();
        let preview: Vec<_> = files.iter().take(6).cloned().collect();
        let mut line = format!("{dir}/ ({n} files): {}", preview.join(", "));
        if n > preview.len() {
            line.push_str(&format!(" … +{} more", n - preview.len()));
        }
        out.push(line);
    }
    out
}

fn group_diag(lines: Vec<String>) -> Vec<String> {
    let mut passthrough = Vec::new();
    let mut groups: BTreeMap<String, (u32, String, Vec<String>)> = BTreeMap::new();
    for line in lines {
        if let Some((code, msg, site)) = parse_diag(&line) {
            let e = groups.entry(code).or_insert((0, msg, Vec::new()));
            e.0 += 1;
            if e.2.len() < 4 {
                e.2.push(site);
            }
        } else {
            passthrough.push(line);
        }
    }
    if groups.is_empty() {
        return passthrough;
    }
    let mut out = passthrough;
    for (code, (n, msg, sites)) in groups {
        let sites_s = sites.join(", ");
        if n > 1 {
            out.push(format!("{code} ×{n}: {msg} ({sites_s})"));
        } else {
            out.push(format!("{code}: {msg} ({sites_s})"));
        }
    }
    out
}

fn path_token(line: &str) -> Option<&str> {
    if line.is_empty() || line.contains(':') && !line.starts_with("./") && line.contains('/') {
        // Likely a diagnostic line, not a bare path listing.
        if line.contains("error ") || line.contains("Error") || line.contains("TS") {
            return None;
        }
    }
    let token = line.split_whitespace().last().unwrap_or(line);
    if looks_like_path(token) {
        Some(token)
    } else if looks_like_path(line) {
        Some(line)
    } else {
        None
    }
}

fn looks_like_path(s: &str) -> bool {
    (s.contains('/') || s.contains('\\'))
        && !s.starts_with("http")
        && !s.starts_with('-')
        && s.chars().any(|c| c == '.' || c == '/')
}

fn split_dir(path: &str) -> (String, String) {
    let p = path.replace('\\', "/");
    match p.rsplit_once('/') {
        Some(("", file)) => (".".into(), file.to_string()),
        Some((dir, file)) => (dir.to_string(), file.to_string()),
        None => (".".into(), p),
    }
}

fn parse_diag(line: &str) -> Option<(String, String, String)> {
    // Rust: error[E0308]: msg --> file:line:col
    if let Some(rest) = line.strip_prefix("error[") {
        let (code, tail) = rest.split_once("]: ")?;
        let msg = tail.split(" --> ").next().unwrap_or(tail).trim();
        let site = tail
            .split(" --> ")
            .nth(1)
            .unwrap_or("")
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        return Some((format!("error[{code}]"), msg.to_string(), site));
    }
    // TypeScript: path(line,col): error TS2304: msg
    if let Some(idx) = line.find(": error TS") {
        let site = line[..idx].trim().to_string();
        let tail = &line[idx + 2..];
        let code = tail
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let msg = tail.split_once(':').map(|(_, m)| m.trim()).unwrap_or("");
        if !code.is_empty() {
            return Some((code.to_string(), msg.to_string(), site));
        }
    }
    // ESLint: path:line:col: error rule-id msg
    if let Some((head, tail)) = line.split_once(": error ") {
        let site = head.trim().to_string();
        let rule = tail.split_whitespace().next().unwrap_or("error");
        let msg = tail.split_once(' ').map(|(_, m)| m.trim()).unwrap_or(tail);
        return Some((rule.to_string(), msg.to_string(), site));
    }
    // Python exception class at end of traceback frame line
    if line.chars().next().is_some_and(char::is_ascii_uppercase) && line.contains("Error:") {
        let code = line.split(':').next().unwrap_or(line).trim();
        return Some((code.to_string(), line.to_string(), String::new()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_groups_find_output() {
        let lines = vec![
            "src/a.rs".into(),
            "src/b.rs".into(),
            "lib/x.rs".into(),
        ];
        let out = group_dir(lines);
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(|l| l.contains("src/ (2 files)")));
        assert!(out.iter().any(|l| l.contains("lib/ (1 files)")));
    }

    #[test]
    fn diag_groups_tsc_errors() {
        let lines = vec![
            "src/a.ts(1,2): error TS2304: Cannot find name 'x'.".into(),
            "src/b.ts(3,4): error TS2304: Cannot find name 'y'.".into(),
        ];
        let out = group_diag(lines);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("TS2304 ×2"));
    }
}
