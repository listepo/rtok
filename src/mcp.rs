//! `rtok mcp` — rmcp JSON-RPC over stdio (plan T4.1).
//!
//! A one-shot `tools/list` (the Check) is accepted without `initialize`.

use std::io::{BufRead, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, bail};
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, JsonObject, ListToolsResult, ServerCapabilities,
    ServerInfo, Tool,
};
use serde_json::{Value, json};

#[cfg(feature = "cmd")]
pub mod wrap;

use crate::config::Config;
use crate::plugin::{Runtime, ToolDef};
use crate::plugins::Registry;
use crate::tokens::Class;

fn expand_def() -> ToolDef {
    ToolDef {
        name: "expand",
        description: "Return archived payload by id; optional lines a-b, regex grep (hits as N:line), context N.",
        input_schema: json!({"type":"object","properties":{"id":{"type":"string"},"lines":{"type":"string"},"grep":{"type":"string"},"context":{"type":"integer"}},"required":["id"]}),
    }
}

/// Serve MCP on stdin/stdout until EOF.
#[cfg_attr(not(feature = "graph"), allow(unused_variables))]
pub fn run(cfg: &Config) -> Result<()> {
    let server = Server::new(cfg)?;
    // Retention is housekeeping with a next-start retry: the server must not die on a
    // contended store (T75) — WAL reads keep every tool serving while another process
    // writes, and the purge queues behind it under the maintenance busy window.
    if let Err(e) = server.cx.store.run_retention(cfg.core.retain_calls_days) {
        eprintln!("rtok mcp: retention skipped until next start: {e:#}");
    }
    crate::otel::export::spawn_ticker(cfg);
    // P8d watcher (T8.16): a thread inside this process, never a second writer.
    // Any value but `off` arms it; `watchman` gets its own backend in T8.17.
    let watch_root: Option<std::path::PathBuf> =
        if server.cx.config.plugins.graph.watch.as_str() != "off" {
            std::env::current_dir().ok()
        } else {
            None
        };
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        #[cfg(feature = "graph")]
        if let Some(root) = &watch_root {
            s.spawn(|| {
                crate::plugins::graph::watch::run(&crate::plugin::Ctx::new(&server.cx), root, &stop)
            });
        }
        let res: Result<()> = (|| {
            let mut stdin = std::io::stdin().lock();
            let mut stdout = std::io::stdout();
            let mut buf = Vec::new();
            while let Some(line) = next_line(&mut stdin, &mut buf, MAX_LINE)? {
                if line.trim().is_empty() {
                    continue;
                }
                if let Some(out) = server.handle_line(&line) {
                    writeln!(stdout, "{out}")?;
                    stdout.flush()?;
                }
            }
            Ok(())
        })();
        stop.store(true, Ordering::Relaxed);
        crate::otel::export::flush_blocking(&server.cx);
        #[cfg(feature = "graph")]
        crate::plugins::graph::lsp::shutdown();
        res
    })
}

/// Guards a one-shot `--call` the way `run`'s stdin-EOF path guards a served session: drops
/// the cached LSP child (T142) on every exit — success, `Err`, or an early `?` — since `call`
/// has no end-of-loop point of its own to shut it down at.
#[cfg(feature = "graph")]
struct LspGuard;

#[cfg(feature = "graph")]
impl Drop for LspGuard {
    fn drop(&mut self) {
        crate::plugins::graph::lsp::shutdown();
    }
}

/// One-shot `tools/call` for hosts that cannot speak MCP (`rtok mcp --call`, T70.3).
pub fn call(cfg: &Config, name: &str, args: &Value) -> Result<String> {
    #[cfg(feature = "graph")]
    let _lsp_guard = LspGuard;
    let server = Server::new(cfg)?;
    if !server.allows(name) {
        return Err(unknown_tool(name));
    }
    let plugin = server
        .listed
        .iter()
        .find(|t| t.def.name == name)
        .map(|t| t.plugin)
        .unwrap_or("archive");
    let args = if args.is_null() {
        json!({})
    } else {
        args.clone()
    };
    let (text, ok) = match invoke(&server.cx, name, &args) {
        Ok(t) => (t, true),
        Err(e) => (e.to_string(), false),
    };
    let _ = record(&server.cx, plugin, name, &args, &text);
    if ok { Ok(text) } else { bail!("{text}") }
}

/// Longest request line kept in memory. Tool arguments are notes and paths, far below this.
const MAX_LINE: u64 = 8 << 20;

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

/// The one place `"unknown tool: <name>"` is worded — a name `invoke` never heard of and a
/// name the allow-list dropped (T192) must read identically, so both paths call this instead
/// of formatting the string a second time.
fn unknown_tool(name: &str) -> anyhow::Error {
    anyhow::anyhow!("unknown tool: {name}")
}

/// The next request line, `None` at EOF. `lines()` ended the server on one non-UTF-8 byte (its
/// `Err` went up through `?`) and buffered a line of any length first. Now bad bytes become
/// U+FFFD and fail JSON parsing like any junk line, and a line over `max` answers `-32700`
/// (the rest of it is drained unbuffered, and the truncated `"{"` stub fails parsing downstream
/// instead of the loop skipping it as empty).
fn next_line(r: &mut impl BufRead, buf: &mut Vec<u8>, max: u64) -> std::io::Result<Option<String>> {
    buf.clear();
    if (&mut *r).take(max + 1).read_until(b'\n', buf)? == 0 {
        return Ok(None);
    }
    if buf.len() as u64 > max {
        if buf.last() != Some(&b'\n') {
            r.skip_until(b'\n')?;
        }
        return Ok(Some("{".to_owned()));
    }
    Ok(Some(String::from_utf8_lossy(buf).into_owned()))
}

struct Listed {
    plugin: &'static str,
    def: ToolDef,
}

struct Server {
    cx: Runtime,
    listed: Vec<Listed>,
}

impl Server {
    fn new(cfg: &Config) -> Result<Self> {
        // One session per process: `read_cache` rows are keyed by session and never expire, so
        // the literal "mcp" made every `rtok mcp` process answer `unchanged since <sha>` for a
        // file only another conversation had read. The surface stays "mcp" (see `record`).
        let cx = Runtime::open(cfg.clone(), format!("mcp-{}", std::process::id()))?;
        let mut listed = vec![Listed {
            plugin: "archive",
            def: expand_def(),
        }];
        let builtin: Vec<&str> = crate::plugins::all()
            .iter()
            .map(|p| p.manifest().id)
            .collect();
        for p in Registry::new(cfg).enabled() {
            let id = p.manifest().id;
            // Out-of-tree (WASM) plugins have no `tools/call` arm in `invoke` yet, so listing
            // their tools only bought callers an `unknown tool` error.
            if !builtin.contains(&id) {
                continue;
            }
            for def in p.mcp_tools() {
                if listed.iter().any(|t| t.def.name == def.name) {
                    continue;
                }
                listed.push(Listed { plugin: id, def });
            }
        }
        if !cfg.mcp.tools.is_empty() {
            // `expand` stays listed whatever the allow-list says: D4 losslessness.
            listed.retain(|t| {
                t.def.name == "expand" || cfg.mcp.tools.iter().any(|n| n.as_str() == t.def.name)
            });
        }
        Ok(Self { cx, listed })
    }

    fn tools(&self) -> Vec<Tool> {
        self.listed.iter().map(|t| to_tool(&t.def)).collect()
    }

    fn allows(&self, name: &str) -> bool {
        self.listed.iter().any(|t| t.def.name == name)
    }

    /// One line in, at most one line out. A JSON-RPC batch (top-level array) answers with an
    /// array of the responses its members produced; an empty batch is `-32600` per JSON-RPC 2.0.
    fn handle_line(&self, line: &str) -> Option<String> {
        if line.trim().is_empty() {
            return None;
        }
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return Some(rpc_error(Value::Null, -32700, "parse error").to_string()),
        };
        if let Some(items) = req.as_array() {
            if items.is_empty() {
                return Some(rpc_error(Value::Null, -32600, "empty batch").to_string());
            }
            let out: Vec<Value> = items.iter().filter_map(|v| self.handle_value(v)).collect();
            return (!out.is_empty()).then(|| Value::Array(out).to_string());
        }
        self.handle_value(&req).map(|v| v.to_string())
    }

    fn handle_value(&self, req: &Value) -> Option<Value> {
        let obj = match req.as_object() {
            Some(o) => o,
            None => return Some(rpc_error(Value::Null, -32600, "invalid request")),
        };
        let method = obj.get("method").and_then(|m| m.as_str()).unwrap_or("");
        if obj.get("id").is_none() || method.starts_with("notifications/") {
            return None;
        }
        let id = obj["id"].clone();
        if method.is_empty() {
            return Some(rpc_error(id, -32600, "invalid request"));
        }
        let result = match method {
            "initialize" => {
                // Default `Implementation` still comes from rmcp's build env (`name: "rmcp"`).
                // 3.x types are non_exhaustive; construct via the public builders.
                let info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
                    .with_server_info(Implementation::new("rtok", env!("CARGO_PKG_VERSION")));
                serde_json::to_value(&info).unwrap_or(json!({}))
            }
            "ping" => json!({}),
            "tools/list" => serde_json::to_value(ListToolsResult::with_all_items(self.tools()))
                .unwrap_or(json!({"tools": []})),
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let args = req["params"]["arguments"].clone();
                serde_json::to_value(self.call_tool(name, &args)).unwrap_or(json!({}))
            }
            _ => {
                return Some(
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":method}}),
                );
            }
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }

    fn call_tool(&self, name: &str, args: &Value) -> CallToolResult {
        let plugin = self
            .listed
            .iter()
            .find(|t| t.def.name == name)
            .map(|t| t.plugin)
            .unwrap_or("archive");
        let args = if args.is_null() {
            json!({})
        } else {
            args.clone()
        };
        // A failure is an `isError` result with the same message text, not a success block
        // the model has to recognise by wording. A name the allow-list dropped never reaches
        // `invoke` — it must not run a tool the config says is off — but it fails with the
        // exact text `invoke`'s own unknown-name arm would give (`unknown_tool`, T192).
        let (text, ok) = if self.allows(name) {
            match invoke(&self.cx, name, &args) {
                Ok(t) => (t, true),
                Err(e) => (e.to_string(), false),
            }
        } else {
            (unknown_tool(name).to_string(), false)
        };
        let _ = record(&self.cx, plugin, name, &args, &text);
        let content = vec![ContentBlock::text(text)];
        if ok {
            CallToolResult::success(content)
        } else {
            CallToolResult::error(content)
        }
    }
}

fn to_tool(def: &ToolDef) -> Tool {
    let schema = def
        .input_schema
        .as_object()
        .cloned()
        .unwrap_or_else(JsonObject::new);
    Tool::new(def.name, def.description, Arc::new(schema))
}

fn invoke(cx: &Runtime, name: &str, args: &Value) -> Result<String> {
    match name {
        "expand" => expand_text(cx, args),
        #[cfg(feature = "memory")]
        "mem_save" => mem_save(cx, args),
        #[cfg(feature = "memory")]
        "mem_search" => mem_search(cx, args),
        #[cfg(feature = "memory")]
        "mem_get" => mem_get(cx, args),
        #[cfg(feature = "memory")]
        "mem_update" => mem_update(cx, args),
        #[cfg(feature = "memory")]
        "handoff" => handoff(cx, args),
        #[cfg(feature = "read")]
        "read" => read_file(cx, args),
        #[cfg(feature = "read")]
        "search" => search_files(cx, args),
        #[cfg(feature = "read")]
        "tree" => tree_files(cx, args),
        #[cfg(feature = "graph")]
        "symbol" | "callers" | "impact" | "outline" | "explore" => {
            crate::plugins::graph::call(&crate::plugin::Ctx::new(cx), name, args)
        }
        _ => Err(unknown_tool(name)),
    }
}

fn expand_text(cx: &Runtime, args: &Value) -> Result<String> {
    let id = args["id"].as_str().unwrap_or("");
    if let Some(spec) = args["lines"].as_str() {
        crate::expand::parse_range(spec, usize::MAX)?;
    }
    let Some(bytes) = crate::expand::fetch(cx, id)? else {
        bail!("unknown archive id: {id}");
    };
    let text = String::from_utf8_lossy(&bytes);
    let context = args["context"].as_u64().map_or(0, |n| n as usize);
    let body = slice(
        &text,
        args["lines"].as_str(),
        args["grep"].as_str(),
        context,
    )?;
    Ok(cap_result(
        &body,
        id,
        cx.config.mcp.max_result_chars as usize,
    ))
}

fn slice(text: &str, lines: Option<&str>, grep: Option<&str>, context: usize) -> Result<String> {
    Ok(crate::expand::filter_lines(text, lines, grep, context)?.join("\n"))
}

fn cap_result(text: &str, id: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    crate::expand::cut(text, &format!("\n… expand({id}) …\n"), max)
}

#[cfg(feature = "memory")]
fn mem_save(cx: &Runtime, args: &Value) -> Result<String> {
    let kind = args["kind"].as_str().unwrap_or("note");
    let title = args["title"].as_str().unwrap_or("");
    let body = args["body"].as_str().unwrap_or("");
    let project = args["project"].as_str();
    let (id, updated) = crate::plugins::memory::mem_save(cx, kind, title, body, project)?;
    Ok(json!({"id": id, "updated": updated}).to_string())
}

#[cfg(feature = "memory")]
fn mem_search(cx: &Runtime, args: &Value) -> Result<String> {
    let query = args["query"].as_str().unwrap_or("");
    // `plugins.memory.search_limit` is the ceiling, not just the default: the caller's
    // `limit` used to size the response unbounded.
    let max = u64::from(cx.config.plugins.memory.search_limit);
    let limit = args["limit"].as_u64().map_or(max, |n| n.min(max)) as u32;
    let hits = crate::plugins::memory::mem_search(cx, query, limit)?;
    Ok(json!(
        hits.iter()
            .map(|h| json!({"id": h.id, "title": h.title, "snippet": h.snippet}))
            .collect::<Vec<_>>()
    )
    .to_string())
}

#[cfg(feature = "read")]
fn search_files(cx: &Runtime, args: &Value) -> Result<String> {
    let pattern = args["pattern"].as_str().unwrap_or("");
    let path = args["path"].as_str().unwrap_or(".");
    let max = args["max"].as_u64().map(|n| n as u32);
    crate::plugins::read::search::search(&crate::plugin::Ctx::new(cx), pattern, path, max)
}

#[cfg(feature = "read")]
fn tree_files(cx: &Runtime, args: &Value) -> Result<String> {
    let path = args["path"].as_str().unwrap_or(".");
    let depth = args["depth"].as_u64().map(|n| n as u32);
    crate::plugins::read::search::tree(&crate::plugin::Ctx::new(cx), path, depth)
}

#[cfg(feature = "read")]
fn read_file(cx: &Runtime, args: &Value) -> Result<String> {
    let path = args["path"].as_str().unwrap_or("");
    let mode = args["mode"].as_str().unwrap_or("");
    let range = args["range"].as_str();
    crate::plugins::read::read(&crate::plugin::Ctx::new(cx), path, mode, range)
}

#[cfg(feature = "memory")]
fn mem_get(cx: &Runtime, args: &Value) -> Result<String> {
    let id = args["id"]
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| anyhow::anyhow!("invalid note id: {}", args["id"]))?;
    crate::plugins::memory::mem_get(cx, id)?.ok_or_else(|| anyhow::anyhow!("unknown note id: {id}"))
}

#[cfg(feature = "memory")]
fn mem_update(cx: &Runtime, args: &Value) -> Result<String> {
    let id = args["id"]
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| anyhow::anyhow!("invalid note id: {}", args["id"]))?;
    let retire = args["retire"].as_bool().unwrap_or(false);
    let superseded_by = args["superseded_by"]
        .as_i64()
        .and_then(|n| i32::try_from(n).ok());
    let pinned = args["pinned"].as_bool();
    crate::plugins::memory::mem_update(cx, id, retire, superseded_by, pinned)
}

fn record(cx: &Runtime, plugin: &str, name: &str, args: &Value, result: &str) -> Result<()> {
    let args_s = args.to_string();
    let before = i64::from(cx.estimate(&args_s, Class::Json));
    let after = i64::from(cx.estimate(result, Class::Json));
    let host = cx.store.host_id(&cx.config.hook.host)?.or(Some(6));
    cx.store
        .upsert_session(&cx.session, host, None, None, Some("mcp"))?;
    let call_id = cx.store.insert_call(
        &cx.session,
        "mcp",
        "mcp_call",
        host,
        None,
        None,
        Some(plugin),
        Some(name),
    )?;
    let cap = cx.config.core.call_io_inline_bytes as usize;
    cx.store.insert_call_io(
        call_id,
        Some(args_s.as_bytes()),
        Some(result.as_bytes()),
        cap,
        Some(&cx.config.core.archive_dir),
    )?;
    cx.store
        .insert_tokens(call_id, None, "before", "estimate", before)?;
    cx.store
        .insert_tokens(call_id, None, "after", "estimate", after)?;
    cx.store
        .insert_tokens(call_id, Some(plugin), "mcp", "estimate", after)?;
    Ok(())
}

#[cfg(feature = "memory")]
fn handoff(cx: &Runtime, args: &Value) -> Result<String> {
    let budget = args["budget_tokens"].as_u64().unwrap_or(800) as u32;
    Ok(crate::plugins::memory::handoff::handoff(
        &crate::plugin::Ctx::new(cx),
        budget,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::testutil::config as tmp;
    use crate::tokens::Class;
    use rstest::rstest;
    use std::fs;

    /// A long line answers `-32700` downstream (the next request still parses) and a
    /// non-UTF-8 byte no longer ends the read loop.
    #[test]
    fn next_line_skips_long_lines_and_survives_bad_utf8() {
        let mut r: &[u8] = b"0123456789\n{\"id\":1}\n\xff\n1234\n";
        let mut buf = Vec::new();
        let mut got = Vec::new();
        while let Some(l) = next_line(&mut r, &mut buf, 5).unwrap() {
            got.push(l);
        }
        assert_eq!(got, ["{", "{", "\u{FFFD}\n", "1234\n"]);
        let mut r: &[u8] = b"0123456789\n{\"id\":1}\n";
        assert_eq!(next_line(&mut r, &mut buf, 5).unwrap().unwrap(), "{");
        assert_eq!(
            next_line(&mut r, &mut buf, 64).unwrap().unwrap(),
            "{\"id\":1}\n"
        );
    }

    #[test]
    fn malformed_line_answers_parse_error() {
        let (cfg, dir) = tmp("mcp-bad");
        let server = Server::new(&cfg).unwrap();
        let v: Value = serde_json::from_str(&server.handle_line("{bad").unwrap()).unwrap();
        assert_eq!(v["error"]["code"], -32700);
        assert_eq!(v["id"], Value::Null);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn non_object_line_answers_invalid_request() {
        let (cfg, dir) = tmp("mcp-nonobj");
        let server = Server::new(&cfg).unwrap();
        let v: Value = serde_json::from_str(&server.handle_line("123").unwrap()).unwrap();
        assert_eq!(v["error"]["code"], -32600);
        assert_eq!(v["id"], Value::Null);
        assert!(server.handle_line("").is_none());
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/x"}"#)
                .is_none()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn slice_bare_lines_matches_cli_parse_range() {
        let text = (1..=12)
            .map(|n| format!("L{n}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(slice(&text, Some("10"), None, 0).unwrap(), "L10\nL11\nL12");
        assert_eq!(slice(&text, Some("10-10"), None, 0).unwrap(), "L10");
        assert!(slice(&text, Some("wat"), None, 0).is_err());
    }

    /// T67.2: the MCP `expand` accepts `context` like the CLI `--context`, windows
    /// merged with `--`, absolute numbers, under the same `max_result_chars` cap.
    #[test]
    fn expand_text_context_returns_merged_windows() {
        let (cfg, dir) = tmp("mcp-ctx");
        let cx = crate::plugin::Runtime::open(cfg, "mcp-ctx").unwrap();
        let body = "a\nHIT\nb\nc\nd\ne\nHIT\nf\n";
        let id = cx
            .store
            .put_archive("mcp", body.as_bytes(), &cx.config.core.archive_dir)
            .unwrap();
        let args = serde_json::json!({"id": id, "grep": "HIT", "context": 1});
        let out = expand_text(&cx, &args).unwrap();
        assert_eq!(out, "1:a\n2:HIT\n3:b\n--\n6:e\n7:HIT\n8:f");
        // Without `context` the hits stay bare, as before.
        let args = serde_json::json!({"id": id, "grep": "HIT"});
        assert_eq!(expand_text(&cx, &args).unwrap(), "2:HIT\n7:HIT");
        let _ = fs::remove_dir_all(dir);
    }

    #[rstest]
    #[case(500)]
    #[case(200)]
    fn expand_honours_max_result_chars(#[case] max_chars: u32) {
        let (mut cfg, dir) = tmp("mcp-cap");
        cfg.mcp.max_result_chars = max_chars;
        let cx = crate::plugin::Runtime::open(cfg, "mcp-cap").unwrap();
        let blob = "x".repeat(100 * 1024);
        let id = cx
            .store
            .put_archive("mcp", blob.as_bytes(), &cx.config.core.archive_dir)
            .unwrap();
        let args = serde_json::json!({"id": id});
        let out = expand_text(&cx, &args).unwrap();
        assert!(out.contains("expand("), "{out}");
        assert!(
            out.chars().count() <= max_chars as usize,
            "{}/{}",
            out.chars().count(),
            max_chars
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn descriptions_at_most_60_tokens() {
        let (cfg, dir) = tmp("desc");
        let server = Server::new(&cfg).unwrap();
        let max = cfg.mcp.max_description_tokens;
        for t in server.tools() {
            let d = t.description.as_deref().unwrap_or("");
            let n = crate::tokens::estimate(d, Class::Prose, &cfg.estimator);
            assert!(n <= max, "{} is {n} tokens (max {max}): {d}", t.name);
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn tools_call_writes_calls_io_and_three_token_rows() {
        let (cfg, dir) = tmp("call");
        let server = Server::new(&cfg).unwrap();
        let id = server
            .cx
            .store
            .put_archive("mcp", b"payload", &cfg.core.archive_dir)
            .unwrap();
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"expand","arguments":{{"id":"{id}"}}}}}}"#
        );
        let out = server.handle_line(&line).expect("response");
        assert!(out.contains("payload"), "{out}");
        assert_eq!(server.cx.store.count_kind("mcp_call").unwrap(), 1);
        assert_eq!(server.cx.store.count_call_io().unwrap(), 1);
        assert_eq!(server.cx.store.count_tokens().unwrap(), 3);
        let _ = fs::remove_dir_all(dir);
    }

    /// A failed call is an `isError` result carrying the message, never a success block.
    #[test]
    fn failed_call_sets_is_error() {
        let (cfg, dir) = tmp("iserr");
        let server = Server::new(&cfg).unwrap();
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#;
        let v: Value = serde_json::from_str(&server.handle_line(line).unwrap()).unwrap();
        assert_eq!(v["result"]["isError"], true, "{v}");
        assert_eq!(v["result"]["content"][0]["text"], "unknown tool: nope");
        let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"expand","arguments":{"id":"x","lines":"wat"}}}"#;
        let v: Value = serde_json::from_str(&server.handle_line(line).unwrap()).unwrap();
        assert_eq!(v["result"]["isError"], true, "{v}");
        let _ = fs::remove_dir_all(dir);
    }

    /// T192: `cfg.mcp.tools` allow-list filters listing and calls; `expand` stays
    /// listed unconditionally (D4 losslessness).
    #[test]
    fn tools_allow_list_filters_listing_and_calls() {
        let (mut cfg, dir) = tmp("allow");
        cfg.mcp.tools = vec!["read".to_string()];
        let server = Server::new(&cfg).unwrap();
        let mut names: Vec<String> = server.tools().iter().map(|t| t.name.to_string()).collect();
        names.sort();
        assert_eq!(names, ["expand", "read"]);
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search","arguments":{"pattern":"x"}}}"#;
        let v: Value = serde_json::from_str(&server.handle_line(line).unwrap()).unwrap();
        assert_eq!(v["result"]["isError"], true, "{v}");
        assert_eq!(v["result"]["content"][0]["text"], "unknown tool: search");
        let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"expand","arguments":{"id":"x"}}}"#;
        let v: Value = serde_json::from_str(&server.handle_line(line).unwrap()).unwrap();
        assert_eq!(v["result"]["isError"], true, "{v}");
        assert_eq!(v["result"]["content"][0]["text"], "unknown archive id: x");
        // The one-shot `call()` path (used by hosts that cannot speak MCP) rejects a
        // filtered name with the same text, never running it.
        let err = call(&cfg, "search", &json!({"pattern": "x"}))
            .unwrap_err()
            .to_string();
        assert_eq!(err, "unknown tool: search");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn one_shot_call_uses_the_same_invoke_as_tools_call() {
        let (cfg, dir) = tmp("oneshot");
        let err = call(&cfg, "nope", &json!({})).unwrap_err().to_string();
        assert_eq!(err, "unknown tool: nope");
        let server = Server::new(&cfg).unwrap();
        let id = server
            .cx
            .store
            .put_archive("mcp", b"payload", &cfg.core.archive_dir)
            .unwrap();
        let text = call(&cfg, "expand", &json!({"id": id})).unwrap();
        assert_eq!(text, "payload");
        let _ = fs::remove_dir_all(dir);
    }

    /// A JSON-RPC batch answers as one array; notifications inside it produce nothing.
    #[test]
    fn batch_answers_with_an_array() {
        let (cfg, dir) = tmp("batch");
        let server = Server::new(&cfg).unwrap();
        let line = r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":2,"method":"nope"}]"#;
        let v: Value = serde_json::from_str(&server.handle_line(line).unwrap()).unwrap();
        let arr = v.as_array().expect("array");
        assert_eq!(arr.len(), 2, "{v}");
        assert_eq!(arr[0]["id"], 1);
        assert_eq!(arr[1]["error"]["code"], -32601);
        assert!(server.handle_line("[]").unwrap().contains("-32600"));
        assert!(
            server
                .handle_line(r#"[{"jsonrpc":"2.0","method":"notifications/x"}]"#)
                .is_none()
        );
        let _ = fs::remove_dir_all(dir);
    }

    /// Each `rtok mcp` process is its own session; the surface tag stays "mcp".
    #[test]
    fn session_is_per_process() {
        let (cfg, dir) = tmp("sess");
        let server = Server::new(&cfg).unwrap();
        assert_eq!(server.cx.session, format!("mcp-{}", std::process::id()));
        let _ = fs::remove_dir_all(dir);
    }

    #[rstest]
    fn retention_runs_on_mcp_session_start() {
        let (mut cfg, dir) = tmp("mcp-retain");
        cfg.core.retain_calls_days = 1;
        {
            let store = Store::open(&cfg.core.db_path).unwrap();
            store
                .upsert_session("sess", Some(1), None, None, Some("mcp"))
                .unwrap();
            let call = store
                .insert_call("sess", "mcp", "mcp_call", Some(1), None, None, None, None)
                .unwrap();
            let body = vec![b'z'; 70 * 1024];
            store
                .insert_call_io(
                    call,
                    Some(&body),
                    None,
                    64 * 1024,
                    Some(&cfg.core.archive_dir),
                )
                .unwrap();
            store.set_call_ts(call, 0).unwrap();
        }
        let server = Server::new(&cfg).unwrap();
        server
            .cx
            .store
            .run_retention(cfg.core.retain_calls_days)
            .unwrap();
        assert_eq!(server.cx.store.count_calls().unwrap(), 0);
        let _ = fs::remove_dir_all(dir);
    }
}
