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

use std::collections::{BTreeSet, HashMap, HashSet};
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
pub mod walk;
pub mod status;

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
        let named_path = json!({"type":"object","properties":{"name":{"type":"string"},"path":{"type":"string"}},"required":["name"]});
        vec![
            ToolDef {
                name: "symbol",
                description: "Definitions of a symbol with their source: path:line kind, then the body. Optional path substring and kind narrow the match.",
                input_schema: json!({"type":"object","properties":{"name":{"type":"string"},"path":{"type":"string"},"kind":{"type":"string"}},"required":["name"]}),
            },
            ToolDef {
                name: "callers",
                description: "Which definitions reference a symbol: path, calling definition, count. Optional path substring keeps one subtree.",
                input_schema: named_path,
            },
            ToolDef {
                name: "impact",
                description: "What breaks if a symbol changes: callers up to depth. Optional to: chains reaching it. Empty name + path lists affected tests.",
                input_schema: json!({"type":"object","properties":{"name":{"type":"string"},"to":{"type":"string"},"depth":{"type":"integer"},"path":{"type":"string"}}}),
            },
            ToolDef {
                name: "outline",
                description: "Definitions in one file (read mode=map).",
                input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
            },
            ToolDef {
                name: "explore",
                description: "Answers a code question: the query's symbols as definitions with bodies, call paths between them, impact counts. Optional path narrows.",
                input_schema: json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string"}},"required":["query"]}),
            },
        ]
    }
}

/// T52.1: optional narrow-down for `symbol` / `callers` / `impact` (`path`
/// substring, `kind` exact on `symbol`). No new tool: the filters answer
/// "callers of X inside path Y of kind Z" in the one call that transcripts now
/// spend a scoped-search chain on (610 of 615 `ctx_search` calls carry a path;
/// 252 search-to-search refinements in 33 sessions; research.md §2). `callers`
/// and `impact` honour `path` only. `impact` walks the full graph and filters
/// the reported lines, so depth still crosses files outside the filter.
pub struct Filter {
    pub path: String,
    pub kind: String,
}

impl Filter {
    pub fn none() -> Self {
        Self {
            path: String::new(),
            kind: String::new(),
        }
    }

    pub(crate) fn path_ok(&self, path: &str) -> bool {
        self.path.is_empty() || path.contains(&self.path)
    }

    pub(crate) fn kind_ok(&self, kind: &str) -> bool {
        self.kind.is_empty() || kind == self.kind
    }

    /// ` in <path>` / ` of kind <kind>` suffix for the empty-answer lines; empty
    /// when no filter is set, so unfiltered answers stay byte-exact (T8.9).
    pub(crate) fn scope_note(&self) -> String {
        let mut s = String::new();
        if !self.path.is_empty() {
            s.push_str(&format!(" in {}", self.path));
        }
        if !self.kind.is_empty() {
            s.push_str(&format!(" of kind {}", self.kind));
        }
        s
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


/// Pending re-index paths: hook marks, stat drift and the watcher queue (T68.3).
pub(crate) fn pending_paths(cx: &Ctx, root: &Path) -> Result<Vec<String>> {
    let key = index::canon(root);
    let mut set: HashSet<String> = cx.symbol_pending(&key, root)?.into_iter().collect();
    for p in cx.graph_watch_pending() {
        set.insert(p);
    }
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort();
    Ok(out)
}

fn stale_banner(cx: &Ctx, root: &Path) -> Result<String> {
    let cfg = cx.plugin_config::<crate::config::Graph>("graph");
    let pending = pending_paths(cx, root)?;
    if pending.is_empty() {
        return Ok(String::new());
    }
    let watch_pending = !cx.graph_watch_pending().is_empty() && cfg.watch != "off";
    if !cfg.auto_index || watch_pending {
        let show = pending.len().min(5);
        let listed = pending[..show].join(", ");
        let tail = if pending.len() > 5 { ", …" } else { "" };
        return Ok(format!("stale: {} files pending ({}{})
", pending.len(), listed, tail));
    }
    Ok(String::new())
}

pub(crate) fn with_stale(cx: &Ctx, root: &Path, text: String) -> Result<String> {
    let banner = stale_banner(cx, root)?;
    if banner.is_empty() {
        Ok(text)
    } else {
        Ok(format!("{banner}{text}"))
    }
}

/// MCP dispatch for the four tools (`mcp.rs` `invoke`). An `Err` becomes an `isError` result.
pub fn call(cx: &Ctx, name: &str, args: &Value) -> Result<String> {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let arg = |k: &str| args[k].as_str().unwrap_or("");
    let filter = Filter {
        path: arg("path").to_string(),
        kind: arg("kind").to_string(),
    };
    match name {
        "symbol" => symbol_filtered(cx, &root, arg("name"), &filter),
        "callers" => callers_filtered(cx, &root, arg("name"), &filter),
        "impact" => {
            let name = arg("name");
            if name.is_empty() {
                let path = arg("path");
                let paths = if path.is_empty() {
                    Vec::new()
                } else {
                    vec![rel_of(&root, path)]
                };
                affected_from_paths(
                    cx,
                    &root,
                    &paths,
                    args["depth"].as_u64().unwrap_or(3) as u32,
                    false,
                )
            } else {
                impact_filtered(
                    cx,
                    &root,
                    name,
                    args["depth"].as_u64().unwrap_or(2) as u32,
                    &filter,
                    args["to"].as_str(),
                )
            }
        }
        "outline" => outline(cx, arg("path")),
        "explore" => explore(cx, &root, arg("query"), &filter),
        _ => anyhow::bail!("unknown tool: {name}"),
    }
}

/// `symbol(name)`: `path:line kind` per definition, then that definition's source from
/// `line` to `end_line`, at most `plugins.graph.body_lines` lines each (T8.6). One call
/// answers "what is this and what does it do", which took a `symbol` plus a `read` at v0.1.
pub fn symbol(cx: &Ctx, root: &Path, name: &str) -> Result<String> {
    symbol_filtered(cx, root, name, &Filter::none())
}

/// Filtered `symbol`: a non-empty `filter` keeps only matching definitions (T52.1).
pub fn symbol_filtered(cx: &Ctx, root: &Path, name: &str, filter: &Filter) -> Result<String> {
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        return lsp::symbol(cx, root, name, filter);
    }
    index_for(cx, root)?;
    let rows: Vec<_> = cx
        .symbol_defs(&index::canon(root), name)?
        .into_iter()
        .filter(|(path, kind, ..)| filter.path_ok(path) && filter.kind_ok(kind))
        .collect();
    if rows.is_empty() {
        return with_stale(cx, root, format!("no definition of {name}{}", filter.scope_note()));
    }
    let key = index::canon(root);
    let callees = cx.symbol_callees(&key, name)?;
    with_stale(cx, root, cap(cx, defs_text(cx, root, &rows, &callees))?)
}

/// `{path}:{line} {kind}` per definition, then that definition's source, at most
/// `plugins.graph.body_lines` lines each (T8.6). Shared by `symbol` and `explore`
/// (T68.1) so both print a definition the same way; reads each source file once.
fn calls_line(names: &[String], cap: usize) -> String {
    if names.is_empty() {
        return String::new();
    }
    let show = names.len().min(cap);
    let listed = names[..show].join(", ");
    let extra = names.len() - show;
    if extra > 0 {
        format!("calls: {listed} (+{extra})\n")
    } else {
        format!("calls: {listed}\n")
    }
}

fn ambiguous_banner(n: usize) -> String {
    format!("{n} names ambiguous (?): narrow with path or kind, or backend = \"lsp\"\n")
}

fn mark_ambiguous_lines(out: &str) -> String {
    if out.is_empty() {
        return String::new();
    }
    out.lines().map(|line| format!("{line} ?")).collect::<Vec<_>>().join("\n") + "\n"
}

fn annotate_ambiguous(cx: &Ctx, root: &Path, name: &str, out: String) -> Result<String> {
    if cx.symbol_defs(&index::canon(root), name)?.len() > 1 {
        Ok(format!("{}{}", ambiguous_banner(1), mark_ambiguous_lines(&out)))
    } else {
        Ok(out)
    }
}

fn defs_text(
    cx: &Ctx,
    root: &Path,
    rows: &[(String, String, i32, i32)],
    callees: &[(String, i32, String, i32)],
) -> String {
    let budget = cx.plugin_config::<crate::config::Graph>("graph").body_lines as usize;
    let cap = budget / 2;
    let mut by_def: HashMap<(String, i32), Vec<String>> = HashMap::new();
    for (path, line, callee, _) in callees {
        by_def.entry((path.clone(), *line)).or_default().push(callee.clone());
    }
    let mut out = String::new();
    let mut cached: Option<(String, String)> = None;
    for (path, kind, line, end_line) in rows {
        out.push_str(&format!("{path}:{line} {kind}\n"));
        if !cached.as_ref().is_some_and(|(p, _)| p == path) {
            symbol_src_reads_add(1);
            cached = Some((path.clone(), std::fs::read_to_string(root.join(path)).unwrap_or_default()));
        }
        out.push_str(&body_lines(&cached.as_ref().unwrap().1, *line, *end_line, budget));
        if let Some(names) = by_def.get(&(path.clone(), *line)) {
            out.push_str(&calls_line(names, cap));
        }
    }
    out
}

/// Source of one definition, `line..=end_line`, at most `budget` lines then `N more lines`.
/// Shared with the LSP backend so both print a body the same way.
pub(crate) fn body_lines(src: &str, line: i32, end_line: i32, budget: usize) -> String {
    let first = line.max(1) as usize - 1;
    let last = end_line.max(line) as usize;
    let body: Vec<&str> = src.lines().skip(first).take(last - first).collect();
    let mut out = String::new();
    for l in body.iter().take(budget) {
        out.push_str(l);
        out.push('\n');
    }
    if body.len() > budget {
        out.push_str(&format!("  … {} more lines\n", body.len() - budget));
    }
    out
}

/// `callers(name)`: one line per calling definition, `path  scope xN (Lline)` (T8.5).
/// v0.1 printed every site with its source line; the edge is what the caller needs, and it
/// costs a fraction of the bytes.
pub fn callers(cx: &Ctx, root: &Path, name: &str) -> Result<String> {
    callers_filtered(cx, root, name, &Filter::none())
}

/// Filtered `callers`: a non-empty `filter.path` keeps one subtree (T52.1).
pub fn callers_filtered(cx: &Ctx, root: &Path, name: &str, filter: &Filter) -> Result<String> {
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        return lsp::callers(cx, root, name, filter);
    }
    index_for(cx, root)?;
    let rows: Vec<_> = cx
        .symbol_ref_groups(&index::canon(root), name)?
        .into_iter()
        .filter(|(path, ..)| filter.path_ok(path))
        .collect();
    if rows.is_empty() {
        return with_stale(cx, root, annotate_ambiguous(cx, root, name, format!("no references to {name}{}", filter.scope_note()))?);
    }
    let mut out = String::new();
    for (path, scope, n, line) in rows {
        let scope = if scope.is_empty() { String::new() } else { format!("  {scope}") };
        out.push_str(&format!("{path}{scope} ×{n} (L{line})\n"));
    }
    with_stale(cx, root, cap(cx, annotate_ambiguous(cx, root, name, out)?)?)
}

/// `impact(name, depth)`: breadth-first walk of the `scope` edges T8.5 stored — who calls
/// `name`, who calls them, and so on (T8.7). One `depth  path  scope` line per definition
/// reached. A definition is expanded once, so a call cycle terminates.
pub fn impact(cx: &Ctx, root: &Path, name: &str, depth: u32, to: Option<&str>) -> Result<String> {
    impact_filtered(cx, root, name, depth, &Filter::none(), to)
}

/// Filtered `impact`: a non-empty `filter.path` keeps the reported lines in one
/// subtree; the walk itself still crosses files outside it, so depth is not cut
/// short (T52.1).
pub fn impact_filtered(
    cx: &Ctx,
    root: &Path,
    name: &str,
    depth: u32,
    filter: &Filter,
    to: Option<&str>,
) -> Result<String> {
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        return with_stale(cx, root, lsp::impact(cx, root, name, depth, filter, to)?);
    }
    index_for(cx, root)?;
    if let Some(target) = to.filter(|s| !s.is_empty()) {
        let chains = cx
            .symbol_paths(&index::canon(root), target, name, depth)?
            .into_iter()
            .map(|c| reverse_call_chain(&c))
            .collect::<Vec<_>>();
        if chains.is_empty() {
            return with_stale(
                cx,
                root,
                format!("no path from {name} to {target} within depth {depth}"),
            );
        }
        let body = chains.join("
") + "
";
        return with_stale(cx, root, cap(cx, annotate_ambiguous(cx, root, name, body)?)?);
    }
    let rows: Vec<_> = cx
        .symbol_impact(&index::canon(root), name, depth)?
        .into_iter()
        .filter(|(_, path, _)| filter.path_ok(path))
        .collect();
    if rows.is_empty() {
        return with_stale(
            cx,
            root,
            annotate_ambiguous(cx, root, name, format!("nothing reaches {name}{}", filter.scope_note()))?,
        );
    }
    with_stale(cx, root, cap(cx, annotate_ambiguous(cx, root, name, impact_lines_text(&rows))?)?)
}

/// `symbol_paths` walks callers; `impact --to` prints callee chains, so reverse the arrows.
fn reverse_call_chain(chain: &str) -> String {
    let parts: Vec<&str> = chain.split(" → ").collect();
    parts.into_iter().rev().collect::<Vec<_>>().join(" → ")
}

/// One `depth  path  scope` line per row, `(file)` when the row is file-level.
/// Shared by `impact` and `explore` (T68.1) so both print a walk the same way.
pub(crate) fn impact_lines_text(rows: &[(u32, String, String)]) -> String {
    let mut out = String::new();
    for (d, path, scope) in rows {
        if scope.is_empty() {
            out.push_str(&format!("{d}  {path}  (file)\n"));
        } else {
            out.push_str(&format!("{d}  {path}  {scope}\n"));
        }
    }
    out
}

/// `dead()`: unreferenced private definitions as `path:line kind name` lines (T52.4).
/// Drops pub items, methods in trait impls/trait bodies, test files and `#[test]`
/// fns, `macro` definitions and `main`.
pub fn dead(cx: &Ctx, root: &Path) -> Result<String> {
    index_for(cx, root)?;
    let key = index::canon(root);
    let mut files: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut out = String::new();
    for (path, name, kind, line) in cx.symbol_dead_candidates(&key)? {
        if kind == "macro" || name == "main" || is_test_path(&path) {
            continue;
        }
        let src = files
            .entry(path.clone())
            .or_insert_with(|| {
                std::fs::read_to_string(root.join(&path))
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_string)
                    .collect()
            })
            .clone();
        let def = src
            .get(line.max(1) as usize - 1)
            .map(String::as_str)
            .unwrap_or("");
        let trimmed = def.trim_start();
        if trimmed.starts_with("pub ") || trimmed.starts_with("pub(") || trimmed == "pub" {
            continue;
        }
        if src[..(line.max(1) as usize - 1).min(src.len())]
            .iter()
            .rev()
            .take(3)
            .any(|l| {
                let t = l.trim_start();
                t.starts_with("#[test") || t.starts_with("#[cfg(test")
            })
        {
            continue;
        }
        #[cfg(feature = "lang-rust")]
        if path.ends_with(".rs") {
            let (ranges, types) = rust_impls(&src.join("\n"));
            // Named as an `impl` target (`impl S`, `impl T for S`): used.
            if types.iter().any(|t| t == &name) {
                continue;
            }
            if kind == "method"
                && ranges
                    .iter()
                    .any(|(s, e)| *s <= line as usize && line as usize <= *e)
            {
                continue;
            }
        }
        out.push_str(&format!("{path}:{line} {kind} {name}\n"));
    }
    if out.is_empty() {
        return Ok(format!("no dead code in {}", root.display()));
    }
    cap(cx, out)
}

/// Test files by path: `tests/` dirs and test-named files. Test-only bodies
/// (`#[test]`) are filtered at the call site from the source lines.
fn is_test_path(path: &str) -> bool {
    path == "tests"
        || path.starts_with("tests/")
        || path.contains("/tests/")
        || path.split('/').next_back().is_some_and(|f| {
            f.starts_with("test_") || f.starts_with("_test") || f.contains("_test.")
        })
}

/// 1-based line ranges of `impl X for Y` blocks and `trait` bodies in Rust source
/// (methods there are interface surface, not dead code), plus every `impl` target
/// type name (`impl S`, `impl T for S` — the tags query records no reference for
/// the type of a trait impl, so `S` would otherwise read as dead).
#[cfg(feature = "lang-rust")]
fn rust_impls(src: &str) -> (Vec<(usize, usize)>, Vec<String>) {
    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    if parser.set_language(&lang).is_err() {
        return (Vec::new(), Vec::new());
    }
    let Some(tree) = parser.parse(src, None) else {
        return (Vec::new(), Vec::new());
    };
    let mut out = (Vec::new(), Vec::new());
    collect_impls(tree.root_node(), src.as_bytes(), &mut out);
    out
}

#[cfg(feature = "lang-rust")]
fn collect_impls(
    node: tree_sitter::Node<'_>,
    src: &[u8],
    out: &mut (Vec<(usize, usize)>, Vec<String>),
) {
    if node.kind() == "impl_item" {
        if let Some(t) = node.child_by_field_name("type")
            && let Ok(name) = t.utf8_text(src)
        {
            // Generics read as `S<T>` and never equal a definition name.
            out.1
                .push(name.split('<').next().unwrap_or(name).trim().to_string());
        }
        if node.child_by_field_name("trait").is_some() {
            out.0
                .push((node.start_position().row + 1, node.end_position().row + 1));
        }
    }
    if node.kind() == "trait_item" {
        out.0
            .push((node.start_position().row + 1, node.end_position().row + 1));
    }
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    for child in children {
        collect_impls(child, src, out);
    }
}
const EMPTY_AFFECTED: &str = "no indexed test reaches the change; run the suite";

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

/// Tests that reach files changed in git (`git diff --name-only`). No Measurement.
pub fn affected(
    cx: &Ctx,
    root: &Path,
    since: Option<&str>,
    staged: bool,
    json: bool,
) -> Result<String> {
    affected_from_paths(cx, root, &git_changed_files(root, since, staged), 3, json)
}

pub(crate) fn affected_from_paths(
    cx: &Ctx,
    root: &Path,
    paths: &[String],
    depth: u32,
    json: bool,
) -> Result<String> {
    index_for(cx, root)?;
    let key = index::canon(root);
    let mut hits = BTreeSet::new();
    let mut starts = HashSet::new();
    for raw in paths {
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
    Ok(format_affected(&hits, json))
}

fn defs_in_path(cx: &Ctx, root: &Path, key: &str, rel: &str) -> Result<Vec<String>> {
    let abs = root.join(rel);
    let src = std::fs::read_to_string(&abs).unwrap_or_default();
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for hit in crate::plugins::read::outline::tags(&abs, &src)? {
        if hit.is_def
            && seen.insert(hit.name.clone())
            && cx
                .symbol_defs(key, &hit.name)?
                .iter()
                .any(|(p, ..)| p == rel)
        {
            names.push(hit.name);
        }
    }
    Ok(names)
}

fn git_changed_files(root: &Path, since: Option<&str>, staged: bool) -> Vec<String> {
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

fn format_affected(hits: &BTreeSet<(String, String)>, json: bool) -> String {
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
            json!({"tests": [], "message": EMPTY_AFFECTED}).to_string()
        } else {
            json!({"tests": tests}).to_string()
        };
    }
    if hits.is_empty() {
        return EMPTY_AFFECTED.to_string();
    }
    let mut out = String::new();
    for (file, symbol) in hits {
        out.push_str(&format!("{file} ← via {symbol}\n"));
        if let Some(cmd) = test_command(file, symbol) {
            out.push_str(&format!("{cmd}\n"));
        }
    }
    out
}

/// T8.7 BFS (T8.14 baseline). T68.5 walks it from each changed file's definitions.
pub(crate) fn impact_bfs(
    cx: &Ctx,
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
            for (path, scope, ..) in cx.symbol_ref_groups(root, from)? {
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
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        let allow = &cx.plugin_config::<crate::config::Read>("read").allow_paths;
        let abs = crate::plugins::read::resolve(&root, Path::new(path), allow)?;
        return with_stale(cx, &root, lsp::outline(cx, &root, &abs.to_string_lossy())?);
    }
    let text = crate::plugins::read::read(cx, path, "map", None)?;
    with_stale(cx, &root, cap(cx, text)?)
}

// ---------- T68.1: explore ----------

/// Identifier tokens of a free-text question: alphanumeric/`_` runs, deduped,
/// first 8 — single letters are real identifiers (`b`, `c`, `x`), so nothing but
/// empty runs are dropped; the cap keeps the resolution and the pairwise path
/// walk below fast.
fn explore_tokens(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .filter(|t| seen.insert((*t).to_string()))
        .take(8)
        .map(str::to_string)
        .collect()
}

/// At most this many distinct names land in one answer: pairwise paths are
/// `O(names²)` walks and the answer is capped anyway.
const EXPLORE_MAX_NAMES: usize = 10;

/// The backend pieces `explore` assembles its answer from (T68.1). Both backends —
/// tree-sitter-tags (`TagsExplore`) and LSP (`lsp::LspExplore`) — answer the same
/// four queries, so one assembler produces one answer shape.
pub(crate) trait ExploreParts {
    /// Exact-then-prefix resolution of one query token to definition names (best 5).
    fn resolve(&mut self, token: &str) -> Result<Vec<String>>;
    /// `{path}:{line} {kind}` + body lines for every definition of `name`.
    fn defs(&mut self, name: &str) -> Result<String>;
    /// Call chains `a → … → b` in the caller direction, at most 3 hops.
    fn paths(&mut self, a: &str, b: &str) -> Result<Vec<String>>;
    /// What `impact(name, 1)` would print, and the row count behind it.
    fn impact1(&mut self, name: &str) -> Result<(String, usize)>;
    fn def_count(&mut self, name: &str) -> Result<usize>;
}

/// One assembled `explore` answer plus the bytes the separate calls it replaces
/// would have returned (`symbol` + `impact` per name) — the Measurement's `before`.
pub(crate) fn assemble_explore(
    query: &str,
    filter: &Filter,
    parts: &mut dyn ExploreParts,
) -> Result<(String, u64)> {
    let mut names: Vec<String> = Vec::new();
    for token in explore_tokens(query) {
        for name in parts.resolve(&token)? {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if names.len() >= EXPLORE_MAX_NAMES {
            break;
        }
    }
    if names.is_empty() {
        return Ok((
            format!("no symbols resolved for \"{query}\"{}", filter.scope_note()),
            0,
        ));
    }
    let mut counts = HashMap::new();
    for name in &names {
        counts.insert(name.clone(), parts.def_count(name)?);
    }
    let ambiguous = counts.values().filter(|c| **c > 1).count();
    let mut out = String::new();
    if ambiguous > 0 {
        out.push_str(&ambiguous_banner(ambiguous));
    }
    let mut before = 0u64;
    let mut impact = Vec::new();
    for name in &names {
        let defs = parts.defs(name)?;
        before += defs.len() as u64;
        out.push_str(&format!("= {name}\n{defs}"));
        let (text, n) = parts.impact1(name)?;
        before += text.len() as u64;
        let mark = if counts[name] > 1 { " ?" } else { "" };
        impact.push(format!("{name} ← {n}{mark}"));
    }
    out.push_str("paths:\n");
    let mut any = false;
    for a in &names {
        for b in &names {
            if a != b {
                for chain in parts.paths(a, b)? {
                    out.push_str(&chain);
                    out.push('\n');
                    any = true;
                }
            }
        }
    }
    if !any {
        out.push_str("none\n");
    }
    out.push_str("impact:\n");
    for line in impact {
        out.push_str(&line);
        out.push('\n');
    }
    Ok((out, before))
}

/// `explore(query, path?)` (T68.1): one call answers a code question the way
/// `symbol` + `impact` + caller-walking did in three to five. The query splits
/// into identifier tokens; each resolves exactly, else by prefix best-5 by
/// reference count. The answer prints every definition body once per file, the
/// call paths between the resolved symbols (`symbol_paths`, ≤ 3 hops) and one
/// impact depth-1 line per symbol, then goes through `cap` like the other tools.
pub fn explore(cx: &Ctx, root: &Path, query: &str, filter: &Filter) -> Result<String> {
    if cx.plugin_config::<crate::config::Graph>("graph").backend == "lsp" {
        return lsp::explore(cx, root, query, filter);
    }
    index_for(cx, root)?;
    let mut parts = TagsExplore {
        cx,
        root,
        filter,
        key: index::canon(root),
    };
    let (text, before) = assemble_explore(query, filter, &mut parts)?;
    with_stale(cx, root, cap_kind(cx, text, before, "explore")?)
}

/// The tree-sitter-tags backend's pieces: every query is answered from the indexed
/// rows; definition bodies are read from disk the way `symbol` reads them.
struct TagsExplore<'a> {
    cx: &'a Ctx<'a>,
    root: &'a Path,
    filter: &'a Filter,
    key: String,
}

impl ExploreParts for TagsExplore<'_> {
    fn resolve(&mut self, token: &str) -> Result<Vec<String>> {
        if !self.cx.symbol_defs(&self.key, token)?.is_empty() {
            return Ok(vec![token.to_string()]);
        }
        self.cx.symbol_name_prefix(&self.key, token, 5)
    }

    fn defs(&mut self, name: &str) -> Result<String> {
        let rows: Vec<_> = self
            .cx
            .symbol_defs(&self.key, name)?
            .into_iter()
            .filter(|(path, ..)| self.filter.path_ok(path))
            .collect();
        if rows.is_empty() {
            return Ok(format!(
                "no definition of {name}{}\n",
                self.filter.scope_note()
            ));
        }
        let callees = self.cx.symbol_callees(&self.key, name)?;
        Ok(defs_text(self.cx, self.root, &rows, &callees))
    }

    fn def_count(&mut self, name: &str) -> Result<usize> {
        Ok(self.cx.symbol_defs(&self.key, name)?.len())
    }

    fn paths(&mut self, a: &str, b: &str) -> Result<Vec<String>> {
        self.cx.symbol_paths(&self.key, a, b, 3)
    }

    fn impact1(&mut self, name: &str) -> Result<(String, usize)> {
        let rows: Vec<_> = self
            .cx
            .symbol_impact(&self.key, name, 1)?
            .into_iter()
            .filter(|(_, path, _)| self.filter.path_ok(path))
            .collect();
        if rows.is_empty() {
            return Ok((
                format!("nothing reaches {name}{}", self.filter.scope_note()),
                0,
            ));
        }
        let n = rows.len();
        Ok((impact_lines_text(&rows), n))
    }
}

/// Cap at `plugins.graph.max_tokens`: whole head lines that fit, then `N more, expand <id>`.
/// Always records one measurement; `ref_id` when truncated. `before_bytes` is the
/// uncapped text for the four plain tools, and the sum of the calls `explore`
/// replaced for `kind = "explore"` (T68.1).
fn cap_kind(cx: &Ctx, text: String, before_bytes: u64, kind: &'static str) -> Result<String> {
    let max = cx.plugin_config::<crate::config::Graph>("graph").max_tokens;
    let est = cx.estimate(&text, Class::Code);
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
        kind,
        before_bytes,
        after_bytes: out.len() as u64,
        est_before: est,
        est_after: cx.estimate(&out, Class::Code),
        ref_id,
        call_id: cx.call_id(),
    })?;
    Ok(out)
}

fn cap(cx: &Ctx, text: String) -> Result<String> {
    let before = text.len() as u64;
    cap_kind(cx, text, before, "cap")
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
        let two = impact(&Ctx::new(&cx), &dir, "c", 2, None).unwrap();
        assert_eq!(at(&two, "b").as_deref(), Some("1"), "{two}");
        assert_eq!(at(&two, "a").as_deref(), Some("2"), "{two}");
        let one = impact(&Ctx::new(&cx), &dir, "c", 1, None).unwrap();
        assert_eq!(at(&one, "b").as_deref(), Some("1"), "{one}");
        assert_eq!(at(&one, "a"), None, "depth 1 must stop at the callers");
        // x calls y, y calls x, both reach c: the walk visits each once and returns.
        let deep = impact(&Ctx::new(&cx), &dir, "c", 4, None).unwrap();
        assert_eq!(
            deep.lines().filter(|l| l.ends_with("  x")).count(),
            1,
            "{deep}"
        );
        assert_eq!(
            impact(&Ctx::new(&cx), &dir, "no_such_fn", 2, None).unwrap(),
            "nothing reaches no_such_fn"
        );
        let _ = fs::remove_dir_all(dir);
    }


    #[test]
    fn symbol_lists_callees_on_impact_fixture() {
        let (cx, dir) = cx("callees");
        fs::write(dir.join("chain.rs"), "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn c() {}\n").unwrap();
        let ctx = Ctx::new(&cx);
        assert!(symbol(&ctx, &dir, "a").unwrap().contains("calls: b"));
        assert!(symbol(&ctx, &dir, "b").unwrap().contains("calls: c"));
        assert!(!symbol(&ctx, &dir, "c").unwrap().contains("calls:"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn callers_and_impact_mark_ambiguous_names() {
        let (cx, dir) = cx("ambiguous");
        fs::write(dir.join("a.rs"), "fn new() {}\nfn alpha() { new(); }\n").unwrap();
        fs::write(dir.join("b.rs"), "fn new() {}\n").unwrap();
        let ctx = Ctx::new(&cx);
        assert!(!callers(&ctx, &dir, "alpha").unwrap().contains('?'));
        let new = callers(&ctx, &dir, "new").unwrap();
        assert!(new.starts_with("1 names ambiguous"));
        assert!(new.contains(" ?\n"));
        assert!(impact(&ctx, &dir, "new", 1, None).unwrap().starts_with("1 names ambiguous"));
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
        let mut bfs = impact_bfs(&Ctx::new(&cx), &key, "sink", 4).unwrap();
        cte.sort();
        bfs.sort();
        assert!(!cte.is_empty(), "fan-out-10 must reach sink");
        assert_eq!(cte, bfs, "query vs BFS");
        let _ = fs::remove_dir_all(dir);
    }

    /// T52.1: `path` keeps one subtree. One name defined in two files: the
    /// unfiltered answer lists both, the filtered one only the match, and a
    /// match-nothing filter names the scope in the empty answer.
    #[test]
    fn symbol_path_filter_keeps_one_file() {
        let (cx, dir) = cx("filter-path");
        fs::write(dir.join("a.rs"), "fn dup() {}\n").unwrap();
        fs::write(dir.join("b.rs"), "fn dup() {}\n").unwrap();
        let ctx = Ctx::new(&cx);
        let both = symbol_filtered(&ctx, &dir, "dup", &Filter::none()).unwrap();
        assert!(both.contains("a.rs") && both.contains("b.rs"), "{both}");
        let one = symbol_filtered(
            &ctx,
            &dir,
            "dup",
            &Filter {
                path: "b.rs".into(),
                kind: String::new(),
            },
        )
        .unwrap();
        assert!(one.contains("b.rs") && !one.contains("a.rs"), "{one}");
        assert_eq!(
            symbol_filtered(
                &ctx,
                &dir,
                "dup",
                &Filter {
                    path: "zzz".into(),
                    kind: String::new(),
                },
            )
            .unwrap(),
            "no definition of dup in zzz"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T52.1: `kind` keeps one definition kind. A struct and a function share a
    /// name (different namespaces, compiles); the filter keeps the asked kind.
    #[test]
    fn symbol_kind_filter_picks_struct_over_function() {
        let (cx, dir) = cx("filter-kind");
        fs::write(dir.join("k.rs"), "struct Shape;\nfn Shape() {}\n").unwrap();
        let ctx = Ctx::new(&cx);
        let out = symbol_filtered(
            &ctx,
            &dir,
            "Shape",
            &Filter {
                path: String::new(),
                kind: "struct".into(),
            },
        )
        .unwrap();
        assert!(out.contains("struct"), "{out}");
        assert!(!out.contains("fn Shape"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }

    /// T52.1: `callers` with a path keeps the callers in that subtree only.
    #[test]
    fn callers_path_filter_keeps_one_subtree() {
        let (cx, dir) = cx("filter-callers");
        fs::write(
            dir.join("chain.rs"),
            "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn c() {}\n",
        )
        .unwrap();
        fs::write(dir.join("other.rs"), "fn d() {\n    c();\n    c();\n}\n").unwrap();
        let ctx = Ctx::new(&cx);
        let one = callers_filtered(
            &ctx,
            &dir,
            "c",
            &Filter {
                path: "other".into(),
                kind: String::new(),
            },
        )
        .unwrap();
        assert!(
            one.contains("other.rs") && !one.contains("chain.rs"),
            "{one}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// T52.1: `impact` with a path reports only the lines in that subtree.
    #[test]
    fn impact_path_filter_reports_matching_lines() {
        let (cx, dir) = cx("filter-impact");
        fs::write(
            dir.join("chain.rs"),
            "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn c() {}\n",
        )
        .unwrap();
        fs::write(dir.join("other.rs"), "fn d() {\n    c();\n    c();\n}\n").unwrap();
        let ctx = Ctx::new(&cx);
        let out = impact_filtered(
            &ctx,
            &dir,
            "c",
            2,
            &Filter {
                path: "other".into(),
                kind: String::new(),
            },
            None,
        )
        .unwrap();
        assert_eq!(out, "1  other.rs  d\n", "{out}");
        assert_eq!(
            impact_filtered(
                &ctx,
                &dir,
                "c",
                2,
                &Filter {
                    path: "zzz".into(),
                    kind: String::new(),
                },
                None,
            )
            .unwrap(),
            "nothing reaches c in zzz"
        );
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
            stale.starts_with("stale:") && stale.contains("a.rs"),
            "pending edit must carry a staleness banner: {stale}"
        );
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

    /// Gate P8b: five tools, each description ≤ 60 tokens, the whole surface ≤ 150.
    #[test]
    fn graph_surface_is_five_tools_under_150_tokens() {
        let (cx, dir) = cx("surface");
        let tools = Graph.mcp_tools();
        assert_eq!(tools.len(), 5);
        let est = |d: &str| crate::tokens::estimate(d, Class::Prose, &cx.config.estimator);
        let n: u32 = tools.iter().map(|t| est(t.description)).sum();
        println!(
            "graph surface: {} tools, {n} description tokens",
            tools.len()
        );
        assert!(n <= 150, "graph descriptions are {n} tokens");
        for t in &tools {
            assert!(
                est(t.description) <= 60,
                "{} description is {} tokens",
                t.name,
                est(t.description)
            );
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn outline_reuses_read_map() {
        let (cx, dir) = cx("outline");
        let out = outline(&Ctx::new(&cx), "src/main.rs").unwrap();
        assert!(out.contains("main"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }

    /// T52.4: one truly-dead private fn is listed; pub API, trait-impl methods,
    /// `#[test]` fns, macro definitions and test-dir files are not.
    #[test]
    fn dead_lists_only_the_private_orphan() {
        let (cx, dir) = cx("dead");
        fs::write(
            dir.join("live.rs"),
            "pub fn caller() {\n    used();\n}\nfn used() {}\n",
        )
        .unwrap();
        fs::write(dir.join("dead.rs"), "fn orphan() {}\n").unwrap();
        fs::write(dir.join("api.rs"), "pub fn exported() {}\n").unwrap();
        fs::write(
            dir.join("traits.rs"),
            "struct S;\ntrait T {\n    fn m(&self);\n}\nimpl T for S {\n    fn m(&self) {}\n}\n",
        )
        .unwrap();
        fs::write(dir.join("mac.rs"), "macro_rules! gen {\n    () => {};\n}\n").unwrap();
        fs::write(dir.join("tested.rs"), "#[test]\nfn my_test() {}\n").unwrap();
        fs::create_dir_all(dir.join("tests")).unwrap();
        fs::write(dir.join("tests/helper.rs"), "fn help_me() {}\n").unwrap();
        let out = dead(&Ctx::new(&cx), &dir).unwrap();
        assert!(out.contains("orphan"), "{out}");
        for kept in [
            "used", "caller", "exported", "m", "gen", "my_test", "help_me", "T", "S",
        ] {
            assert!(
                !out.lines().any(|l| l.ends_with(&format!(" {kept}"))),
                "{kept} must not be listed as dead:\n{out}"
            );
        }
        let _ = fs::remove_dir_all(dir);
    }

    /// The guard fails before any language server is spawned, so no LSP binary is needed.
    #[test]
    fn lsp_outline_refuses_paths_outside_the_root() {
        let (mut c, dir) = crate::testutil::config("lsp-outside");
        c.plugins.graph.backend = "lsp".into();
        let cx = crate::plugin::Runtime::open(c, "lsp-outside").unwrap();
        for path in ["/etc/passwd", "../../../../../../etc/passwd"] {
            let err = outline(&Ctx::new(&cx), path).unwrap_err().to_string();
            assert!(err.contains("outside cwd"), "{path}: {err}");
        }
        let _ = fs::remove_dir_all(dir);
    }

    // ---------- T68.1: explore ----------

    /// A fake backend pins the assembled answer's section format byte-exact without
    /// a store or a language server: `= name` + bodies, `paths:` (or `none`), then
    /// `impact:` counts — and `before` is exactly the bytes the replaced
    /// `symbol` + `impact` calls would have printed.
    struct FakeParts;

    impl ExploreParts for FakeParts {
        fn resolve(&mut self, token: &str) -> Result<Vec<String>> {
            Ok(match token {
                "aa" => vec!["aa".to_string()],
                "bb" => vec!["bb".to_string()],
                _ => Vec::new(),
            })
        }
        fn defs(&mut self, name: &str) -> Result<String> {
            Ok(format!("src/{name}.rs:1 function\nfn {name}() {{}}\n"))
        }
        fn paths(&mut self, a: &str, b: &str) -> Result<Vec<String>> {
            if a == "aa" && b == "bb" {
                return Ok(vec!["aa → cc → bb".to_string()]);
            }
            Ok(Vec::new())
        }
        fn impact1(&mut self, name: &str) -> Result<(String, usize)> {
            Ok((format!("nothing reaches {name}"), 0))
        }
        fn def_count(&mut self, _name: &str) -> Result<usize> {
            Ok(1)
        }
    }

    #[test]
    fn assemble_explore_formats_sections_and_counts_replaced_bytes() {
        let mut parts = FakeParts;
        let (out, before) = assemble_explore("aa bb zz", &Filter::none(), &mut parts).unwrap();
        assert_eq!(
            out,
            "= aa\nsrc/aa.rs:1 function\nfn aa() {}\n\
             = bb\nsrc/bb.rs:1 function\nfn bb() {}\n\
             paths:\naa → cc → bb\n\
             impact:\naa ← 0\nbb ← 0\n"
        );
        let replaced: u64 = [
            "src/aa.rs:1 function\nfn aa() {}\n",
            "src/bb.rs:1 function\nfn bb() {}\n",
            "nothing reaches aa",
            "nothing reaches bb",
        ]
        .iter()
        .map(|s| s.len() as u64)
        .sum();
        assert_eq!(before, replaced);
    }

    #[test]
    fn assemble_explore_prints_none_when_no_path_connects() {
        let (out, _) = assemble_explore("zz", &Filter::none(), &mut FakeParts).unwrap();
        assert_eq!(out, "no symbols resolved for \"zz\"");
        let (out, _) = assemble_explore("aa", &Filter::none(), &mut FakeParts).unwrap();
        assert!(out.contains("paths:\nnone\n"), "{out}");
        assert!(out.contains("impact:\naa ← 0\n"), "{out}");
    }

    #[test]
    fn explore_tokens_split_query_and_dedupe() {
        assert_eq!(
            explore_tokens("how do Foo_bar and foo-bar relate?"),
            vec![
                "how".to_string(),
                "do".to_string(),
                "Foo_bar".to_string(),
                "and".to_string(),
                "foo".to_string(),
                "bar".to_string(),
                "relate".to_string()
            ]
        );
    }

    /// End to end over the tags index: one question about `b` and `c` answers with
    /// both definition bodies, the only caller chain between them and both impact
    /// counts — byte for byte.
    #[test]
    fn explore_answers_a_two_symbol_question_byte_exact() {
        let (cx, dir) = cx("explore");
        fs::write(
            dir.join("chain.rs"),
            "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn c() {}\n",
        )
        .unwrap();
        fs::write(dir.join("other.rs"), "fn d() {\n    c();\n    c();\n}\n").unwrap();
        let out = explore(
            &Ctx::new(&cx),
            &dir,
            "how do b and c interact",
            &Filter::none(),
        )
        .unwrap();
        assert_eq!(
            out,
            "= b\nchain.rs:4 function\nfn b() {\n    c();\n}\ncalls: c\n\
             = c\nchain.rs:7 function\nfn c() {}\n\
             paths:\nc → b\n\
             impact:\nb ← 1\nc ← 2\n"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// Prefix resolution ranks by reference count: `alphabet` (referenced twice)
    /// lands before `alpha` (referenced once) for the token `alph`.
    #[test]
    fn explore_prefix_resolution_ranks_by_reference_count() {
        let (cx, dir) = cx("explore-prefix");
        fs::write(
            dir.join("p.rs"),
            "fn alpha() {}\nfn alphabet() {}\nfn user() {\n    alphabet();\n    alpha();\n    alphabet();\n}\n",
        )
        .unwrap();
        let out = explore(&Ctx::new(&cx), &dir, "alph", &Filter::none()).unwrap();
        let alphabet = out.find("= alphabet\n").expect("alphabet section");
        let alpha = out.find("= alpha\n").expect("alpha section");
        assert!(alphabet < alpha, "{out}");
        assert_eq!(
            explore(&Ctx::new(&cx), &dir, "zz zz", &Filter::none()).unwrap(),
            "no symbols resolved for \"zz zz\""
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// The `path` filter keeps the printed definitions in one subtree, same as
    /// `symbol`'s filter.
    #[test]
    fn explore_path_filter_keeps_one_subtree() {
        let (cx, dir) = cx("explore-path");
        fs::write(dir.join("a.rs"), "fn dup() {}\n").unwrap();
        fs::write(dir.join("b.rs"), "fn dup() {}\n").unwrap();
        let out = explore(
            &Ctx::new(&cx),
            &dir,
            "dup",
            &Filter {
                path: "b.rs".into(),
                kind: String::new(),
            },
        )
        .unwrap();
        assert!(out.contains("b.rs:1") && !out.contains("a.rs:1"), "{out}");
        let _ = fs::remove_dir_all(dir);
    }

    /// T68.4: `impact --to` prints call chains between two symbols on the fixture.
    #[test]
    fn impact_to_filters_call_chains() {
        let (cx, dir) = cx("impact-to");
        fs::write(
            dir.join("chain.rs"),
            "fn a() {
    b();
}
fn b() {
    c();
}
fn c() {}
",
        )
        .unwrap();
        let ctx = Ctx::new(&cx);
        assert_eq!(
            impact(&ctx, &dir, "a", 3, Some("c")).unwrap(),
            "a → b → c
"
        );
        assert_eq!(
            impact(&ctx, &dir, "a", 3, Some("b")).unwrap(),
            "a → b
"
        );
        assert_eq!(
            impact(&ctx, &dir, "b", 3, Some("a")).unwrap(),
            "no path from b to a within depth 3"
        );
        assert_eq!(
            impact(&ctx, &dir, "a", 1, Some("c")).unwrap(),
            "no path from a to c within depth 1"
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// `symbol_paths` walks caller edges: chains read from the callee towards its
    /// callers, cycles terminate, and a name with no inbound chain answers empty.
    #[test]
    fn symbol_paths_walks_callers_and_terminates_on_cycles() {
        let (cx, dir) = cx("paths");
        fs::write(
            dir.join("chain.rs"),
            "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn c() {}\n",
        )
        .unwrap();
        fs::write(dir.join("other.rs"), "fn d() {\n    c();\n}\n").unwrap();
        index::run(&Ctx::new(&cx), &dir, false).unwrap();
        let key = index::canon(&dir);
        assert_eq!(
            cx.store.symbol_paths(&key, "c", "b", 3).unwrap(),
            vec!["c → b".to_string()]
        );
        assert!(cx.store.symbol_paths(&key, "b", "c", 3).unwrap().is_empty());
        assert_eq!(
            cx.store.symbol_paths(&key, "c", "a", 3).unwrap(),
            vec!["c → b → a".to_string()]
        );
        assert_eq!(
            cx.store.symbol_paths(&key, "c", "d", 3).unwrap(),
            vec!["c → d".to_string()]
        );
        fs::write(
            dir.join("cyc.rs"),
            "fn e() {\n    f();\n    g();\n}\nfn f() {\n    e();\n}\nfn g() {}\n",
        )
        .unwrap();
        index::run(&Ctx::new(&cx), &dir, false).unwrap();
        assert_eq!(
            cx.store.symbol_paths(&key, "e", "f", 3).unwrap(),
            vec!["e → f".to_string()]
        );
        assert!(
            cx.store.symbol_paths(&key, "f", "g", 3).unwrap().is_empty(),
            "e → f cycles; g is never a caller here"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
