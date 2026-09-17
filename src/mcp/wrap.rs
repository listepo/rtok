//! `rtok mcp -- <server argv>` (plan T59.4): a lossless wrapper around a foreign stdio MCP
//! server. Every frame is forwarded as the peer wrote it, except the response to a
//! `tools/call` whose text blocks run past the `[mcp]` cmd rule (`Rule::default()` when the
//! user has none): each long block is archived raw, cut by `rules::apply`, and closed with
//! the same `expand` trailer `rtok run` prints. `tools/list`, prompts, resources, `isError`
//! results, notifications and anything that does not parse pass through untouched, and a
//! store that fails to open only turns the wrapper into a plain pipe (fail open, D4).
//! Both stdio framings are handled per frame: newline-delimited JSON (the MCP spec) and
//! `Content-Length` headers (LSP-style servers).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use rtok_plugin_sdk::{Archive, Class, Measurement};
use serde_json::Value;

use crate::config::Config;
use crate::plugin::Runtime;
use crate::plugins::cmd::rules::{self, Settings};

#[derive(Clone, Copy)]
enum Framing {
    Line,
    Header,
}

/// `tools/call` request ids the client sent and the server has not answered yet, with the
/// tool name each asked for. Keyed by the id's JSON text so `1` and `"1"` stay distinct.
type Pending = Arc<Mutex<HashMap<String, String>>>;

/// Spawn `argv`, pipe our stdio through it and return the server's exit code.
pub fn run(cfg: &Config, argv: &[String]) -> Result<i32> {
    let Some((bin, rest)) = argv.split_first() else {
        bail!("rtok mcp: missing server command after `--`");
    };
    let mut child = Command::new(bin)
        .args(rest)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut to_server = child.stdin.take().expect("piped stdin");
    let from_server = child.stdout.take().expect("piped stdout");
    let pending: Pending = Arc::default();
    let noted = Arc::clone(&pending);
    // Client → server: forward verbatim, note which ids are `tools/call`. Dropping the
    // child's stdin at our EOF is what asks the server to exit.
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut frame = Vec::new();
        while let Some(framing) = read_frame(&mut stdin, &mut frame) {
            if let Ok(v) = serde_json::from_slice::<Value>(&frame) {
                note_call(&noted, &v);
            }
            if write_frame(&mut to_server, framing, &frame).is_err() {
                break;
            }
        }
    });
    let runtime = Runtime::open(cfg.clone(), format!("mcp-wrap-{}", std::process::id())).ok();
    let settings = Settings::from_config(cfg);
    let server = Path::new(bin)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("mcp")
        .to_string();
    let mut stdout = std::io::stdout().lock();
    let mut reader = BufReader::new(from_server);
    let mut frame = Vec::new();
    while let Some(framing) = read_frame(&mut reader, &mut frame) {
        let short = runtime.as_ref().and_then(|cx| {
            let tool = take_call(&pending, &frame)?;
            shorten(cx, &settings, &server, &tool, &frame)
        });
        let body = short.as_deref().unwrap_or(&frame);
        if write_frame(&mut stdout, framing, body).is_err() {
            break;
        }
    }
    Ok(child.wait()?.code().unwrap_or(1))
}

/// One frame's JSON body without its framing. `None` at EOF or on a broken header.
fn read_frame(r: &mut impl BufRead, buf: &mut Vec<u8>) -> Option<Framing> {
    buf.clear();
    let first = loop {
        let head = r.fill_buf().ok()?;
        match head.first() {
            None => return None,
            Some(b) if b.is_ascii_whitespace() => r.consume(1),
            Some(b) => break *b,
        }
    };
    // A JSON frame starts with `{` or `[`; only a header block starts with a letter.
    if first != b'C' && first != b'c' {
        r.read_until(b'\n', buf).ok()?;
        while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
            buf.pop();
        }
        return Some(Framing::Line);
    }
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            len = value.trim().parse::<usize>().ok();
        }
    }
    buf.resize(len?, 0);
    r.read_exact(buf).ok()?;
    Some(Framing::Header)
}

fn write_frame(w: &mut impl Write, framing: Framing, body: &[u8]) -> std::io::Result<()> {
    match framing {
        Framing::Line => {
            w.write_all(body)?;
            w.write_all(b"\n")?;
        }
        Framing::Header => {
            write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
            w.write_all(body)?;
        }
    }
    w.flush()
}

fn note_call(pending: &Pending, v: &Value) {
    if v.get("method").and_then(Value::as_str) != Some("tools/call") {
        return;
    }
    let (Some(id), Some(tool)) = (
        v.get("id"),
        v.pointer("/params/name").and_then(Value::as_str),
    ) else {
        return;
    };
    if let Ok(mut p) = pending.lock() {
        p.insert(id.to_string(), tool.to_string());
    }
}

/// The tool name when `frame` answers a noted `tools/call`; the id is forgotten either way.
fn take_call(pending: &Pending, frame: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(frame).ok()?;
    let id = v.get("id")?.to_string();
    pending.lock().ok()?.remove(&id)
}

/// The response with every long text block archived and cut; `None` when nothing changed,
/// the result is an error, or the frame is not a tool result.
fn shorten(
    cx: &Runtime,
    settings: &Settings,
    server: &str,
    tool: &str,
    frame: &[u8],
) -> Option<Vec<u8>> {
    let mut v: Value = serde_json::from_slice(frame).ok()?;
    let result = v.get_mut("result")?;
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let rule = settings.pick("mcp");
    let mut changed = false;
    for block in result.get_mut("content")?.as_array_mut()? {
        let Some(text) = block.get("text").and_then(Value::as_str) else {
            continue;
        };
        let lines = text.lines().count() as u32;
        if lines <= rule.max_lines {
            continue;
        }
        let Ok(id) = cx.put_archive(text.as_bytes()) else {
            continue;
        };
        let cut = rules::apply(settings, text, 0, &rule, &id);
        if cut.len() >= text.len() {
            continue;
        }
        let printed = format!("{cut}\n[rtok {id} · {lines} lines · expand: rtok expand {id}]");
        let _ = cx.record(&Measurement {
            plugin: "cmd",
            kind: "wrap",
            before_bytes: text.len() as u64,
            after_bytes: printed.len() as u64,
            est_before: cx.estimate(text, Class::Code),
            est_after: cx.estimate(&printed, Class::Code),
            ref_id: Some(format!("{server}/{tool}:{id}")),
            call_id: None,
        });
        block["text"] = Value::String(printed);
        changed = true;
    }
    changed.then(|| serde_json::to_vec(&v).ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_both_framings_from_one_stream() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let mut stream = Vec::new();
        stream.extend_from_slice(b"\r\n");
        write_frame(&mut stream, Framing::Line, body).unwrap();
        write_frame(&mut stream, Framing::Header, body).unwrap();
        stream.extend_from_slice(b"content-length: 2\r\nX-Other: y\r\n\r\n{}");
        let mut r = Cursor::new(stream);
        let mut buf = Vec::new();
        assert!(matches!(read_frame(&mut r, &mut buf), Some(Framing::Line)));
        assert_eq!(buf, body);
        assert!(matches!(
            read_frame(&mut r, &mut buf),
            Some(Framing::Header)
        ));
        assert_eq!(buf, body);
        assert!(matches!(
            read_frame(&mut r, &mut buf),
            Some(Framing::Header)
        ));
        assert_eq!(buf, b"{}");
        assert!(read_frame(&mut r, &mut buf).is_none());
    }

    #[test]
    fn pending_matches_tool_call_ids_only() {
        let pending: Pending = Arc::default();
        note_call(
            &pending,
            &serde_json::json!({"id":7,"method":"tools/call","params":{"name":"grep"}}),
        );
        note_call(&pending, &serde_json::json!({"id":8,"method":"tools/list"}));
        assert_eq!(take_call(&pending, br#"{"id":8,"result":{}}"#), None);
        assert_eq!(take_call(&pending, br#"{"id":"7","result":{}}"#), None);
        assert_eq!(
            take_call(&pending, br#"{"id":7,"result":{}}"#).as_deref(),
            Some("grep")
        );
        assert_eq!(take_call(&pending, br#"{"id":7,"result":{}}"#), None);
    }
}
