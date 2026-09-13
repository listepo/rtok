//! T30.2 / Gate P30: same MCP names; tags miss the type-position fixture; LSP hits it.
//!
//! Skips the rust-analyzer path when `rust-analyzer --version` is not on PATH.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use rtok::config::Config;
use rtok::plugin::{Ctx, Plugin, Runtime};
use rtok::plugins::graph::{Graph, callers, outline, symbol};

const CHAIN: &str = "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn c() {}\n";
const OTHER: &str = "fn d() {\n    c();\n    c();\n}\n";
const FIXTURE: &str = "\
pub struct OnlyTyped;
pub fn user(_t: Vec<OnlyTyped>) {}
";

fn rust_analyzer_on_path() -> bool {
    Command::new("rust-analyzer")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn open(tag: &str, backend: &str) -> (Runtime, PathBuf) {
    let dir = std::env::temp_dir().join(format!("rtok-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::default();
    cfg.core.db_path = dir.join("rtok.db");
    cfg.core.archive_dir = dir.join("archive");
    cfg.plugins.graph.backend = backend.into();
    (Runtime::open(cfg, tag).unwrap(), dir)
}

fn onlytyped_crate(dir: &std::path::Path) -> PathBuf {
    let root = dir.join("crate");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"lsp_gate\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), FIXTURE).unwrap();
    root
}

#[test]
fn mcp_tool_names_are_unchanged() {
    let names: Vec<_> = Graph.mcp_tools().into_iter().map(|t| t.name).collect();
    assert_eq!(names, ["symbol", "callers", "impact", "outline"]);
}

/// Gate P30: `backend = "tags"` keeps the T8.9 contract bytes.
#[test]
fn tags_backend_callers_bytes_match_contract() {
    let (cx, dir) = open("p30-tags-bytes", "tags");
    let a = dir.join("a");
    fs::create_dir_all(&a).unwrap();
    fs::write(a.join("chain.rs"), CHAIN).unwrap();
    fs::write(a.join("other.rs"), OTHER).unwrap();
    let ctx = Ctx::new(&cx);
    assert_eq!(
        symbol(&ctx, &a, "b").unwrap(),
        "chain.rs:4 function\nfn b() {\n    c();\n}\n"
    );
    assert_eq!(
        callers(&ctx, &a, "c").unwrap(),
        "chain.rs  b ×1 (L5)\nother.rs  d ×2 (L2)\n"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// Gate P30: type-position `OnlyTyped` is a tags miss (T8.8).
#[test]
fn tags_backend_misses_onlytyped_type_position() {
    let (cx, dir) = open("p30-tags-miss", "tags");
    let root = onlytyped_crate(&dir);
    let out = callers(&Ctx::new(&cx), &root, "OnlyTyped").unwrap();
    assert_eq!(out, "no references to OnlyTyped", "{out}");
    let _ = fs::remove_dir_all(&dir);
}

/// Gate P30: rust-analyzer `textDocument/references` hits `user`'s `Vec<OnlyTyped>`.
#[test]
fn lsp_backend_hits_onlytyped_type_position() {
    if !rust_analyzer_on_path() {
        eprintln!("skip: rust-analyzer not on PATH");
        return;
    }
    let (cx, dir) = open("p30-lsp-hit", "lsp");
    let root = onlytyped_crate(&dir);
    let ctx = Ctx::new(&cx);
    let out = callers(&ctx, &root, "OnlyTyped").unwrap();
    assert!(
        out.contains("user"),
        "LSP callers should hit user(Vec<OnlyTyped>): {out}"
    );
    assert!(!out.starts_with("no references to OnlyTyped"), "{out}");
    let kinds: Vec<_> = cx
        .store
        .list_measurements("graph")
        .unwrap()
        .into_iter()
        .map(|m| m.kind)
        .collect();
    assert!(
        kinds.iter().any(|k| k.starts_with("lsp")),
        "expected plugin=graph lsp measurement, got {kinds:?}"
    );
    let sym = symbol(&ctx, &root, "OnlyTyped").unwrap();
    assert!(sym.contains("OnlyTyped"), "{sym}");
    let map = outline(&ctx, &root.join("src/lib.rs").to_string_lossy()).unwrap();
    assert!(map.contains("OnlyTyped") || map.contains("user"), "{map}");
    let _ = fs::remove_dir_all(&dir);
}
