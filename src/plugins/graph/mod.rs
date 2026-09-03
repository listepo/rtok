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

use std::collections::HashSet;
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
                description: "Definitions of a symbol with their source: path:line kind, then the body.",
                input_schema: name.clone(),
            },
            ToolDef {
                name: "callers",
                description: "Which definitions reference a symbol: path, calling definition, count.",
                input_schema: name,
            },
            ToolDef {
                name: "impact",
                description: "What breaks if a symbol changes: callers, their callers, up to depth.",
                input_schema: json!({"type":"object","properties":{"name":{"type":"string"},"depth":{"type":"integer"}},"required":["name"]}),
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
        "impact" => impact(
            cx,
            &root,
            arg("name"),
            args["depth"].as_u64().unwrap_or(2) as u32,
        ),
        "outline" => outline(cx, arg("path")),
        _ => Ok(format!("unknown tool: {name}")),
    }
    .unwrap_or_else(|e| e.to_string())
}

/// `symbol(name)`: `path:line kind` per definition, then that definition's source from
/// `line` to `end_line`, at most `plugins.graph.body_lines` lines each (T8.6). One call
/// answers "what is this and what does it do", which took a `symbol` plus a `read` at v0.1.
pub fn symbol(cx: &Ctx, root: &Path, name: &str) -> Result<String> {
    index::run(cx, root)?;
    let rows = cx.store.symbol_defs(&index::canon(root), name)?;
    if rows.is_empty() {
        return Ok(format!("no definition of {name}"));
    }
    let budget = cx.config.plugins.graph.body_lines as usize;
    let mut out = String::new();
    for (path, kind, line, end_line) in &rows {
        out.push_str(&format!("{path}:{line} {kind}\n"));
        let src = std::fs::read_to_string(root.join(path)).unwrap_or_default();
        let first = (*line).max(1) as usize - 1;
        let last = (*end_line).max(*line) as usize;
        let body: Vec<&str> = src.lines().skip(first).take(last - first).collect();
        for l in body.iter().take(budget) {
            out.push_str(l);
            out.push('\n');
        }
        if body.len() > budget {
            out.push_str(&format!("  … {} more lines\n", body.len() - budget));
        }
    }
    cap(cx, out)
}

/// `callers(name)`: one line per calling definition, `path  scope xN (Lline)` (T8.5).
/// v0.1 printed every site with its source line; the edge is what the caller needs, and it
/// costs a fraction of the bytes.
pub fn callers(cx: &Ctx, root: &Path, name: &str) -> Result<String> {
    index::run(cx, root)?;
    let rows = cx.store.symbol_ref_groups(&index::canon(root), name)?;
    if rows.is_empty() {
        return Ok(format!("no references to {name}"));
    }
    let mut out = String::new();
    for (path, scope, n, line) in rows {
        let scope = if scope.is_empty() {
            String::new()
        } else {
            format!("  {scope}")
        };
        out.push_str(&format!("{path}{scope} ×{n} (L{line})\n"));
    }
    cap(cx, out)
}

/// `impact(name, depth)`: breadth-first walk of the `scope` edges T8.5 stored — who calls
/// `name`, who calls them, and so on (T8.7). One `depth  path  scope` line per definition
/// reached. A definition is expanded once, so a call cycle terminates.
pub fn impact(cx: &Ctx, root: &Path, name: &str, depth: u32) -> Result<String> {
    index::run(cx, root)?;
    let key = index::canon(root);
    let mut seen: HashSet<String> = HashSet::from([name.to_string()]);
    let mut frontier = vec![name.to_string()];
    let mut out = String::new();
    for d in 1..=depth.clamp(1, 4) {
        let mut next = Vec::new();
        for from in &frontier {
            for (path, scope, ..) in cx.store.symbol_ref_groups(&key, from)? {
                // A file-level reference has no definition to walk on from; it is still a
                // place the change lands, so it is reported and not expanded.
                if scope.is_empty() {
                    out.push_str(&format!("{d}  {path}  (file)\n"));
                } else if seen.insert(scope.clone()) {
                    out.push_str(&format!("{d}  {path}  {scope}\n"));
                    next.push(scope);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    if out.is_empty() {
        return Ok(format!("nothing reaches {name}"));
    }
    cap(cx, out)
}

/// `outline(path)`: the `read` plugin's `map` mode, capped like the other two.
pub fn outline(cx: &Ctx, path: &str) -> Result<String> {
    let text = crate::plugins::read::read(cx, path, "map", None)?;
    cap(cx, text)
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

    /// T8.6: one call gives the definition and its source. The `cap` body is read back
    /// verbatim from the file, so the test cannot pass on a stale or paraphrased index.
    #[test]
    fn symbol_returns_the_definition_body() {
        let (cx, dir) = cx("body");
        let out = symbol(&cx, &crate_root(), "cap").unwrap();
        let src = fs::read_to_string(crate_root().join("src/plugins/graph/mod.rs")).unwrap();
        let head = src
            .lines()
            .find(|l| l.starts_with("fn cap(cx: &Ctx"))
            .unwrap();
        assert!(out.contains(head), "{out}");
        assert!(
            out.lines().any(|l| l.contains("src/plugins/graph/mod.rs:")),
            "{out}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.6: bodies make `symbol` far larger than v0.1, so the cap and the archive matter
    /// more, not less. 500 one-line definitions of the same name must still fit the budget.
    #[test]
    fn five_hundred_definitions_are_capped_with_archive_id() {
        let (cx, dir) = cx("defcap");
        // One file, not 500: the index runs a transaction per file, and 500 of them put
        // ~13 s of fixture setup into every `just check` for nothing this test measures.
        let src = "fn dup() {\n    ();\n}\n".repeat(500);
        fs::write(dir.join("d.rs"), &src).unwrap();
        let out = symbol(&cx, &dir, "dup").unwrap();
        let trailer = out.lines().last().unwrap();
        assert!(trailer.contains(" more, expand "), "{trailer}");
        let id = trailer.rsplit(' ').next().unwrap();
        let full = String::from_utf8(cx.store.get_archive(id).unwrap().unwrap()).unwrap();
        assert_eq!(
            full.lines().filter(|l| l.starts_with("fn dup()")).count(),
            500,
            "the archive holds every definition"
        );
        assert!(cx.estimate(&out, Class::Code) <= cx.config.plugins.graph.max_tokens);
        assert_eq!(cx.store.measurement_count("graph").unwrap(), 1);
        let _ = fs::remove_dir_all(dir);
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

    /// T8.5: the caller is a definition, not a line number. `Ctx::estimate` calls the free
    /// `tokens::estimate`, so `src/plugin.rs` must report `estimate` as the calling scope.
    #[test]
    fn callers_estimate_lists_src_plugin_rs() {
        let (cx, dir) = cx("callers");
        let out = callers(&cx, &crate_root(), "estimate").unwrap();
        assert!(
            out.lines()
                .any(|l| l.starts_with("src/plugin.rs  estimate \u{d7}")),
            "{out}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn five_hundred_hits_are_capped_with_archive_id() {
        let (cx, dir) = cx("cap");
        // 500 distinct callers, not 500 calls: grouping collapses the latter to one line.
        let mut src = String::from("fn zeta() {}\n");
        for i in 0..500 {
            src.push_str(&format!("fn c{i}() {{ zeta(); }}\n"));
        }
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
        assert_eq!(String::from_utf8(full).unwrap().lines().count(), 500);
        let max = cx.config.plugins.graph.max_tokens;
        let est = cx.estimate(&out, Class::Code);
        assert!(est <= max, "{est} > {max}");
        assert_eq!(cx.store.measurement_count("graph").unwrap(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.7: `impact` walks the edges `callers` only reports one hop of. Depth bounds the
    /// walk, and `x`/`y` calling each other must not loop.
    #[test]
    fn impact_walks_the_call_chain_and_terminates() {
        let (cx, dir) = cx("impact");
        fs::write(
            dir.join("chain.rs"),
            "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn x() {\n    y();\n    c();\n}\nfn y() {\n    x();\n}\n",
        )
        .unwrap();
        let at = |out: &str, n: &str| {
            out.lines()
                .find(|l| l.ends_with(&format!("  {n}")))
                .map(|l| l[..1].to_string())
        };
        let two = impact(&cx, &dir, "c", 2).unwrap();
        assert_eq!(at(&two, "b").as_deref(), Some("1"), "{two}");
        assert_eq!(at(&two, "a").as_deref(), Some("2"), "{two}");
        let one = impact(&cx, &dir, "c", 1).unwrap();
        assert_eq!(at(&one, "b").as_deref(), Some("1"), "{one}");
        assert_eq!(at(&one, "a"), None, "depth 1 must stop at the callers");
        // x calls y, y calls x, both reach c: the walk visits each once and returns.
        let deep = impact(&cx, &dir, "c", 4).unwrap();
        assert_eq!(
            deep.lines().filter(|l| l.ends_with("  x")).count(),
            1,
            "{deep}"
        );
        assert_eq!(
            impact(&cx, &dir, "no_such_fn", 2).unwrap(),
            "nothing reaches no_such_fn"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// Gate P8b: four tools, and the whole graph surface under 150 description tokens.
    #[test]
    fn graph_surface_is_four_tools_under_150_tokens() {
        let (cx, dir) = cx("surface");
        let tools = Graph.mcp_tools();
        assert_eq!(tools.len(), 4);
        let n: u32 = tools
            .iter()
            .map(|t| crate::tokens::estimate(t.description, Class::Prose, &cx.config.estimator))
            .sum();
        println!(
            "graph surface: {} tools, {n} description tokens",
            tools.len()
        );
        assert!(n <= 150, "graph descriptions are {n} tokens");
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
