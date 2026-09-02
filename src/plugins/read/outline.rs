//! `mode=map` / `mode=signatures` via tree-sitter-tags (plan T4.3).

use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::Language;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

/// Definitions as `kind name line`, or verbatim definition lines.
pub fn render(path: &Path, src: &str, mode: &str) -> Result<String> {
    let Some(cfg) = config(path) else {
        return Ok(fallback(src));
    };
    let cfg = cfg?;
    let mut ctx = TagsContext::new();
    let (tags, _) = ctx
        .generate_tags(&cfg, src.as_bytes(), None)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut out = Vec::new();
    for tag in tags.flatten() {
        if !tag.is_definition {
            continue;
        }
        let Some(name) = src.get(tag.name_range.clone()) else {
            continue;
        };
        if mode == "signatures" {
            let line = src.get(tag.line_range.clone()).unwrap_or("").trim_end();
            out.push(line.to_string());
        } else {
            let line = src.get(tag.line_range.clone()).unwrap_or("");
            let kw = line
                .split_whitespace()
                .next()
                .unwrap_or_else(|| cfg.syntax_type_name(tag.syntax_type_id));
            out.push(format!("{} {name} {}", kw, tag.span.start.row + 1));
        }
    }
    if out.is_empty() {
        Ok(fallback(src))
    } else {
        Ok(out.join("\n"))
    }
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
