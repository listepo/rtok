//! `rtok doctor` (plan T1.4): hooks, MCP servers, proxy chain.

use crate::config::Config;
use crate::tokens::{self, Class};
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn run(cfg: &Config) -> Result<String> {
    let mut out = String::new();
    let settings = read_json(&cfg.doctor.settings_path);
    let hooks = count_hooks(settings.as_ref());
    out.push_str(&format!("hooks {}\n", hooks.total));
    for (ev, n) in &hooks.by_event {
        out.push_str(&format!("  {ev} {n}\n"));
    }
    out.push_str("mcp\n");
    let claude = read_json(&cfg.doctor.claude_json);
    let servers = mcp_servers(claude.as_ref(), Path::new(&cfg.doctor.mcp_json));
    for (name, cmd) in &servers {
        let (n_tools, desc_tokens) = list_tools(cmd, Duration::from_secs(2), &cfg.estimator);
        out.push_str(&format!(
            "  {name} ({n_tools} tools, ~{desc_tokens} desc tokens) {cmd}\n"
        ));
    }
    let chain = proxy_chain(
        settings.as_ref(),
        Duration::from_millis(cfg.doctor.probe_timeout_ms.max(300)),
    );
    out.push_str(&format!("proxy {}\n", chain));
    let base_owned = std::env::var("ANTHROPIC_BASE_URL").ok().or_else(|| {
        settings
            .as_ref()
            .and_then(|s| s.pointer("/env/ANTHROPIC_BASE_URL"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    if base_owned.as_deref().is_some_and(|b| !b.is_empty()) {
        out.push_str("mcp_tool_search likely disabled (ANTHROPIC_BASE_URL is set)\n");
    }
    let bash = std::env::var("BASH_MAX_OUTPUT_LENGTH").ok();
    out.push_str(&format!(
        "BASH_MAX_OUTPUT_LENGTH {}\n",
        bash.as_deref().unwrap_or("(unset)")
    ));
    let compact = settings
        .as_ref()
        .and_then(|s| s.get("autoCompactWindow"))
        .cloned();
    out.push_str(&format!(
        "autoCompactWindow {}\n",
        compact
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(unset)".into())
    ));
    Ok(out)
}

struct HookCount {
    total: usize,
    by_event: BTreeMap<String, usize>,
}

fn count_hooks(settings: Option<&Value>) -> HookCount {
    let mut c = HookCount {
        total: 0,
        by_event: BTreeMap::new(),
    };
    let Some(hooks) = settings
        .and_then(|s| s.get("hooks"))
        .and_then(Value::as_object)
    else {
        return c;
    };
    for (event, entries) in hooks {
        let n = match entries {
            Value::Array(a) => a.iter().map(inner_hook_count).sum(),
            _ => 0,
        };
        c.total += n;
        c.by_event.insert(event.clone(), n);
    }
    c
}

fn inner_hook_count(entry: &Value) -> usize {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(1)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn mcp_servers(claude: Option<&Value>, mcp_json: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for src in [claude, read_json(mcp_json).as_ref()] {
        let Some(map) = src
            .and_then(|v| v.get("mcpServers"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (name, spec) in map {
            let cmd = spec
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !out.iter().any(|(n, _)| n == name) {
                out.push((name.clone(), cmd));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn list_tools(cmd: &str, timeout: Duration, est: &crate::config::Estimator) -> (usize, u32) {
    if cmd.is_empty() || !Path::new(cmd).is_file() {
        return (0, 0);
    }
    let payload = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"rtok","version":"0.1.0"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        "\n"
    );
    let mut child = match Command::new(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return (0, 0);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => return (0, 0),
        }
    }
    let mut buf = Vec::new();
    if let Some(mut so) = child.stdout.take() {
        let _ = so.read_to_end(&mut buf);
    }
    let text = String::from_utf8_lossy(&buf);
    for line in text.lines().rev() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(tools) = v.pointer("/result/tools").and_then(Value::as_array) else {
            continue;
        };
        let tokens: u32 = tools
            .iter()
            .map(|t| {
                let d = t.get("description").and_then(Value::as_str).unwrap_or("");
                tokens::estimate(d, Class::Prose, est)
            })
            .sum();
        return (tools.len(), tokens);
    }
    (0, 0)
}

fn proxy_chain(settings: Option<&Value>, timeout: Duration) -> String {
    let mut hops = Vec::new();
    let mut url = settings
        .and_then(|s| s.pointer("/env/ANTHROPIC_BASE_URL"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok());
    let mut seen = 0;
    while let Some(u) = url.take() {
        if seen > 4 {
            break;
        }
        seen += 1;
        hops.push(hostport(&u));
        url = next_upstream(&u, timeout).filter(|n| !hops.iter().any(|h| n.contains(h)));
    }
    hops.join("→")
}

fn hostport(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split('/').next().unwrap_or(rest);
    host.trim_start_matches("127.0.0.1:").to_string()
}

fn next_upstream(url: &str, timeout: Duration) -> Option<String> {
    let body = http_get(url, "/health", timeout)?;
    let v: Value = serde_json::from_str(&body).ok()?;
    v.pointer("/checks/upstream/url")
        .or(v.pointer("/config/anthropic_api_url"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn http_get(base: &str, path: &str, timeout: Duration) -> Option<String> {
    let rest = base.split("://").nth(1).unwrap_or(base);
    let hostport = rest.split('/').next().unwrap_or(rest);
    let addr = hostport.to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {hostport}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let body = text.split("\r\n\r\n").nth(1)?;
    Some(body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_nested_hook_commands() {
        let s = json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command"},{"type":"command"}]},{"hooks":[{"type":"command"}]}]}});
        let c = count_hooks(Some(&s));
        assert_eq!(c.total, 3);
        assert_eq!(c.by_event["PreToolUse"], 3);
    }

    #[test]
    fn hostport_strips_loopback() {
        assert_eq!(hostport("http://127.0.0.1:8788"), "8788");
        assert_eq!(hostport("http://127.0.0.1:8787/w/claude"), "8787");
    }
}
