//! `rtok mcp` — rmcp JSON-RPC over stdio (plan T4.1).
//!
//! A one-shot `tools/list` (the Check) is accepted without `initialize`.

use std::io::{BufRead, Write};
use std::sync::Arc;

use anyhow::Result;
use rmcp::model::{CallToolResult, Content, JsonObject, ListToolsResult, ServerInfo, Tool};
use serde_json::{Value, json};

use crate::config::Config;
use crate::plugin::{Ctx, ToolDef};
use crate::plugins::Registry;
use crate::tokens::Class;

fn expand_def() -> ToolDef {
    ToolDef {
        name: "expand",
        description: "Return archived payload by id; optional lines a-b and grep.",
        input_schema: json!({"type":"object","properties":{"id":{"type":"string"},"lines":{"type":"string"},"grep":{"type":"string"}},"required":["id"]}),
    }
}

/// Serve MCP on stdin/stdout until EOF.
pub fn run(cfg: &Config) -> Result<()> {
    let server = Server::new(cfg)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(out) = server.handle_line(&line) {
            writeln!(stdout, "{out}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

struct Listed {
    plugin: &'static str,
    def: ToolDef,
}

struct Server {
    cx: Ctx,
    listed: Vec<Listed>,
}

impl Server {
    fn new(cfg: &Config) -> Result<Self> {
        let cx = Ctx::open(cfg.clone(), "mcp")?;
        let mut listed = vec![Listed {
            plugin: "archive",
            def: expand_def(),
        }];
        for p in Registry::new(cfg).enabled() {
            let id = p.manifest().id;
            for def in p.mcp_tools() {
                if listed.iter().any(|t| t.def.name == def.name) {
                    continue;
                }
                listed.push(Listed { plugin: id, def });
            }
        }
        Ok(Self { cx, listed })
    }

    fn tools(&self) -> Vec<Tool> {
        self.listed.iter().map(|t| to_tool(&t.def)).collect()
    }

    fn handle_line(&self, line: &str) -> Option<String> {
        let req: Value = serde_json::from_str(line).ok()?;
        let method = req["method"].as_str().unwrap_or("");
        if req.get("id").is_none() || method.starts_with("notifications/") {
            return None;
        }
        let id = req["id"].clone();
        let result = match method {
            "initialize" => {
                let mut info = ServerInfo::default();
                info.capabilities.tools = Some(rmcp::model::ToolsCapability::default());
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
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":method}})
                        .to_string(),
                );
            }
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}).to_string())
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
        let text = invoke(&self.cx, name, &args);
        let _ = record(&self.cx, plugin, name, &args, &text);
        CallToolResult::success(vec![Content::text(text)])
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

fn invoke(cx: &Ctx, name: &str, args: &Value) -> String {
    match name {
        "expand" => expand_text(cx, args),
        #[cfg(feature = "memory")]
        "mem_save" => mem_save(cx, args),
        #[cfg(feature = "memory")]
        "mem_search" => mem_search(cx, args),
        #[cfg(feature = "memory")]
        "mem_get" => mem_get(cx, args),
        #[cfg(feature = "read")]
        "read" => read_file(cx, args),
        #[cfg(feature = "read")]
        "search" => search_files(cx, args),
        #[cfg(feature = "read")]
        "tree" => tree_files(cx, args),
        #[cfg(feature = "graph")]
        "symbol" | "callers" | "outline" => crate::plugins::graph::call(cx, name, args),
        _ => format!("unknown tool: {name}"),
    }
}

fn expand_text(cx: &Ctx, args: &Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    match crate::expand::fetch(cx, id) {
        Ok(Some(bytes)) => {
            let text = String::from_utf8_lossy(&bytes);
            slice(&text, args["lines"].as_str(), args["grep"].as_str())
        }
        Ok(None) => format!("unknown archive id: {id}"),
        Err(e) => e.to_string(),
    }
}

fn slice(text: &str, lines: Option<&str>, grep: Option<&str>) -> String {
    let mut out: Vec<&str> = text.lines().collect();
    if let Some(spec) = lines
        && let Some((a, b)) = spec.split_once('-')
        && let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>())
    {
        out = out.into_iter().take(b).skip(a.saturating_sub(1)).collect();
    }
    if let Some(g) = grep {
        out.retain(|l| l.contains(g));
    }
    out.join("\n")
}

#[cfg(feature = "memory")]
fn mem_save(cx: &Ctx, args: &Value) -> String {
    let kind = args["kind"].as_str().unwrap_or("note");
    let title = args["title"].as_str().unwrap_or("");
    let body = args["body"].as_str().unwrap_or("");
    let project = args["project"].as_str();
    match crate::plugins::memory::mem_save(cx, kind, title, body, project) {
        Ok(id) => json!({"id": id}).to_string(),
        Err(e) => e.to_string(),
    }
}

#[cfg(feature = "memory")]
fn mem_search(cx: &Ctx, args: &Value) -> String {
    let query = args["query"].as_str().unwrap_or("");
    let limit = args["limit"].as_u64().unwrap_or(5) as u32;
    match crate::plugins::memory::mem_search(cx, query, limit) {
        Ok(hits) => json!(
            hits.iter()
                .map(|h| json!({"id": h.id, "title": h.title, "snippet": h.snippet}))
                .collect::<Vec<_>>()
        )
        .to_string(),
        Err(e) => e.to_string(),
    }
}

#[cfg(feature = "read")]
fn search_files(cx: &Ctx, args: &Value) -> String {
    let pattern = args["pattern"].as_str().unwrap_or("");
    let path = args["path"].as_str().unwrap_or(".");
    let max = args["max"].as_u64().map(|n| n as u32);
    match crate::plugins::read::search::search(cx, pattern, path, max) {
        Ok(s) => s,
        Err(e) => e.to_string(),
    }
}

#[cfg(feature = "read")]
fn tree_files(cx: &Ctx, args: &Value) -> String {
    let path = args["path"].as_str().unwrap_or(".");
    let depth = args["depth"].as_u64().map(|n| n as u32);
    match crate::plugins::read::search::tree(cx, path, depth) {
        Ok(s) => s,
        Err(e) => e.to_string(),
    }
}

#[cfg(feature = "read")]
fn read_file(cx: &Ctx, args: &Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let mode = args["mode"].as_str().unwrap_or("");
    let range = args["range"].as_str();
    match crate::plugins::read::read(cx, path, mode, range) {
        Ok(s) => s,
        Err(e) => e.to_string(),
    }
}

#[cfg(feature = "memory")]
fn mem_get(cx: &Ctx, args: &Value) -> String {
    let id = args["id"].as_i64().unwrap_or(0) as i32;
    match crate::plugins::memory::mem_get(cx, id) {
        Ok(Some(body)) => body,
        Ok(None) => format!("unknown note id: {id}"),
        Err(e) => e.to_string(),
    }
}

fn record(cx: &Ctx, plugin: &str, name: &str, args: &Value, result: &str) -> Result<()> {
    let args_s = args.to_string();
    let before = i64::from(cx.estimate(&args_s, Class::Json));
    let after = i64::from(cx.estimate(result, Class::Json));
    let host = cx.store.host_id(&cx.config.hook.host)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tokens::Class;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-mcp-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        (c, dir)
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
}
