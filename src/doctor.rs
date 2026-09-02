//! `rtok doctor` (plan T1.4): hooks, MCP servers, proxy chain.

use crate::config::Config;
use crate::tokens::{self, Class};
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
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
    let mut servers = mcp_servers(claude.as_ref(), Path::new(&cfg.doctor.mcp_json));
    // rtok's own MCP surface, so the P6/P8 gates compare like with like.
    // Not under test: current_exe would be the test binary.
    if !cfg!(test)
        && let Ok(exe) = std::env::current_exe()
    {
        servers.push(Server {
            name: "rtok".into(),
            cmd: exe.display().to_string(),
            args: vec!["mcp".into()],
            env: BTreeMap::new(),
        });
    }
    let timeout = Duration::from_millis(cfg.doctor.mcp_timeout_ms.max(500));
    for s in &servers {
        let (n_tools, desc_tokens) = list_tools(s, timeout, &cfg.estimator);
        out.push_str(&format!(
            "  {} ({n_tools} tools, ~{desc_tokens} desc tokens) {}\n",
            s.name, s.cmd
        ));
    }
    let timeout = Duration::from_millis(cfg.doctor.probe_timeout_ms.max(300));
    let anthropic = settings
        .as_ref()
        .and_then(|s| s.pointer("/env/ANTHROPIC_BASE_URL"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok());
    out.push_str(&format!("proxy {}\n", proxy_chain(anthropic, timeout)));
    out.push_str(&format!(
        "proxy openai {}\n",
        proxy_chain(openai_seed(cfg, settings.as_ref()), timeout)
    ));
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
    if cfg.doctor.instructions {
        out.push_str(&instruction_audit(cfg, settings.as_ref(), claude.as_ref()));
    }
    Ok(out)
}

const INJECTORS: &[&str] = &[
    "lean-ctx",
    "engram",
    "ponytail",
    "claude-mem",
    "token-optimizer",
    "caveman",
    "headroom",
];

struct Source {
    name: String,
    path: String,
    text: String,
}

fn instruction_audit(cfg: &Config, settings: Option<&Value>, claude: Option<&Value>) -> String {
    let mut srcs = Vec::new();
    if let Some(dir) = cfg.doctor.settings_path.parent() {
        push_file(&mut srcs, "claude-user", &dir.join("CLAUDE.md"));
    }
    if let Some(root) = git_root() {
        push_file(&mut srcs, "claude-project", &root.join("CLAUDE.md"));
        push_file(&mut srcs, "agents-project", &root.join("AGENTS.md"));
    }
    let blob = format!(
        "{}{}",
        settings.map(Value::to_string).unwrap_or_default(),
        claude.map(Value::to_string).unwrap_or_default()
    );
    for name in INJECTORS {
        if blob.contains(name) {
            let path = find_skill(name).unwrap_or_else(|| format!("mcp:{name}"));
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            srcs.push(Source {
                name: (*name).into(),
                path,
                text,
            });
        }
    }
    let mut out = String::from("instructions\n");
    let warn_at = cfg.doctor.instruction_warn_tokens;
    for s in &srcs {
        let n = tokens::estimate(&s.text, Class::Prose, &cfg.estimator);
        let flag = if n > warn_at { " WARN" } else { "" };
        out.push_str(&format!("  {} {n} tokens {}{flag}\n", s.name, s.path));
    }
    for (sent, names) in duplicates(&srcs) {
        out.push_str(&format!("  duplicate `{sent}` in {}\n", names.join(", ")));
    }
    out
}

fn push_file(srcs: &mut Vec<Source>, name: &str, path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let disp = path.display().to_string();
    if srcs.iter().any(|s| s.path == disp) {
        return;
    }
    srcs.push(Source {
        name: name.into(),
        path: disp,
        text,
    });
}

fn git_root() -> Option<std::path::PathBuf> {
    let mut p = std::env::current_dir().ok()?;
    loop {
        if p.join(".git").exists() {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

fn find_skill(name: &str) -> Option<String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let p = home.join(".claude/skills").join(name).join("SKILL.md");
    p.is_file().then(|| p.display().to_string())
}

fn duplicates(srcs: &[Source]) -> Vec<(String, Vec<String>)> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in srcs {
        for line in s.text.lines() {
            let t = line.trim();
            if t.len() < 40 {
                continue;
            }
            map.entry(t.to_string()).or_default().push(s.name.clone());
        }
    }
    map.into_iter()
        .filter_map(|(sent, mut names)| {
            names.sort();
            names.dedup();
            (names.len() > 1).then_some((sent.chars().take(60).collect(), names))
        })
        .collect()
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

/// One `mcpServers` entry. `args`/`env` matter: spawning the bare `command` made every
/// `uvx`/`npx`-launched server (serena, mobile, engram) report 0 tools.
struct Server {
    name: String,
    cmd: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

fn mcp_servers(claude: Option<&Value>, mcp_json: &Path) -> Vec<Server> {
    let mut out: Vec<Server> = Vec::new();
    for src in [claude, read_json(mcp_json).as_ref()] {
        let Some(map) = src
            .and_then(|v| v.get("mcpServers"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (name, spec) in map {
            if out.iter().any(|s| &s.name == name) {
                continue;
            }
            let args = spec
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            let env = spec
                .get("env")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect();
            out.push(Server {
                name: name.clone(),
                cmd: spec
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                args,
                env,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn list_tools(s: &Server, timeout: Duration, est: &crate::config::Estimator) -> (usize, u32) {
    if s.cmd.is_empty() {
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
    let mut child = match Command::new(&s.cmd)
        .args(&s.args)
        .envs(&s.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    // Keep stdin open until we have the answer: some servers exit on EOF before replying,
    // and slow starters (uvx, npx) need the full timeout rather than a wait-for-exit.
    let mut stdin = child.stdin.take();
    if let Some(si) = stdin.as_mut() {
        let _ = si.write_all(payload.as_bytes());
    }
    let (tx, rx) = mpsc::channel::<Value>();
    let stdout = child.stdout.take();
    std::thread::spawn(move || {
        let Some(so) = stdout else { return };
        for line in BufReader::new(so).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && let Some(tools) = v.pointer("/result/tools")
            {
                let _ = tx.send(tools.clone());
                return;
            }
        }
    });
    let tools = rx.recv_timeout(timeout).ok();
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let Some(tools) = tools.as_ref().and_then(Value::as_array) else {
        return (0, 0);
    };
    let tokens: u32 = tools
        .iter()
        .map(|t| {
            let d = t.get("description").and_then(Value::as_str).unwrap_or("");
            tokens::estimate(d, Class::Prose, est)
        })
        .sum();
    (tools.len(), tokens)
}

fn nonempty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.is_empty())
}

fn openai_seed(cfg: &Config, settings: Option<&Value>) -> Option<String> {
    nonempty(std::env::var("OPENAI_BASE_URL").ok())
        .or_else(|| {
            settings
                .and_then(|s| s.pointer("/env/OPENAI_BASE_URL"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            read_json(&cfg.setup.opencode.config_path)
                .as_ref()
                .and_then(|s| s.pointer("/env/OPENAI_BASE_URL"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            let doc: toml_edit::DocumentMut = std::fs::read_to_string(&cfg.setup.codex.config_path)
                .ok()?
                .parse()
                .ok()?;
            doc.get("model_providers")?
                .get("rtok")?
                .get("base_url")?
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

fn proxy_chain(seed: Option<String>, timeout: Duration) -> String {
    let mut hops = Vec::new();
    let mut url = nonempty(seed);
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
    fn mcp_servers_keep_args_and_env() {
        let c = json!({"mcpServers":{
            "serena":{"command":"/usr/bin/uvx","args":["--from","serena-agent","serena"],"env":{"A":"1"}},
            "bare":{"command":"x"}}});
        let s = mcp_servers(Some(&c), Path::new("/nonexistent/.mcp.json"));
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "bare");
        assert!(s[0].args.is_empty());
        assert_eq!(s[1].args, ["--from", "serena-agent", "serena"]);
        assert_eq!(s[1].env["A"], "1");
    }

    #[test]
    fn hostport_strips_loopback() {
        assert_eq!(hostport("http://127.0.0.1:8788"), "8788");
        assert_eq!(hostport("http://127.0.0.1:8787/w/claude"), "8787");
    }

    #[test]
    fn instructions_lists_four_injectors() {
        let dir = std::env::temp_dir().join("rtok-t72-instructions");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("CLAUDE.md"),
            "user claude md padding for a long enough line xx\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("claude.json"),
            r#"{"mcpServers":{"lean-ctx":{"command":"/bin/true"},"engram":{"command":"/bin/true"},"ponytail":{"command":"/bin/true"},"claude-mem":{"command":"/bin/true"}}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let mut cfg = Config::default();
        cfg.doctor.settings_path = dir.join("settings.json");
        cfg.doctor.claude_json = dir.join("claude.json");
        cfg.doctor.instructions = true;
        let s = run(&cfg).unwrap();
        assert!(s.contains("instructions"), "{s}");
        let n = s
            .lines()
            .filter(|l| l.starts_with("  ") && l.contains("tokens"))
            .count();
        assert!(n >= 4, "{s}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lists_anthropic_and_openai_proxy_chains() {
        let dir = std::env::temp_dir().join(format!("rtok-t115-doctor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:8790"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("opencode.json"),
            r#"{"env":{"OPENAI_BASE_URL":"http://127.0.0.1:8790/v1"}}"#,
        )
        .unwrap();
        let mut cfg = Config::default();
        cfg.doctor.settings_path = dir.join("settings.json");
        cfg.doctor.claude_json = dir.join("missing-claude.json");
        cfg.doctor.mcp_json = dir.join("missing-mcp.json");
        cfg.setup.opencode.config_path = dir.join("opencode.json");
        cfg.setup.codex.config_path = dir.join("missing-codex.toml");
        let s = run(&cfg).unwrap();
        assert!(
            s.lines().any(|l| l.starts_with("proxy ")
                && !l.starts_with("proxy openai")
                && l.contains("8790")),
            "{s}"
        );
        assert!(
            s.lines()
                .any(|l| l.starts_with("proxy openai ") && l.contains("8790")),
            "{s}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
