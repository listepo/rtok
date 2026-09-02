//! PreCompact checkpoint + compact restore (plan T2.5).

use crate::plugin::{Ctx, Injection};
use crate::tokens::Class;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// Parsed compact snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkpoint {
    pub prompts: Vec<String>,
    pub paths: Vec<String>,
    pub errors: Vec<String>,
}

impl Checkpoint {
    pub fn render(&self) -> String {
        let mut s = String::from("checkpoint\n");
        for p in &self.prompts {
            s.push_str("- ");
            s.push_str(p);
            s.push('\n');
        }
        for p in &self.paths {
            s.push_str("path ");
            s.push_str(p);
            s.push('\n');
        }
        for e in &self.errors {
            s.push_str("err ");
            s.push_str(e);
            s.push('\n');
        }
        s
    }
}

/// Last 20 user prompts (≤ 300 chars), file paths, and error lines from a JSONL transcript.
pub fn extract(jsonl: &str) -> Checkpoint {
    let mut prompts = Vec::new();
    let mut paths = BTreeSet::new();
    let mut errors = Vec::new();
    for line in jsonl.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        walk(&v, &mut paths, &mut |s| {
            if errors.len() < 8 && s.to_ascii_lowercase().contains("error") {
                errors.push(s.chars().take(200).collect());
            }
        });
        if let Some(p) = user_prompt(&v) {
            prompts.push(p);
        }
    }
    if prompts.len() > 20 {
        prompts = prompts.split_off(prompts.len() - 20);
    }
    Checkpoint {
        prompts,
        paths: paths.into_iter().collect(),
        errors,
    }
}

fn user_prompt(v: &Value) -> Option<String> {
    if v.get("type").and_then(Value::as_str) != Some("user") {
        return None;
    }
    let c = v.pointer("/message/content")?;
    let raw = match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter_map(|b| {
                if b.get("type").and_then(Value::as_str) != Some("text") {
                    return None;
                }
                b.get("text").and_then(Value::as_str).map(str::to_string)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.chars().take(300).collect())
    }
}

fn walk(v: &Value, paths: &mut BTreeSet<String>, text: &mut impl FnMut(&str)) {
    match v {
        Value::String(s) => text(s),
        Value::Object(m) => {
            if let Some(p) = m.get("file_path").and_then(Value::as_str)
                && !p.is_empty()
            {
                paths.insert(p.to_string());
            }
            for c in m.values() {
                walk(c, paths, text);
            }
        }
        Value::Array(a) => {
            for c in a {
                walk(c, paths, text);
            }
        }
        _ => {}
    }
}

/// Read `transcript_path`, store a `notes` row `kind=checkpoint`.
pub fn save(transcript_path: &str, cx: &Ctx) -> anyhow::Result<Checkpoint> {
    let cp = extract(&std::fs::read_to_string(Path::new(transcript_path)).unwrap_or_default());
    cx.store
        .insert_note(Some("rtok"), "checkpoint", "compact", &cp.render())?;
    Ok(cp)
}

/// Latest checkpoint as an injection, capped at `plugins.memory.checkpoint_tokens`.
pub fn offer(cx: &Ctx) -> Option<Injection> {
    let mut text = cx.store.latest_note("checkpoint").ok().flatten()?;
    let cap = cx.config.plugins.memory.checkpoint_tokens.max(1);
    while cx.estimate(&text, Class::Prose) > cap && !text.is_empty() {
        text.pop();
    }
    (!text.is_empty()).then_some(Injection {
        plugin: "inject",
        text,
        priority: 9,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{"type":"user","message":{"role":"user","content":"edit the three files"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/a.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"src/b.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"src/c.rs"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"still failing with error: boom"}]}}
"#;

    #[test]
    fn fixture_has_three_paths_and_compact_injects_under_budget() {
        let cp = extract(FIXTURE);
        assert_eq!(cp.paths, ["src/a.rs", "src/b.rs", "src/c.rs"]);
        let dir = std::env::temp_dir().join("rtok-t25-fixture-has-three-paths");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.jsonl"), FIXTURE).unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        let pre = serde_json::json!({"hook_event_name":"PreCompact","session_id":"t25","transcript_path":dir.join("t.jsonl").to_str().unwrap(),"trigger":"auto"});
        let mut out = Vec::new();
        crate::hooks::run("PreCompact", pre.to_string().as_bytes(), &mut out, &cfg);
        assert_eq!(out, b"{}");
        let body = crate::store::Store::open(&cfg.core.db_path)
            .unwrap()
            .latest_note("checkpoint")
            .unwrap()
            .expect("note");
        let start = serde_json::json!({"hook_event_name":"SessionStart","session_id":"t25","source":"compact"});
        out.clear();
        crate::hooks::run("SessionStart", start.to_string().as_bytes(), &mut out, &cfg);
        let text = serde_json::from_slice::<serde_json::Value>(&out).unwrap()["hookSpecificOutput"]
            ["additionalContext"]
            .as_str()
            .unwrap()
            .to_string();
        let cx = Ctx::in_memory("budget").unwrap();
        for p in ["src/a.rs", "src/b.rs", "src/c.rs"] {
            assert!(body.contains(p) && text.contains(p), "{body}\n{text}");
        }
        assert!(cx.estimate(&text, Class::Prose) <= cx.config.plugins.memory.checkpoint_tokens);
    }
}
