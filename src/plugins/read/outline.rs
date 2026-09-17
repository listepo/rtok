//! `mode=map` / `mode=signatures` via tree-sitter-tags (plan T4.3).

use std::path::Path;
use std::sync::OnceLock;

use anyhow::Result;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

/// One tags-query hit (definition or reference).
#[derive(Debug, Clone)]
pub struct TagHit {
    pub kind: String,
    pub name: String,
    pub line: usize,
    /// Last line of the tagged node, from the tag's byte range (T8.5). A reference spans
    /// one line, so `end_line == line`; a definition covers its whole body.
    pub end_line: usize,
    pub is_def: bool,
    pub line_text: String,
}

/// True when `path` has a tags-supported extension (cheap; does not parse).
pub fn supported(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    match ext {
        #[cfg(feature = "lang-rust")]
        "rs" => true,
        #[cfg(feature = "lang-ts")]
        "ts" | "tsx" => true,
        #[cfg(feature = "lang-js")]
        "js" | "mjs" | "cjs" => true,
        #[cfg(feature = "lang-python")]
        "py" => true,
        #[cfg(feature = "lang-dart")]
        "dart" => true,
        #[cfg(feature = "lang-c")]
        "c" | "h" => true,
        #[cfg(feature = "lang-go")]
        "go" => true,
        _ => false,
    }
}

/// Definitions and references from the grammar's tags query. Unknown language → empty.
pub fn tags(path: &Path, src: &str) -> Result<Vec<TagHit>> {
    let Some(cfg) = config(path) else {
        return Ok(Vec::new());
    };
    let cfg = cfg?;
    let mut ctx = TagsContext::new();
    let (tags, _) = ctx
        .generate_tags(cfg, src.as_bytes(), None)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    // Byte offset of each line start; `partition_point` turns a byte into a 1-based line.
    let starts: Vec<usize> = std::iter::once(0)
        .chain(src.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let line_of = |b: usize| starts.partition_point(|&s| s <= b).max(1);
    let mut out = Vec::new();
    for tag in tags.flatten() {
        let Some(name) = src.get(tag.name_range.clone()) else {
            continue;
        };
        let line_text = src.get(tag.line_range.clone()).unwrap_or("").to_string();
        out.push(TagHit {
            kind: cfg.syntax_type_name(tag.syntax_type_id).to_string(),
            name: name.to_string(),
            line: tag.span.start.row + 1,
            end_line: line_of(tag.range.end.saturating_sub(1)),
            is_def: tag.is_definition,
            line_text,
        });
    }
    Ok(out)
}

/// Definitions as `kind name line`, or verbatim definition lines.
pub fn render(path: &Path, src: &str, mode: &str) -> Result<String> {
    let defs: Vec<TagHit> = tags(path, src)?.into_iter().filter(|h| h.is_def).collect();
    if defs.is_empty() {
        return Ok(fallback(src));
    }
    let mut out = Vec::new();
    for hit in defs {
        if mode == "signatures" {
            out.push(hit.line_text.trim_end().to_string());
        } else {
            let kw = hit
                .line_text
                .split_whitespace()
                .next()
                .unwrap_or(hit.kind.as_str());
            out.push(format!("{} {} {}", kw, hit.name, hit.line));
        }
    }
    Ok(out.join("\n"))
}

fn fallback(src: &str) -> String {
    let body = src
        .lines()
        .take(60)
        .enumerate()
        .map(|(i, l)| format!("{}:{l}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{body}\n(note: unknown language, showing lines 1-60)")
}

/// A language's compiled tags query, kept for the process. A failed compile keeps its message,
/// so every later call gets the same `Err` rather than a retry.
type Cached = OnceLock<Result<TagsConfiguration, String>>;

fn cached(
    cell: &'static Cached,
    build: impl FnOnce() -> Result<TagsConfiguration, tree_sitter_tags::Error>,
) -> Option<Result<&'static TagsConfiguration>> {
    let cfg = cell.get_or_init(|| build().map_err(|e| format!("tags query: {e}")));
    Some(cfg.as_ref().map_err(|e| anyhow::anyhow!("{e}")))
}

/// One `static` cell per match arm, so each language compiles at most once.
macro_rules! compiled {
    ($lang:expr, $tags:expr, $locals:expr) => {{
        static CELL: Cached = OnceLock::new();
        cached(&CELL, || {
            TagsConfiguration::new($lang.into(), $tags, $locals)
        })
    }};
}

/// The upstream Rust tags query only sees bare and method calls; path-qualified calls
/// (`tokens::estimate(..)`) are the common form, so `callers` (T8.2) needs this pattern too.
#[cfg(feature = "lang-rust")]
pub(crate) const RUST_SCOPED_CALL: &str = "
(call_expression
    function: (scoped_identifier
        name: (identifier) @name)) @reference.call
";

/// The query for `path`'s language, compiled on first use (T35.1): the compile was 19 ms of a
/// 26.5 ms `tags` call on `graph/index.rs` (debug, 2026-09-10), paid again on every file.
fn config(path: &Path) -> Option<Result<&'static TagsConfiguration>> {
    match path.extension()?.to_str()? {
        #[cfg(feature = "lang-rust")]
        "rs" => compiled!(
            tree_sitter_rust::LANGUAGE,
            &format!("{}{RUST_SCOPED_CALL}", tree_sitter_rust::TAGS_QUERY),
            ""
        ),
        #[cfg(feature = "lang-ts")]
        "ts" => compiled!(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            tree_sitter_typescript::TAGS_QUERY,
            tree_sitter_typescript::LOCALS_QUERY
        ),
        #[cfg(feature = "lang-ts")]
        "tsx" => compiled!(
            tree_sitter_typescript::LANGUAGE_TSX,
            tree_sitter_typescript::TAGS_QUERY,
            tree_sitter_typescript::LOCALS_QUERY
        ),
        #[cfg(feature = "lang-js")]
        "js" | "mjs" | "cjs" => compiled!(
            tree_sitter_javascript::LANGUAGE,
            tree_sitter_javascript::TAGS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY
        ),
        #[cfg(feature = "lang-python")]
        "py" => compiled!(
            tree_sitter_python::LANGUAGE,
            tree_sitter_python::TAGS_QUERY,
            ""
        ),
        #[cfg(feature = "lang-dart")]
        "dart" => compiled!(
            tree_sitter_dart::LANGUAGE,
            tree_sitter_dart::TAGS_QUERY,
            tree_sitter_dart::LOCALS_QUERY
        ),
        #[cfg(feature = "lang-c")]
        "c" | "h" => compiled!(tree_sitter_c::LANGUAGE, tree_sitter_c::TAGS_QUERY, ""),
        #[cfg(feature = "lang-go")]
        "go" => compiled!(tree_sitter_go::LANGUAGE, tree_sitter_go::TAGS_QUERY, ""),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(src: &str) -> String {
        let mut s = src.to_string();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        let n = s.lines().count();
        for i in n..20 {
            s.push_str(&format!("// pad {i}\n"));
        }
        s
    }

    #[test]
    fn golden_per_language() {
        let cases = [
            ("a.rs", "fn main() {}\nfn helper() {}\n", "fn main"),
            (
                "a.ts",
                "function greet(n: string) { return n; }\n",
                "function greet",
            ),
            ("a.js", "function add(x) { return x; }\n", "function add"),
            ("a.py", "def run():\n    return 1\n", "def run"),
            ("a.dart", "void start() {}\n", "void start"),
            ("a.c", "int add(int x) { return x; }\n", "int add"),
            ("a.go", "func Sum() int { return 0 }\n", "func Sum"),
        ];
        for (path, src, needle) in cases {
            let src = pad(src);
            let map = render(Path::new(path), &src, "map").unwrap();
            assert!(map.contains(needle), "{path} map: {map}");
            let sig = render(Path::new(path), &src, "signatures").unwrap();
            let name = needle.split_whitespace().nth(1).unwrap_or(needle);
            assert!(
                sig.lines().any(|l| l.contains(name)),
                "{path} signatures: {sig}"
            );
        }
    }

    /// Same pointer for two files of one language: the query compiled once (T35.1). A
    /// recompile per call would hand back a fresh configuration each time.
    #[test]
    fn each_language_compiles_once() {
        let a = config(Path::new("a.rs")).unwrap().unwrap();
        let b = config(Path::new("src/b.rs")).unwrap().unwrap();
        assert!(std::ptr::eq(a, b), "a second .rs file recompiled the query");
        let ts = config(Path::new("a.ts")).unwrap().unwrap();
        assert!(
            !std::ptr::eq(a, ts),
            "two languages share one configuration"
        );
    }

    #[test]
    fn unknown_language_falls_back() {
        let src = "hello\nworld\n";
        let out = render(Path::new("a.txt"), src, "map").unwrap();
        assert!(out.contains("1:hello"), "{out}");
        assert!(out.contains("unknown language"), "{out}");
    }
}
