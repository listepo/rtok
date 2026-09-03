//! `graph` — `symbol` / `callers` / `outline` over a symbol index rtok builds itself
//! with tree-sitter-tags, with capped output (plan P8).
//!
//! Spec: the catalogue in `plan.md` §1 names the tools this replaces; none is a
//! dependency (D6) — the behaviour is re-implemented here.
//!
//! T8.2: three MCP tools. Each call first runs the incremental index over the current
//! directory (unchanged files are skipped by sha256, so a call costs one directory walk),
//! then answers from the `symbols` table. Every response is capped at
//! `plugins.graph.max_tokens`: the head lines that fit, then `N more, expand <id>` with the
//! full text archived. One `cap` measurement per call records capped vs uncapped estimate.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{Value, json};

use crate::plugin::{Ctx, Manifest, Measurement, Plugin, PostToolUse, Surface, ToolDef};
use crate::tokens::Class;

pub mod index;

pub struct Graph;

impl Plugin for Graph {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "graph",
            surfaces: &[Surface::Mcp],
            default_on: true,
        }
    }

    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> {
        if ev.tool_name != "Edit" && ev.tool_name != "Write" {
            return None;
        }
        if let Some(p) = ev.tool_input.get("file_path").and_then(|v| v.as_str()) {
            let _ = cx.store.mark_symbols_stale(&index::canon(Path::new(p)));
        }
        None
    }

    fn mcp_tools(&self) -> Vec<ToolDef> {
        let name =
            json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]});
        vec![
            ToolDef {
                name: "symbol",
                description: "Definitions of a symbol: path:line kind.",
                input_schema: name.clone(),
            },
            ToolDef {
                name: "callers",
                description: "Reference sites of a symbol grouped by file, with the line text.",
                input_schema: name,
            },
            ToolDef {
                name: "outline",
                description: "Definitions in one file (read mode=map).",
                input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
            },
        ]
    }
}

/// MCP dispatch for the three tools (`mcp.rs` `invoke`). Errors become the result text.
pub fn call(cx: &Ctx, name: &str, args: &Value) -> String {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let arg = |k: &str| args[k].as_str().unwrap_or("");
    match name {
        "symbol" => symbol(cx, &root, arg("name")),
        "callers" => callers(cx, &root, arg("name")),
        "outline" => outline(cx, arg("path")),
        _ => Ok(format!("unknown tool: {name}")),
    }
    .unwrap_or_else(|e| e.to_string())
}

/// `symbol(name)`: one `path:line kind` per definition.
pub fn symbol(cx: &Ctx, root: &Path, name: &str) -> Result<String> {
    index::run(cx, root)?;
    let rows = cx.store.symbol_defs(&index::canon(root), name)?;
    if rows.is_empty() {
        return Ok(format!("no definition of {name}"));
    }
    let text = rows
        .iter()
        .map(|(path, kind, line)| format!("{path}:{line} {kind}"))
        .collect::<Vec<_>>()
        .join("\n");
    cap(cx, text)
}

/// `callers(name)`: reference sites grouped by file, `  line: text` under each path.
pub fn callers(cx: &Ctx, root: &Path, name: &str) -> Result<String> {
    index::run(cx, root)?;
    let rows = cx.store.symbol_refs(&index::canon(root), name)?;
    if rows.is_empty() {
        return Ok(format!("no references to {name}"));
    }
    let mut out = String::new();
    let mut file: Option<(String, Vec<String>)> = None;
    for (path, line) in rows {
        if file.as_ref().is_none_or(|(p, _)| *p != path) {
            out.push_str(&path);
            out.push('\n');
            file = Some((path.clone(), file_lines(root, &path)));
        }
        let text = file
            .as_ref()
            .and_then(|(_, lines)| lines.get(line.max(1) as usize - 1))
            .map(|l| l.trim())
            .unwrap_or("");
        out.push_str(&format!("  {line}: {text}\n"));
    }
    cap(cx, out)
}

/// `outline(path)`: the `read` plugin's `map` mode, capped like the other two.
pub fn outline(cx: &Ctx, path: &str) -> Result<String> {
    let text = crate::plugins::read::read(cx, path, "map", None)?;
    cap(cx, text)
}

fn file_lines(root: &Path, rel: &str) -> Vec<String> {
    std::fs::read_to_string(root.join(rel))
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Cap at `plugins.graph.max_tokens`: whole head lines that fit, then `N more, expand <id>`.
/// Always records one measurement (capped vs uncapped estimate); `ref_id` when truncated.
fn cap(cx: &Ctx, text: String) -> Result<String> {
    let max = cx.config.plugins.graph.max_tokens;
    let est = cx.estimate(&text, Class::Code);
    let before_bytes = text.len() as u64;
    let (out, ref_id) = if est <= max {
        (text, None)
    } else {
        let id = cx
            .store
            .put_archive(&cx.session, text.as_bytes(), &cx.config.core.archive_dir)?;
        // The estimator is linear in chars, so the char budget scales the same way;
        // leave room for the trailer line (count + a 64-hex archive id).
        let budget = (text.len() * max as usize / est as usize).saturating_sub(120);
        let total = text.lines().count();
        let mut head = String::new();
        let mut shown = 0;
        for line in text.lines() {
            if shown > 0 && head.len() + line.len() + 1 > budget {
                break;
            }
            head.push_str(line);
            head.push('\n');
            shown += 1;
        }
        (
            format!("{head}{} more, expand {id}", total - shown),
            Some(id),
        )
    };
    cx.record(&Measurement {
        plugin: "graph",
        kind: "cap",
        before_bytes,
        after_bytes: out.len() as u64,
        est_before: est,
        est_after: cx.estimate(&out, Class::Code),
        ref_id,
        call_id: cx.call_id,
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::index::tests::cx;
    use super::*;
    use std::fs;

    fn crate_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn symbol_main_is_in_src_main_rs() {
        let (cx, dir) = cx("symbol");
        let out = symbol(&cx, &crate_root(), "main").unwrap();
        assert!(out.lines().any(|l| l.starts_with("src/main.rs:")), "{out}");
        assert_eq!(
            symbol(&cx, &crate_root(), "no_such_fn").unwrap(),
            "no definition of no_such_fn"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn callers_estimate_lists_src_plugin_rs() {
        let (cx, dir) = cx("callers");
        let out = callers(&cx, &crate_root(), "estimate").unwrap();
        let mut lines = out.lines();
        assert!(lines.any(|l| l == "src/plugin.rs"), "{out}");
        let site = lines.next().unwrap_or("");
        assert!(
            site.starts_with("  ") && site.contains("estimate"),
            "{site}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn five_hundred_hits_are_capped_with_archive_id() {
        let (cx, dir) = cx("cap");
        let mut src = String::from("fn zeta() {}\nfn main() {\n");
        for _ in 0..500 {
            src.push_str("    zeta();\n");
        }
        src.push_str("}\n");
        fs::write(dir.join("zeta.rs"), &src).unwrap();
        let out = callers(&cx, &dir, "zeta").unwrap();
        let trailer = out.lines().last().unwrap();
        assert!(trailer.contains(" more, expand "), "{trailer}");
        let id = trailer.rsplit(' ').next().unwrap();
        let full = cx
            .store
            .get_archive(id)
            .unwrap()
            .expect("archived full text");
        assert_eq!(String::from_utf8(full).unwrap().lines().count(), 501);
        let max = cx.config.plugins.graph.max_tokens;
        let est = cx.estimate(&out, Class::Code);
        assert!(est <= max, "{est} > {max}");
        assert_eq!(cx.store.measurement_count("graph").unwrap(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn outline_reuses_read_map() {
        let (cx, dir) = cx("outline");
        let out = outline(&cx, "src/main.rs").unwrap();
        assert!(out.contains("main"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }
}
