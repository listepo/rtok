//! `mode=map` / `mode=signatures` via tree-sitter-tags (plan T4.3).

use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::Language;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

/// One tags-query hit (definition or reference).
#[derive(Debug, Clone)]
pub struct TagHit {
    pub kind: String,
    pub name: String,
    pub line: usize,
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
        .generate_tags(&cfg, src.as_bytes(), None)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
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

fn make(lang: Language, tags: &str, locals: &str) -> Option<Result<TagsConfiguration>> {
    Some(TagsConfiguration::new(lang, tags, locals).context("tags query"))
}

fn config(path: &Path) -> Option<Result<TagsConfiguration>> {
    match path.extension()?.to_str()? {
        #[cfg(feature = "lang-rust")]
        "rs" => make(
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::TAGS_QUERY,
            "",
        ),
        #[cfg(feature = "lang-ts")]
        "ts" => make(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            tree_sitter_typescript::TAGS_QUERY,
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        #[cfg(feature = "lang-ts")]
        "tsx" => make(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            tree_sitter_typescript::TAGS_QUERY,
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        #[cfg(feature = "lang-js")]
        "js" | "mjs" | "cjs" => make(
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::TAGS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        #[cfg(feature = "lang-python")]
        "py" => make(
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::TAGS_QUERY,
            "",
        ),
        #[cfg(feature = "lang-dart")]
        "dart" => make(
            tree_sitter_dart::LANGUAGE.into(),
            tree_sitter_dart::TAGS_QUERY,
            tree_sitter_dart::LOCALS_QUERY,
        ),
        #[cfg(feature = "lang-c")]
        "c" | "h" => make(
            tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::TAGS_QUERY,
            "",
        ),
        #[cfg(feature = "lang-go")]
        "go" => make(
            tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::TAGS_QUERY,
            "",
        ),
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

    #[test]
    fn unknown_language_falls_back() {
        let src = "hello\nworld\n";
        let out = render(Path::new("a.txt"), src, "map").unwrap();
        assert!(out.contains("1:hello"), "{out}");
        assert!(out.contains("unknown language"), "{out}");
    }
}
