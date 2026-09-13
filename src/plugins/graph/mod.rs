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

#[cfg(test)]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use serde_json::{Value, json};

use rtok_plugin_sdk::{
    Class, Ctx, DashboardPage, Manifest, Measurement, Plugin, PostToolUse, Surface, ToolDef,
};

pub mod index;
pub mod lsp;
pub mod watch;

#[cfg(test)]
thread_local! {
    pub(crate) static SYMBOL_SRC_READS: AtomicUsize = const { AtomicUsize::new(0) };
}

pub struct Graph;

impl Plugin for Graph {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "graph",
            surfaces: &[Surface::Mcp],
            default_on: true,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "Graph",
            "symbol / callers / impact from a tree-sitter-tags index.",
            true,
        )
    }

    fn post_tool(&self, ev: &PostToolUse, cx: &Ctx) -> Option<String> {
        if ev.tool_name != "Edit" && ev.tool_name != "Write" {
            return None;
        }
        if let Some(p) = ev.tool_input.get("file_path").and_then(|v| v.as_str()) {
            let _ = cx.mark_symbols_stale(&index::canon(Path::new(p)));
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

/// Walk or not (T8.15): `auto_index = true` is today's behaviour — every call
/// walks and the stat gate skips unchanged files. `false` indexes a root with no
/// rows once (`index::ensure`) and never walks again; re-indexing is
/// `rtok graph index` or the P8d watcher, and a hook-staled file reads as
/// missing until then.
pub fn index_for(cx: &Ctx, root: &Path) -> Result<index::Report> {
    if cx.plugin_config::<crate::config::Graph>("graph").auto_index {
        index::run(cx, root, false)
    } else {
        index::ensure(cx, root)
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
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        return lsp::symbol(cx, root, name);
    }
    index_for(cx, root)?;
    let rows = cx.symbol_defs(&index::canon(root), name)?;
    if rows.is_empty() {
        return Ok(format!("no definition of {name}"));
    }
    let budget = cx.plugin_config::<crate::config::Graph>("graph").body_lines as usize;
    let mut out = String::new();
    let mut cached: Option<(String, String)> = None;
    for (path, kind, line, end_line) in &rows {
        out.push_str(&format!("{path}:{line} {kind}\n"));
        if !cached.as_ref().is_some_and(|(p, _)| p == path) {
            symbol_src_reads_add(1);
            cached = Some((
                path.clone(),
                std::fs::read_to_string(root.join(path)).unwrap_or_default(),
            ));
        }
        let src = &cached.as_ref().unwrap().1;
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
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        return lsp::callers(cx, root, name);
    }
    index_for(cx, root)?;
    let rows = cx.symbol_ref_groups(&index::canon(root), name)?;
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
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        return lsp::impact(cx, root, name, depth);
    }
    index_for(cx, root)?;
    let rows = cx.symbol_impact(&index::canon(root), name, depth)?;
    if rows.is_empty() {
        return Ok(format!("nothing reaches {name}"));
    }
    let mut out = String::new();
    for (d, path, scope) in rows {
        if scope.is_empty() {
            out.push_str(&format!("{d}  {path}  (file)\n"));
        } else {
            out.push_str(&format!("{d}  {path}  {scope}\n"));
        }
    }
    cap(cx, out)
}

/// T8.7 BFS, kept as the T8.14 baseline. Not used on the tool path after T8.13.
#[cfg(test)]
pub(crate) fn impact_bfs(
    store: &crate::store::Store,
    root: &str,
    name: &str,
    depth: u32,
) -> Result<Vec<(u32, String, String)>> {
    let mut seen: HashSet<String> = HashSet::from([name.to_string()]);
    let mut frontier = vec![name.to_string()];
    let mut out = Vec::new();
    for d in 1..=depth.clamp(1, 4) {
        let mut next = Vec::new();
        for from in &frontier {
            for (path, scope, ..) in store.symbol_ref_groups(root, from)? {
                if scope.is_empty() {
                    out.push((d, path, String::new()));
                } else if seen.insert(scope.clone()) {
                    out.push((d, path, scope.clone()));
                    next.push(scope);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
fn symbol_src_reads_add(n: usize) {
    SYMBOL_SRC_READS.with(|c| c.fetch_add(n, Ordering::Relaxed));
}

#[cfg(not(test))]
fn symbol_src_reads_add(_n: usize) {}

/// `outline(path)`: the `read` plugin's `map` mode, capped like the other two.
pub fn outline(cx: &Ctx, path: &str) -> Result<String> {
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        return lsp::outline(cx, &root, path);
    }
    let text = crate::plugins::read::read(cx, path, "map", None)?;
    cap(cx, text)
}

/// Cap at `plugins.graph.max_tokens`: whole head lines that fit, then `N more, expand <id>`.
/// Always records one measurement (capped vs uncapped estimate); `ref_id` when truncated.
fn cap(cx: &Ctx, text: String) -> Result<String> {
    let max = cx.plugin_config::<crate::config::Graph>("graph").max_tokens;
    let est = cx.estimate(&text, Class::Code);
    let before_bytes = text.len() as u64;
    let (out, ref_id) = if est <= max {
        (text, None)
    } else {
        let id = cx.put_archive(text.as_bytes())?;
        // The estimator is linear in chars, so the char budget scales the same way;
        // leave room for the trailer line (count + a 64-hex archive id).
        let text_chars = text.chars().count();
        let budget_chars = (text_chars * max as usize / est as usize).saturating_sub(120);
        let total = text.lines().count();
        let mut head = String::new();
        let mut shown = 0;
        for line in text.lines() {
            if shown > 0 && head.chars().count() + line.chars().count() + 1 > budget_chars {
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
        call_id: cx.call_id(),
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::index::tests::cx;
    use super::*;
    use rstest::rstest;
    use std::fs;

    fn crate_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// T8.6: one call gives the definition and its source. The `cap` body is read back
    /// verbatim from the file, so the test cannot pass on a stale or paraphrased index.
    #[test]
    fn symbol_returns_the_definition_body() {
        let (cx, dir) = cx("body");
        let out = symbol(&Ctx::new(&cx), &crate_root(), "cap").unwrap();
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
        let out = symbol(&Ctx::new(&cx), &dir, "dup").unwrap();
        let trailer = out.lines().last().unwrap();
        assert!(trailer.contains(" more, expand "), "{trailer}");
        let id = trailer.rsplit(' ').next().unwrap();
        let full = String::from_utf8(cx.store.get_archive(id, None).unwrap().unwrap()).unwrap();
        assert_eq!(
            full.lines().filter(|l| l.starts_with("fn dup()")).count(),
            500,
            "the archive holds every definition"
        );
        assert!(cx.estimate(&out, Class::Code) <= cx.config.plugins.graph.max_tokens);
        assert_eq!(cx.store.measurement_count("graph").unwrap(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// T36.16: one source file, many definitions — read it once, not once per row.
    #[rstest]
    fn symbol_reads_each_source_file_once() {
        SYMBOL_SRC_READS.with(|c| c.store(0, Ordering::Relaxed));
        let (cx, dir) = cx("symread");
        let src = "fn dup() {\n    ();\n}\n".repeat(500);
        fs::write(dir.join("d.rs"), &src).unwrap();
        symbol(&Ctx::new(&cx), &dir, "dup").unwrap();
        let reads = SYMBOL_SRC_READS.with(|c| c.load(Ordering::Relaxed));
        assert_eq!(reads, 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// T36.16: cap scales in chars so CJK-heavy output cannot overshoot `max_tokens`.
    #[rstest]
    fn cjk_capped_output_respects_max_tokens() {
        let (cx, dir) = cx("cjkcap");
        let body = "// 漢字漢字漢字漢字漢字漢字漢字漢字漢字漢字\n".repeat(3000);
        fs::write(dir.join("cjk.rs"), format!("fn cjk() {{\n{body}}}")).unwrap();
        let out = symbol(&Ctx::new(&cx), &dir, "cjk").unwrap();
        assert!(cx.estimate(&out, Class::Code) <= cx.config.plugins.graph.max_tokens);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn symbol_main_is_in_src_main_rs() {
        let (cx, dir) = cx("symbol");
        let out = symbol(&Ctx::new(&cx), &crate_root(), "main").unwrap();
        assert!(out.lines().any(|l| l.starts_with("src/main.rs:")), "{out}");
        assert_eq!(
            symbol(&Ctx::new(&cx), &crate_root(), "no_such_fn").unwrap(),
            "no definition of no_such_fn"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.5: the caller is a definition, not a line number. `Runtime::estimate` calls the free
    /// `tokens::estimate`, so `src/plugin.rs` must report `estimate` as the calling scope.
    #[test]
    fn callers_estimate_lists_src_plugin_rs() {
        let (cx, dir) = cx("callers");
        let out = callers(&Ctx::new(&cx), &crate_root(), "estimate").unwrap();
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
        let out = callers(&Ctx::new(&cx), &dir, "zeta").unwrap();
        let trailer = out.lines().last().unwrap();
        assert!(trailer.contains(" more, expand "), "{trailer}");
        let id = trailer.rsplit(' ').next().unwrap();
        let full = cx
            .store
            .get_archive(id, None)
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
        let two = impact(&Ctx::new(&cx), &dir, "c", 2).unwrap();
        assert_eq!(at(&two, "b").as_deref(), Some("1"), "{two}");
        assert_eq!(at(&two, "a").as_deref(), Some("2"), "{two}");
        let one = impact(&Ctx::new(&cx), &dir, "c", 1).unwrap();
        assert_eq!(at(&one, "b").as_deref(), Some("1"), "{one}");
        assert_eq!(at(&one, "a"), None, "depth 1 must stop at the callers");
        // x calls y, y calls x, both reach c: the walk visits each once and returns.
        let deep = impact(&Ctx::new(&cx), &dir, "c", 4).unwrap();
        assert_eq!(
            deep.lines().filter(|l| l.ends_with("  x")).count(),
            1,
            "{deep}"
        );
        assert_eq!(
            impact(&Ctx::new(&cx), &dir, "no_such_fn", 2).unwrap(),
            "nothing reaches no_such_fn"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.13: CTE / path query and the BFS return the same (depth, path, scope) set.
    #[test]
    fn impact_fanout_matches_bfs() {
        let (cx, dir) = cx("fanout");
        let mut src = String::from("fn sink() {}\n");
        for i in 0..10 {
            src.push_str(&format!("fn a{i}() {{ sink(); }}\n"));
            src.push_str(&format!("fn b{i}() {{ a{i}(); }}\n"));
            src.push_str(&format!("fn c{i}() {{ b{i}(); }}\n"));
            src.push_str(&format!("fn d{i}() {{ c{i}(); }}\n"));
        }
        fs::write(dir.join("fan.rs"), src).unwrap();
        index::run(&Ctx::new(&cx), &dir, false).unwrap();
        let key = index::canon(&dir);
        let mut cte: Vec<_> = cx.store.symbol_impact(&key, "sink", 4).unwrap();
        let mut bfs = impact_bfs(&cx.store, &key, "sink", 4).unwrap();
        cte.sort();
        bfs.sort();
        assert!(!cte.is_empty(), "fan-out-10 must reach sink");
        assert_eq!(cte, bfs, "query vs BFS");
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.15: with `auto_index = false` an edit is invisible until `rtok graph index`
    /// (which is `index::run`). The call itself opens no file.
    #[test]
    fn auto_index_false_is_stale_until_explicit_index() {
        let (mut cx, dir) = cx("noauto");
        cx.config.plugins.graph.auto_index = false;
        fs::write(dir.join("a.rs"), "fn alpha() {}\n").unwrap();
        let first = symbol(&Ctx::new(&cx), &dir, "alpha").unwrap();
        assert!(first.contains("a.rs:1"), "{first}");
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(dir.join("a.rs"), "// bump\nfn alpha() {}\n").unwrap();
        let stale = symbol(&Ctx::new(&cx), &dir, "alpha").unwrap();
        assert!(
            stale.contains("a.rs:1"),
            "must still report the old line: {stale}"
        );
        let r = index_for(&Ctx::new(&cx), &dir).unwrap();
        assert_eq!(r.read, 0, "the call must open no file");
        index::run(&Ctx::new(&cx), &dir, false).unwrap(); // what `rtok graph index` does
        let fresh = symbol(&Ctx::new(&cx), &dir, "alpha").unwrap();
        assert!(
            fresh.contains("a.rs:2"),
            "explicit index shows the new line: {fresh}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T8.15: a root with no rows is still indexed once with `auto_index = false`.
    #[test]
    fn auto_index_false_empty_root_still_answers() {
        let (mut cx, dir) = cx("noauto-empty");
        cx.config.plugins.graph.auto_index = false;
        fs::write(dir.join("main.rs"), "fn main() {}\n").unwrap();
        let out = symbol(&Ctx::new(&cx), &dir, "main").unwrap();
        assert!(out.contains("main.rs:1"), "{out}");
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
        let out = outline(&Ctx::new(&cx), "src/main.rs").unwrap();
        assert!(out.contains("main"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }
}
