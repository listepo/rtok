//! `read` — MCP `read` / `search` / `tree` with modes, size caps and re-read dedup (plan P4).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.

use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};
use serde_json::json;

use crate::plugin::{Ctx, Manifest, Plugin, Surface, ToolDef};

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

    fn mcp_tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "read",
                description: "Read a file; mode full|lines; optional range a-b.",
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

/// Numbered file contents. `mode=lines` honours `range` (`a-b`, 1-based inclusive).
pub fn read(cx: &Ctx, path: &str, mode: &str, range: Option<&str>) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let abs = resolve(&cwd, Path::new(path), &cx.config.plugins.read.allow_paths)?;
    let raw = std::fs::read_to_string(&abs)?;
    let mut rows: Vec<(usize, &str)> = raw.lines().enumerate().map(|(i, l)| (i + 1, l)).collect();
    let mode = if mode.is_empty() {
        cx.config.plugins.read.default_mode.as_str()
    } else {
        mode
    };
    if mode == "lines"
        && let Some(spec) = range
        && let Some((a, b)) = spec.split_once('-')
        && let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>())
    {
        rows.retain(|(n, _)| *n >= a && *n <= b);
    }
    let numbered = rows
        .iter()
        .map(|(n, l)| format!("{n}:{l}"))
        .collect::<Vec<_>>()
        .join("\n");
    cap(cx, numbered)
}

pub(crate) fn resolve(cwd: &Path, path: &Path, extra: &[PathBuf]) -> Result<PathBuf> {
    let abs = if path.is_absolute() {
        normalize(Path::new("/"), path)
    } else {
        normalize(cwd, path)
    };
    if under(&abs, cwd) || extra.iter().any(|r| under(&abs, r)) {
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
    let max = cx.config.plugins.read.max_chars as usize;
    if text.chars().count() <= max {
        return Ok(text);
    }
    let id = cx
        .store
        .put_archive(&cx.session, text.as_bytes(), &cx.config.core.archive_dir)?;
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
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;

    fn cx(name: &str) -> (Ctx, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-read-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        c.plugins.read.allow_paths = vec![dir.clone()];
        (Ctx::open(c, name).unwrap(), dir)
    }

    #[test]
    fn three_lines_are_numbered() {
        let (cx, dir) = cx("three");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\nbeta\ngamma\n").unwrap();
        let out = read(&cx, p.to_str().unwrap(), "full", None).unwrap();
        assert_eq!(out, "1:alpha\n2:beta\n3:gamma");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn hundred_kb_is_capped_with_archive_id() {
        let (cx, dir) = cx("big");
        let p = dir.join("big.txt");
        let blob = "x".repeat(100 * 1024);
        fs::write(&p, &blob).unwrap();
        let out = read(&cx, p.to_str().unwrap(), "full", None).unwrap();
        assert!(out.contains("archived"), "{out}");
        assert!(out.chars().count() < blob.len(), "capped");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dotdot_etc_passwd_is_err() {
        let (cx, dir) = cx("guard");
        let err = read(&cx, "../etc/passwd", "full", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("outside cwd"), "{err}");
        let _ = fs::remove_dir_all(dir);
    }
}
