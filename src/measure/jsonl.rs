//! Claude Code transcript parser (plan T1.1).
//!
//! Port of the `scratchpad/token-research/measure_sessions.py` logic: one JSON object
//! per line; skip malformed lines and count them. Turn index increments on each
//! `user` / `assistant` message.

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
    pub turn: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub turn: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
    pub output_tokens: u32,
    pub turn: u32,
}

#[derive(Debug, Default)]
pub struct Parsed {
    pub lines: u64,
    pub malformed: u64,
    pub tool_uses: Vec<ToolUse>,
    pub tool_results: Vec<ToolResult>,
    pub assistant_texts: Vec<String>,
    pub usages: Vec<Usage>,
    pub turns: u32,
}

pub fn parse_jsonl(text: &str) -> Parsed {
    parse_lines(text.lines())
}

pub fn parse_path(path: &Path) -> std::io::Result<Parsed> {
    let f = std::fs::File::open(path)?;
    Ok(parse_lines(BufReader::new(f).lines().map_while(Result::ok)))
}

/// Walk `dir` recursively for `*.jsonl`. Used to assert 0 parse failures on real logs.
pub fn parse_dir(dir: &Path) -> std::io::Result<Parsed> {
    let mut acc = Parsed::default();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let rd = match std::fs::read_dir(&d) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                let one = parse_path(&p)?;
                acc.lines += one.lines;
                acc.malformed += one.malformed;
                acc.turns += one.turns;
                acc.tool_uses.extend(one.tool_uses);
                acc.tool_results.extend(one.tool_results);
                acc.assistant_texts.extend(one.assistant_texts);
                acc.usages.extend(one.usages);
            }
        }
    }
    Ok(acc)
}

fn parse_lines<I, S>(lines: I) -> Parsed
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = Parsed::default();
    for line in lines {
        let line = line.as_ref().trim();
        if line.is_empty() {
            continue;
        }
        out.lines += 1;
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                out.malformed += 1;
                continue;
            }
        };
        ingest(&v, &mut out);
    }
    out
}

fn ingest(v: &Value, out: &mut Parsed) {
    let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
    let is_turn = ty == "user" || ty == "assistant";
    if is_turn {
        out.turns += 1;
    }
    let turn = out.turns.saturating_sub(1);
    let msg = v.get("message").unwrap_or(v);
    if let Some(u) = usage_of(msg).or_else(|| usage_of(v))
        && is_turn
    {
        out.usages.push(Usage { turn, ..u });
    }
    match msg.get("content") {
        Some(Value::String(s)) if ty == "assistant" && !s.is_empty() => {
            out.assistant_texts.push(s.clone());
        }
        Some(Value::Array(blocks)) => {
            for b in blocks {
                ingest_block(b, ty, turn, out);
            }
        }
        _ => {}
    }
}

fn ingest_block(b: &Value, ty: &str, turn: u32, out: &mut Parsed) {
    match b.get("type").and_then(Value::as_str) {
        Some("tool_use") => {
            let id = b
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let name = b
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let input = b.get("input").cloned().unwrap_or(Value::Null);
            out.tool_uses.push(ToolUse {
                id,
                name,
                input,
                turn,
            });
        }
        Some("tool_result") => {
            let tool_use_id = b
                .get("tool_use_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            out.tool_results.push(ToolResult {
                tool_use_id,
                content: flatten_content(b.get("content")),
                turn,
            });
        }
        Some("text") if ty == "assistant" => {
            if let Some(t) = b.get("text").and_then(Value::as_str)
                && !t.is_empty()
            {
                out.assistant_texts.push(t.to_string());
            }
        }
        _ => {}
    }
}

fn usage_of(v: &Value) -> Option<Usage> {
    let u = v.get("usage")?.as_object()?;
    Some(Usage {
        input_tokens: num(&u.get("input_tokens")),
        cache_creation_input_tokens: num(&u.get("cache_creation_input_tokens")),
        cache_read_input_tokens: num(&u.get("cache_read_input_tokens")),
        output_tokens: num(&u.get("output_tokens")),
        turn: 0,
    })
}

fn num(v: &Option<&Value>) -> u32 {
    v.and_then(Value::as_u64).unwrap_or(0) as u32
}

fn flatten_content(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| {
                x.as_str()
                    .map(str::to_string)
                    .or_else(|| x.get("text").and_then(Value::as_str).map(str::to_string))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture_200() -> (String, Expected) {
        let mut lines = Vec::new();
        for i in 0..40 {
            lines.push(
                json!({
                    "type": "assistant",
                    "message": {
                        "role": "assistant",
                        "content": [
                            {"type": "text", "text": format!("ok {i}")},
                            {"type": "tool_use", "id": format!("t{i}"), "name": "Bash", "input": {"command": "ls"}}
                        ],
                        "usage": {
                            "input_tokens": 10,
                            "cache_creation_input_tokens": 20,
                            "cache_read_input_tokens": 30,
                            "output_tokens": 40
                        }
                    }
                })
                .to_string(),
            );
            lines.push(
                json!({
                    "type": "user",
                    "message": {
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": format!("t{i}"),
                            "content": format!("out {i}")
                        }]
                    }
                })
                .to_string(),
            );
        }
        for i in 0..40 {
            lines.push(
                json!({
                    "type": "assistant",
                    "message": {
                        "role": "assistant",
                        "content": [{"type": "text", "text": format!("hi {i}")}],
                        "usage": {"input_tokens": 1, "output_tokens": 2}
                    }
                })
                .to_string(),
            );
        }
        for i in 0..60 {
            lines.push(json!({"type": "attachment", "i": i}).to_string());
        }
        for i in 0..20 {
            lines.push(format!("not-json {i}"));
        }
        assert_eq!(lines.len(), 200);
        (
            lines.join("\n"),
            Expected {
                lines: 200,
                malformed: 20,
                tool_uses: 40,
                tool_results: 40,
                assistant_texts: 80,
                usages: 80,
                turns: 120,
            },
        )
    }

    struct Expected {
        lines: u64,
        malformed: u64,
        tool_uses: usize,
        tool_results: usize,
        assistant_texts: usize,
        usages: usize,
        turns: u32,
    }

    #[test]
    fn fixture_200_expected_counts() {
        let (text, exp) = fixture_200();
        let p = parse_jsonl(&text);
        assert_eq!(p.lines, exp.lines);
        assert_eq!(p.malformed, exp.malformed);
        assert_eq!(p.tool_uses.len(), exp.tool_uses);
        assert_eq!(p.tool_results.len(), exp.tool_results);
        assert_eq!(p.assistant_texts.len(), exp.assistant_texts);
        assert_eq!(p.usages.len(), exp.usages);
        assert_eq!(p.turns, exp.turns);
        assert_eq!(p.tool_uses[0].id, "t0");
        assert_eq!(p.tool_uses[0].name, "Bash");
        assert_eq!(p.tool_results[0].tool_use_id, "t0");
        assert_eq!(p.tool_results[0].content, "out 0");
        assert_eq!(p.usages[0].cache_read_input_tokens, 30);
        assert_eq!(p.tool_uses[0].turn, 0);
        assert_eq!(p.tool_results[0].turn, 1);
    }

    #[test]
    fn skips_malformed_and_counts_them() {
        let p = parse_jsonl("{\n{}\n");
        assert_eq!(p.lines, 2);
        assert_eq!(p.malformed, 1);
    }

    #[test]
    fn real_claude_projects_zero_parse_failures() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let dir = Path::new(&home).join(".claude/projects/-Users-listepo-GitHub-rtok");
        if !dir.is_dir() {
            return;
        }
        let p = parse_dir(&dir).expect("read jsonl");
        assert!(p.lines > 0, "expected jsonl under {}", dir.display());
        assert_eq!(
            p.malformed, 0,
            "{} parse failures in {} lines",
            p.malformed, p.lines
        );
    }
}
