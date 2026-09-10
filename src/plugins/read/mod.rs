//! `read` — MCP `read` / `search` / `tree` with modes, size caps and re-read dedup (plan P4).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};
use serde_json::json;

use rtok_plugin_sdk::{
    Ctx, DashboardPage, Manifest, Plugin, PostToolUse, PreToolDecision, PreToolUse, Surface,
    ToolDef,
};

pub mod cache;
pub mod hook;
pub(crate) mod outline;
pub mod search;

pub struct Read;

impl Plugin for Read {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "read",
            surfaces: &[Surface::Mcp, Surface::Hook],
            default_on: true,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "Read",
            "Outline, map, and search instead of dumping full files into context.",
            true,
        )
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        hook::pre_tool(ev, cx)
    }

    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> {
        cache::invalidate(ev, cx);
        None
    }

    fn mcp_tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "read",
                description: "Read a file; mode full|lines|map|signatures; optional range a-b.",
                input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"mode":{"type":"string"},"range":{"type":"string"}},"required":["path"]}),
            },
            ToolDef {
                name: "search",
                description: "Regex search files; path:line: snippet, max hits.",
                input_schema: json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"},"max":{"type":"integer"}},"required":["pattern"]}),
            },
            ToolDef {
                name: "tree",
                description: "Compact directory listing with sizes; depth cap.",
                input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"depth":{"type":"integer"}}}),
            },
        ]
    }
}

pub fn read(cx: &Ctx, path: &str, mode: &str, range: Option<&str>) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let cfg = cx.plugin_config::<crate::config::Read>("read");
    let abs = resolve(&cwd, Path::new(path), &cfg.allow_paths)?;
    let raw = std::fs::read_to_string(&abs)?;
    let mode = if mode.is_empty() {
        cfg.default_mode.as_str()
    } else {
        mode
    };
    let body = if mode == "map" || mode == "signatures" {
        outline::render(&abs, &raw, mode)?
    } else {
        let mut rows: Vec<(usize, &str)> =
            raw.lines().enumerate().map(|(i, l)| (i + 1, l)).collect();
        if mode == "lines"
            && let Some(spec) = range
            && let Some((a, b)) = spec.split_once('-')
            && let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>())
        {
            rows.retain(|(n, _)| *n >= a && *n <= b);
        }
        rows.iter()
            .map(|(n, l)| format!("{n}:{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let key = cache::key(abs.to_string_lossy().as_ref(), mode, range);
    if let Some(hit) = cache::hit(cx, &key, body.as_bytes(), body.lines().count()) {
        return Ok(hit);
    }
    let _ = cache::remember(cx, &key, body.as_bytes());
    cap(cx, body)
}

pub(crate) fn resolve(cwd: &Path, path: &Path, extra: &[PathBuf]) -> Result<PathBuf> {
    let abs = if path.is_absolute() {
        normalize(Path::new("/"), path)
    } else {
        normalize(cwd, path)
    };
    // Lexical allow, then (when the path exists) reject symlink escapes past the root.
    let check = abs.canonicalize().unwrap_or_else(|_| abs.clone());
    let roots: Vec<PathBuf> = std::iter::once(cwd.to_path_buf())
        .chain(extra.iter().cloned())
        .map(|r| r.canonicalize().unwrap_or(r))
        .collect();
    if roots.iter().any(|r| under(&check, r)) {
        return Ok(abs);
    }
    bail!("path outside cwd: {}", path.display())
}

fn normalize(root: &Path, path: &Path) -> PathBuf {
    let mut out = if path.is_absolute() {
        PathBuf::new()
    } else {
        root.to_path_buf()
    };
    for c in path.components() {
        match c {
            Component::RootDir => out = PathBuf::from("/"),
            Component::Prefix(p) => out = PathBuf::from(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

fn under(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

fn cap(cx: &Ctx, text: String) -> Result<String> {
    let max = cx.plugin_config::<crate::config::Read>("read").max_chars as usize;
    if text.chars().count() <= max {
        return Ok(text);
    }
    let id = cx.put_archive(text.as_bytes())?;
    let chars: Vec<char> = text.chars().collect();
    let keep = (max / 2).max(1);
    let head: String = chars.iter().take(keep).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(keep)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Ok(format!("{head}\n… archived {id} …\n{tail}"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;

    /// A runtime whose fresh temp dir is also the one `allow_paths` root; shared with `cache.rs`.
    pub(crate) fn cx(name: &str) -> (crate::plugin::Runtime, PathBuf) {
        let (mut c, dir) = crate::testutil::config(name);
        c.plugins.read.allow_paths = vec![dir.clone()];
        (crate::plugin::Runtime::open(c, name).unwrap(), dir)
    }

    #[test]
    fn three_lines_are_numbered() {
        let (cx, dir) = cx("three");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\nbeta\ngamma\n").unwrap();
        let out = read(&Ctx::new(&cx), p.to_str().unwrap(), "full", None).unwrap();
        assert_eq!(out, "1:alpha\n2:beta\n3:gamma");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn hundred_kb_is_capped_with_archive_id() {
        let (cx, dir) = cx("big");
        let p = dir.join("big.txt");
        let blob = "x".repeat(100 * 1024);
        fs::write(&p, &blob).unwrap();
        let out = read(&Ctx::new(&cx), p.to_str().unwrap(), "full", None).unwrap();
        assert!(out.contains("archived"), "{out}");
        assert!(out.chars().count() < blob.len(), "capped");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn symlink_escape_is_err() {
        let (_cx, dir) = cx("symlink");
        let cwd = dir.join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        // Target lives beside the allow_paths root, not under it.
        let outside = dir
            .parent()
            .unwrap()
            .join(format!("rtok-read-symlink-out-{}", std::process::id()));
        fs::write(&outside, "secret\n").unwrap();
        let link = cwd.join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(not(unix))]
        {
            let _ = fs::remove_file(&outside);
            let _ = fs::remove_dir_all(dir);
            return;
        }
        // `read` passes the process cwd to `resolve`; calling `resolve` with `cwd` and no
        // allow_paths checks the same guard without moving the cwd every parallel test shares
        // (moving it failed `map_src_main_lists_fn_main` on ubuntu CI, 2026-09-11).
        let err = resolve(&cwd, Path::new("escape"), &[])
            .unwrap_err()
            .to_string();
        let _ = fs::remove_file(&outside);
        assert!(err.contains("outside cwd"), "{err}");
        let _ = fs::remove_dir_all(dir);
    }

    /// The lexical twin of `symlink_escape_is_err`: `..` climbs past every allowed root.
    #[test]
    fn dot_dot_escape_is_err() {
        let (cx, dir) = cx("dotdot");
        let deep = "../".repeat(32) + "etc/passwd";
        assert!(read(&Ctx::new(&cx), &deep, "full", None).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn map_src_main_lists_fn_main() {
        let (cx, dir) = crate::testutil::runtime("mapmain");
        let out = read(&Ctx::new(&cx), "src/main.rs", "map", None).unwrap();
        assert!(out.contains("fn main"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }
}
