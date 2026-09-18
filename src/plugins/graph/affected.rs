//! `rtok graph affected` — which indexed tests a diff reaches (T68.5).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rtok_plugin_sdk::Ctx;
use serde_json::{Value, json};

use super::{impact_bfs, index, is_test_path};

const EMPTY: &str = "no indexed test reaches the change; run the suite";

pub fn run(
    cx: &Ctx,
    root: &Path,
    since: Option<&str>,
    staged: bool,
    json: bool,
) -> Result<String> {
    let changed = git_changed(root, since, staged);
    affected_text(cx, root, &changed, 3, json)
}

pub fn affected_text(
    cx: &Ctx,
    root: &Path,
    changed: &[String],
    depth: u32,
    json: bool,
) -> Result<String> {
    super::index_for(cx, root)?;
    let key = index::canon(root);
    let mut hits = BTreeSet::new();
    let mut starts = HashSet::new();
    for raw in changed {
        let rel = rel_of(root, raw);
        for name in defs_in_path(cx, root, &key, &rel)? {
            if is_test_path(&rel) {
                hits.insert((rel.clone(), name.clone()));
            }
            starts.insert(name);
        }
    }
    for name in &starts {
        for (_, path, scope) in impact_bfs(cx, &key, name, depth)? {
            if is_test_path(&path) {
                let via = if scope.is_empty() {
                    name.clone()
                } else {
                    scope
                };
                hits.insert((path, via));
            }
        }
    }
    Ok(format_hits(&hits, json))
}

pub fn impact_by_path(cx: &Ctx, root: &Path, rel: &str, depth: u32) -> Result<String> {
    affected_text(cx, root, &[rel.to_string()], depth, false)
}

fn defs_in_path(cx: &Ctx, root: &Path, key: &str, rel: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for (name, ..) in cx.symbol_file_defs(key, rel)? {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    if !names.is_empty() {
        return Ok(names);
    }
    let abs = root.join(rel);
    let src = std::fs::read_to_string(&abs).unwrap_or_default();
    let mut seen = HashSet::new();
    for hit in crate::plugins::read::outline::tags(&abs, &src)? {
        if hit.is_def && seen.insert(hit.name.clone()) {
            names.push(hit.name);
        }
    }
    Ok(names)
}

fn git_changed(root: &Path, since: Option<&str>, staged: bool) -> Vec<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .args(["diff", "--name-only", "--relative", "-z"]);
    if staged {
        cmd.arg("--cached");
    }
    if let Some(rev) = since {
        cmd.arg(rev);
    }
    let Ok(out) = cmd.output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    out.stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|s| String::from_utf8(s.to_vec()).ok())
        .collect()
}

fn rel_of(root: &Path, path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    match (
        dunce::canonicalize(root.join(path)),
        dunce::canonicalize(root),
    ) {
        (Ok(abs), Ok(root)) => abs
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string()),
        _ => path.to_string(),
    }
}

fn test_command(path: &str, name: &str) -> Option<String> {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "rs" => Some(format!("cargo test {name}")),
        "py" => Some(format!("pytest {path}::{name}")),
        "go" => Some(format!("go test -run {name}")),
        "ts" | "tsx" | "js" | "mjs" | "cjs" | "jsx" => Some(format!("vitest {path}")),
        _ => None,
    }
}

fn format_hits(hits: &BTreeSet<(String, String)>, json: bool) -> String {
    if json {
        let tests: Vec<Value> = hits
            .iter()
            .map(|(file, symbol)| {
                json!({
                    "file": file,
                    "symbol": symbol,
                    "command": test_command(file, symbol),
                })
            })
            .collect();
        return if tests.is_empty() {
            json!({"tests": [], "message": EMPTY}).to_string()
        } else {
            json!({"tests": tests}).to_string()
        };
    }
    if hits.is_empty() {
        return EMPTY.to_string();
    }
    let mut out = String::new();
    for (file, symbol) in hits {
        out.push_str(&format!("{file} ← via {symbol}
"));
        if let Some(cmd) = test_command(file, symbol) {
            out.push_str(&format!("{cmd}
"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Ctx;
    use std::fs;

    fn fixture(tag: &str) -> (Ctx<'static>, PathBuf) {
        let (rt, dir) = crate::testutil::runtime(tag);
        let lib = dir.join("lib.rs");
        fs::write(&lib, "pub fn helper() {}
").unwrap();
        let test = dir.join("tests/reaches.rs");
        fs::create_dir_all(dir.join("tests")).unwrap();
        fs::write(
            &test,
            "#[test] fn reaches() { mycrate::helper(); }
",
        )
        .unwrap();
        let ctx = Ctx::new(&rt);
        super::super::index::run(&ctx, &dir, false).unwrap();
        (ctx, dir)
    }

    #[test]
    fn fixture_finds_test_via_helper() {
        let (ctx, dir) = fixture("affected");
        let out = affected_text(&ctx, &dir, &["lib.rs".into()], 3, false).unwrap();
        assert!(out.contains("tests/reaches.rs"), "{out}");
    }
}
