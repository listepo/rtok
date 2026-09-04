//! T8.9: the `graph` contract, pinned through the binary (P8c).
//!
//! Every assertion here goes through `rtok mcp` on stdio, so nothing below depends on how
//! the index is stored. A storage backend is acceptable when this file passes unchanged:
//! the same four tools, byte for byte, plus the three index behaviours a caller can observe
//! — a second repo does not disturb the first, an edited file is re-read, a deleted file
//! loses its rows. The expected strings are the v0.2 (SQLite) output, copied verbatim.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CHAIN: &str = "fn a() {\n    b();\n}\nfn b() {\n    c();\n}\nfn c() {}\n";
const OTHER: &str = "fn d() {\n    c();\n    c();\n}\n";

/// One `tools/call` against a fresh `rtok mcp` process rooted at `cwd`, sharing `home`.
fn call(home: &Path, cwd: &Path, tool: &str, args: serde_json::Value) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtok"))
        .arg("mcp")
        .env("RTOK_HOME", home)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtok mcp");
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": args}
    });
    child
        .stdin
        .take()
        .unwrap()
        .write_all(req.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let line = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(line.trim()).expect("one JSON-RPC line");
    v["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text in {line}"))
        .to_string()
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-contract-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn repo(home: &Path, name: &str) -> PathBuf {
    let dir = home.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("chain.rs"), CHAIN).unwrap();
    std::fs::write(dir.join("other.rs"), OTHER).unwrap();
    dir
}

#[test]
fn four_tools_byte_exact() {
    let home = tmp("tools");
    let a = repo(&home, "a");
    let name = |n: &str| serde_json::json!({"name": n});
    assert_eq!(
        call(&home, &a, "symbol", name("b")),
        "chain.rs:4 function\nfn b() {\n    c();\n}\n"
    );
    assert_eq!(
        call(&home, &a, "callers", name("c")),
        "chain.rs  b \u{d7}1 (L5)\nother.rs  d \u{d7}2 (L2)\n"
    );
    assert_eq!(
        call(
            &home,
            &a,
            "impact",
            serde_json::json!({"name": "c", "depth": 2})
        ),
        "1  chain.rs  b\n1  other.rs  d\n2  chain.rs  a\n"
    );
    assert_eq!(
        call(
            &home,
            &a,
            "impact",
            serde_json::json!({"name": "c", "depth": 1})
        ),
        "1  chain.rs  b\n1  other.rs  d\n"
    );
    assert_eq!(
        call(
            &home,
            &a,
            "outline",
            serde_json::json!({"path": "chain.rs"})
        ),
        "fn a 1\nfn b 4\nfn c 7"
    );
    assert_eq!(
        call(&home, &a, "symbol", name("zzz")),
        "no definition of zzz"
    );
    assert_eq!(call(&home, &a, "callers", name("a")), "no references to a");
    assert_eq!(call(&home, &a, "impact", name("a")), "nothing reaches a");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn second_repo_leaves_the_first_intact() {
    let home = tmp("roots");
    let a = repo(&home, "a");
    let b = home.join("b");
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(b.join("chain.rs"), "fn alpha() {}\n").unwrap();
    let name = serde_json::json!({"name": "a"});
    let first = call(&home, &a, "symbol", name.clone());
    assert_eq!(first, "chain.rs:1 function\nfn a() {\n    b();\n}\n");
    assert_eq!(
        call(&home, &b, "symbol", name.clone()),
        "no definition of a"
    );
    assert_eq!(
        call(&home, &a, "symbol", name),
        first,
        "repo b evicted repo a"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn edited_and_deleted_files_are_reflected() {
    let home = tmp("edit");
    let a = repo(&home, "a");
    let c = serde_json::json!({"name": "c"});
    assert_eq!(
        call(&home, &a, "callers", c.clone()),
        "chain.rs  b \u{d7}1 (L5)\nother.rs  d \u{d7}2 (L2)\n"
    );
    std::fs::write(
        a.join("other.rs"),
        "fn d() {\n    c();\n    c();\n    c();\n}\n",
    )
    .unwrap();
    assert_eq!(
        call(&home, &a, "callers", c.clone()),
        "chain.rs  b \u{d7}1 (L5)\nother.rs  d \u{d7}3 (L2)\n",
        "edited file was not re-read"
    );
    std::fs::remove_file(a.join("other.rs")).unwrap();
    assert_eq!(
        call(&home, &a, "callers", c),
        "chain.rs  b \u{d7}1 (L5)\n",
        "deleted file kept its rows"
    );
    let _ = std::fs::remove_dir_all(&home);
}
