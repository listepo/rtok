//! T128: sub-agent transcripts — `<session>/subagents/agent-<id>.jsonl`, plus its
//! `agent-<id>.meta.json` (`agentType`, `model`, `toolUseId`) — attributed to the parent
//! session (research.md §17.1). `stats::collect`'s recursive walk used to pick these up
//! as sessions of their own; now it skips them and they are counted here exactly once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::jsonl;
use super::stats::{pct, same_path, tool_path};

/// One `(agentType, model)` split row. The meta fields are optional in the wild.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentRow {
    pub agent_type: String,
    pub model: String,
    pub count: u64,
    pub result_bytes: u64,
    pub read_bytes: u64,
    pub reread_bytes: u64,
}

/// The `subagents` row: what the ad-hoc §17.1 scan counted, now as a `rtok stats` row.
/// A re-read is a sub-agent `Read` of a path the parent read or an earlier sibling read;
/// parent-first, so the two re-read columns are disjoint and sum to [`Self::reread_bytes`].
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subagents {
    /// Parent sessions with at least one sub-agent.
    pub sessions: u64,
    pub count: u64,
    /// Tool-result bytes: the sub-agents, and their parents' whole transcripts.
    pub result_bytes: u64,
    pub parent_result_bytes: u64,
    /// Native `Read` result bytes in the sub-agents…
    pub read_bytes: u64,
    /// … of a path the parent also read (any time in its session),
    pub read_parent: u64,
    /// … of a path an earlier sibling read and the parent did not.
    pub read_sibling: u64,
    pub reread_bytes: u64,
    /// T179: how many of those re-reads there were — `repeat_reads`'s `subagent`
    /// class denominator ([`super::stats::RepeatReadsRow`]).
    pub reread_calls: u64,
    /// The three shares of the §17.1 table: re-read over sub-agent read bytes, over
    /// sub-agent tool-result bytes, and over the tree's (sub-agents + parents).
    pub share_read: f64,
    pub share_results: f64,
    pub share_tree: f64,
    pub usage_input: u64,
    pub usage_cache_read: u64,
    pub usage_cache_write: u64,
    pub usage_output: u64,
    /// Split by `agentType` and `model` from the meta files (`unknown` / `-` when absent).
    pub by_type: Vec<SubagentRow>,
    /// T131: the same read/re-read/share/input-token split, by whether this sub-agent's
    /// first user message carried the `SubagentStart` spawn brief (`has_spawn_brief`).
    pub with_brief: BriefSplitRow,
    pub without_brief: BriefSplitRow,
}

impl Subagents {
    fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// T131: one "spawned with/without a brief" split — read/re-read bytes, the re-read share,
/// and raw input tokens, so a `SubagentStart` spawn brief's on/off difference in re-read
/// share shows up next to T130's cost `Measurement` row (a saving is not real without one).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefSplitRow {
    pub count: u64,
    pub read_bytes: u64,
    pub reread_bytes: u64,
    pub share_read: f64,
    pub usage_input: u64,
}

/// T131: substring of `memory::handoff::INSTRUCTIONS` that marks a `SubagentStart` spawn
/// brief. `measure` and `memory` live in the same crate (`web::model` already uses
/// `measure::stats` unconditionally, so `measure` is always compiled), so this is a real
/// constant, not a mirrored literal — but it still must stay a substring of `INSTRUCTIONS`;
/// `handoff.rs`'s own test pins that so a reworded brief fails loud instead of silently
/// zeroing the "with brief" split.
pub const SPAWN_BRIEF_MARKER: &str = "Get the archived body with rtok expand";

/// T131: does this sub-agent's first user-role message (`jsonl::Parsed::first_user_text`)
/// carry the spawn brief's fixed instructions ([`SPAWN_BRIEF_MARKER`])?
pub(crate) fn has_spawn_brief(first_user_text: &str) -> bool {
    first_user_text.contains(SPAWN_BRIEF_MARKER)
}

/// What the walk needs from one parent transcript: its sidecar directory (the transcript
/// stem), every path a native `Read` named, the spawn order of `Agent`/`Task` calls (the
/// meta's `toolUseId` indexes it), and its tool-result bytes for the tree denominator.
pub(crate) struct Parent {
    sidecar: PathBuf,
    read_paths: Vec<String>,
    spawn_order: BTreeMap<String, usize>,
    result_bytes: u64,
}

impl Parent {
    pub(crate) fn new(transcript: &Path, parsed: &jsonl::Parsed) -> Self {
        let mut read_paths = Vec::new();
        let mut spawn_order = BTreeMap::new();
        for u in &parsed.tool_uses {
            match u.name.as_str() {
                "Read" => {
                    if let Some(p) = tool_path(&u.input) {
                        read_paths.push(p.to_string());
                    }
                }
                "Agent" | "Task" => {
                    spawn_order.insert(u.id.clone(), spawn_order.len());
                }
                _ => {}
            }
        }
        Self {
            sidecar: transcript.with_extension(""),
            read_paths,
            spawn_order,
            result_bytes: parsed
                .tool_results
                .iter()
                .map(|r| r.content.len() as u64)
                .sum(),
        }
    }
}

/// True under a `subagents/` directory: the session walk skips these (never a session of
/// their own), and [`collect`] attributes them to the parent instead.
pub(crate) fn is_subagent(path: &Path) -> bool {
    path.parent()
        .and_then(|d| d.file_name())
        .is_some_and(|n| n == "subagents")
}

/// Every `<stem>/subagents/agent-*.jsonl` under `cutoff`, attributed to its parent.
/// `None` when no parent had a sub-agent, so the report's goldens hold.
pub(crate) fn collect(parents: &[Parent], cutoff: SystemTime) -> Option<Subagents> {
    let mut out = Subagents::default();
    let mut by_type: BTreeMap<(String, String), SubagentRow> = BTreeMap::new();
    for parent in parents {
        let mut agents: Vec<(usize, String, PathBuf, String, String)> = Vec::new();
        for p in super::codex::jsonl_paths(&parent.sidecar.join("subagents"), cutoff) {
            let stem = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            let (agent_type, model, order) = meta(&p, &parent.spawn_order);
            agents.push((order, stem, p, agent_type, model));
        }
        if agents.is_empty() {
            continue;
        }
        out.sessions += 1;
        out.parent_result_bytes += parent.result_bytes;
        // Spawn order decides "an earlier sibling": the parent's `Agent`/`Task` call
        // order via the meta's `toolUseId`, unknown metas last by name.
        agents.sort();
        let mut sibling_paths: Vec<String> = Vec::new();
        for (_, _, path, agent_type, model) in agents {
            let Ok(parsed) = jsonl::parse_path(&path) else {
                continue;
            };
            out.count += 1;
            let with_brief = parsed
                .first_user_text
                .as_deref()
                .is_some_and(has_spawn_brief);
            let mut agent_read_bytes = 0u64;
            let mut agent_reread_bytes = 0u64;
            let mut agent_usage_input = 0u64;
            let mut read_paths: Vec<String> = Vec::new();
            let mut id_name: BTreeMap<&str, &str> = BTreeMap::new();
            let mut id_path: BTreeMap<&str, &str> = BTreeMap::new();
            for u in &parsed.tool_uses {
                id_name.insert(u.id.as_str(), u.name.as_str());
                if u.name == "Read"
                    && let Some(p) = tool_path(&u.input)
                {
                    id_path.insert(u.id.as_str(), p);
                    read_paths.push(p.to_string());
                }
            }
            let row = by_type.entry((agent_type, model)).or_default();
            row.count += 1;
            for r in &parsed.tool_results {
                let bytes = r.content.len() as u64;
                out.result_bytes += bytes;
                row.result_bytes += bytes;
                if id_name.get(r.tool_use_id.as_str()) != Some(&"Read") {
                    continue;
                }
                out.read_bytes += bytes;
                row.read_bytes += bytes;
                agent_read_bytes += bytes;
                let Some(path) = id_path.get(r.tool_use_id.as_str()) else {
                    continue;
                };
                // Parent first: the two re-read columns stay disjoint and sum to the total.
                if parent.read_paths.iter().any(|p| same_path(p, path)) {
                    out.read_parent += bytes;
                    out.reread_bytes += bytes;
                    out.reread_calls += 1;
                    row.reread_bytes += bytes;
                    agent_reread_bytes += bytes;
                } else if sibling_paths.iter().any(|p| same_path(p, path)) {
                    out.read_sibling += bytes;
                    out.reread_bytes += bytes;
                    out.reread_calls += 1;
                    row.reread_bytes += bytes;
                    agent_reread_bytes += bytes;
                }
            }
            for u in &parsed.usages {
                out.usage_input += u64::from(u.input_tokens);
                out.usage_cache_read += u64::from(u.cache_read_input_tokens);
                out.usage_cache_write += u64::from(u.cache_creation_input_tokens);
                out.usage_output += u64::from(u.output_tokens);
                agent_usage_input += u64::from(u.input_tokens);
            }
            let split = if with_brief {
                &mut out.with_brief
            } else {
                &mut out.without_brief
            };
            split.count += 1;
            split.read_bytes += agent_read_bytes;
            split.reread_bytes += agent_reread_bytes;
            split.usage_input += agent_usage_input;
            sibling_paths.extend(read_paths);
        }
    }
    if out.is_empty() {
        return None;
    }
    out.share_read = pct(out.reread_bytes, out.read_bytes);
    out.share_results = pct(out.reread_bytes, out.result_bytes);
    out.share_tree = pct(out.reread_bytes, out.result_bytes + out.parent_result_bytes);
    out.with_brief.share_read = pct(out.with_brief.reread_bytes, out.with_brief.read_bytes);
    out.without_brief.share_read =
        pct(out.without_brief.reread_bytes, out.without_brief.read_bytes);
    out.by_type = by_type
        .into_iter()
        .map(|((agent_type, model), mut row)| {
            row.agent_type = agent_type;
            row.model = model;
            row
        })
        .collect();
    Some(out)
}

/// `(agentType, model, spawn order)` from the sidecar meta; unknown metas sort last.
fn meta(jsonl_path: &Path, spawn_order: &BTreeMap<String, usize>) -> (String, String, usize) {
    let path = jsonl_path.with_extension("meta.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return ("unknown".into(), "-".into(), usize::MAX);
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return ("unknown".into(), "-".into(), usize::MAX);
    };
    let agent_type = v
        .get("agentType")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let model = v
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let order = v
        .get("toolUseId")
        .and_then(Value::as_str)
        .and_then(|id| spawn_order.get(id).copied())
        .unwrap_or(usize::MAX);
    (agent_type, model, order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(v: Value) -> String {
        v.to_string()
    }

    /// One parent + two sub-agents with overlapping reads: the fixture asserts the three
    /// §17.1 shares and the parent-first split of the two re-read columns.
    #[test]
    fn one_parent_and_two_siblings_assert_the_three_shares() {
        let dir = std::env::temp_dir().join(format!(
            "rtok-t128-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let subs = dir.join("sess").join("subagents");
        std::fs::create_dir_all(&subs).unwrap();
        // Parent: reads a.rs and b.rs, then spawns t1 and t2. Its own results are 10 B.
        let parent: Vec<Value> = vec![
            json!({"type":"user","message":{"role":"user","content":"go"}}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"tool_use","id":"t0a","name":"Read","input":{"file_path":"a.rs"}},
                {"type":"tool_use","id":"t0b","name":"Read","input":{"file_path":"b.rs"}},
                {"type":"tool_use","id":"t1","name":"Agent","input":{"prompt":"one"}},
                {"type":"tool_use","id":"t2","name":"Agent","input":{"prompt":"two"}}],
                "usage":{"input_tokens":1,"output_tokens":1}}}),
            json!({"type":"user","message":{"role":"user","content":[
                {"type":"tool_result","tool_use_id":"t0a","content":"AAAA"},
                {"type":"tool_result","tool_use_id":"t0b","content":"BBBBBB"},
                {"type":"tool_result","tool_use_id":"t1","content":""},
                {"type":"tool_result","tool_use_id":"t2","content":""}]}}),
        ];
        std::fs::write(
            dir.join("sess.jsonl"),
            parent
                .iter()
                .map(|v| line(v.clone()))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        // Sub 1 (t1): re-reads a.rs (4 B, parent overlap), reads c.rs (6 B), one 5 B Bash.
        let one: Vec<Value> = vec![
            json!({"type":"user","message":{"role":"user","content":"one"}}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"tool_use","id":"u1","name":"Read","input":{"file_path":"a.rs"}},
                {"type":"tool_use","id":"u2","name":"Read","input":{"file_path":"c.rs"}},
                {"type":"tool_use","id":"u3","name":"Bash","input":{"command":"ls"}}]}}),
            json!({"type":"user","message":{"role":"user","content":[
                {"type":"tool_result","tool_use_id":"u1","content":"AAAA"},
                {"type":"tool_result","tool_use_id":"u2","content":"CCCCCC"},
                {"type":"tool_result","tool_use_id":"u3","content":"XXXXX"}]}}),
        ];
        // Sub 2 (t2): re-reads c.rs (2 B, sibling only), reads d.rs (3 B).
        let two: Vec<Value> = vec![
            json!({"type":"user","message":{"role":"user","content":"two"}}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"tool_use","id":"u1","name":"Read","input":{"file_path":"c.rs"}},
                {"type":"tool_use","id":"u2","name":"Read","input":{"file_path":"d.rs"}}],
                "usage":{"input_tokens":2,"cache_read_input_tokens":7,"cache_creation_input_tokens":3,"output_tokens":4}}}),
            json!({"type":"user","message":{"role":"user","content":[
                {"type":"tool_result","tool_use_id":"u1","content":"CC"},
                {"type":"tool_result","tool_use_id":"u2","content":"DDD"}]}}),
        ];
        for (name, lines) in [("agent-1", &one), ("agent-2", &two)] {
            std::fs::write(
                subs.join(format!("{name}.jsonl")),
                lines
                    .iter()
                    .map(|v| line(v.clone()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
        }
        std::fs::write(
            subs.join("agent-1.meta.json"),
            r#"{"agentType":"explore","model":"haiku","toolUseId":"t1"}"#,
        )
        .unwrap();
        std::fs::write(
            subs.join("agent-2.meta.json"),
            r#"{"agentType":"general-purpose","toolUseId":"t2"}"#,
        )
        .unwrap();

        let parsed = jsonl::parse_path(&dir.join("sess.jsonl")).unwrap();
        let parents = [Parent::new(&dir.join("sess.jsonl"), &parsed)];
        let out = collect(&parents, SystemTime::UNIX_EPOCH).unwrap();

        assert_eq!((out.sessions, out.count), (1, 2));
        assert_eq!(out.result_bytes, 20, "4+6+5 (sub 1) + 2+3 (sub 2)");
        assert_eq!(out.parent_result_bytes, 10);
        assert_eq!(out.read_bytes, 15, "4+6+2+3");
        assert_eq!(out.read_parent, 4, "a.rs, sub 1 — parent read it too");
        assert_eq!(
            out.read_sibling, 2,
            "c.rs, sub 2 — an earlier sibling read it"
        );
        assert_eq!(out.reread_bytes, 6);
        // The three shares of §17.1: of sub-agent read bytes, sub-agent result bytes,
        // and the tree's result bytes.
        assert_eq!(out.share_read, 100.0 * 6.0 / 15.0);
        assert_eq!(out.share_results, 100.0 * 6.0 / 20.0);
        assert_eq!(out.share_tree, 100.0 * 6.0 / 30.0);
        assert_eq!(
            (
                out.usage_input,
                out.usage_cache_read,
                out.usage_cache_write,
                out.usage_output
            ),
            (2, 7, 3, 4)
        );
        assert_eq!(out.by_type.len(), 2);
        let explore = &out.by_type[0];
        assert_eq!(
            (
                explore.agent_type.as_str(),
                explore.model.as_str(),
                explore.count
            ),
            ("explore", "haiku", 1)
        );
        assert_eq!((explore.read_bytes, explore.reread_bytes), (10, 4));
        let general = &out.by_type[1];
        assert_eq!(
            (
                general.agent_type.as_str(),
                general.model.as_str(),
                general.count
            ),
            ("general-purpose", "-", 1),
            "a meta without `model` reads as `-`"
        );
        assert_eq!((general.read_bytes, general.reread_bytes), (5, 2));

        assert!(is_subagent(&subs.join("agent-1.jsonl")));
        assert!(!is_subagent(&dir.join("sess.jsonl")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T131: one sub-agent spawned with a `SubagentStart` brief (its first user message
    /// carries `memory::handoff`'s fixed instructions), one spawned plain — the split must
    /// keep each agent's read/re-read bytes, share, and input tokens in its own bucket.
    #[test]
    fn briefed_and_plain_subagents_split_the_reread_share() {
        let dir = crate::testutil::tmp_dir("t131-brief-split");
        let subs = dir.join("sess").join("subagents");
        std::fs::create_dir_all(&subs).unwrap();
        // Parent reads p.rs, then spawns t1 (briefed) and t2 (plain).
        let parent: Vec<Value> = vec![
            json!({"type":"user","message":{"role":"user","content":"go"}}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"tool_use","id":"t0","name":"Read","input":{"file_path":"p.rs"}},
                {"type":"tool_use","id":"t1","name":"Agent","input":{"prompt":"one"}},
                {"type":"tool_use","id":"t2","name":"Agent","input":{"prompt":"two"}}]}}),
            json!({"type":"user","message":{"role":"user","content":[
                {"type":"tool_result","tool_use_id":"t0","content":"AAAA"},
                {"type":"tool_result","tool_use_id":"t1","content":""},
                {"type":"tool_result","tool_use_id":"t2","content":""}]}}),
        ];
        std::fs::write(
            dir.join("sess.jsonl"),
            parent
                .iter()
                .map(|v| line(v.clone()))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        // Briefed (t1): first user message carries the spawn brief's fixed instructions,
        // re-reads p.rs (4 B, parent overlap) and reads a fresh q.rs (6 B).
        let briefed: Vec<Value> = vec![
            json!({"type":"user","message":{"role":"user","content":
                format!("p.rs — rtok expand abc123\n{SPAWN_BRIEF_MARKER} <id>; answer with path:line citations.")}}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"tool_use","id":"u1","name":"Read","input":{"file_path":"p.rs"}},
                {"type":"tool_use","id":"u2","name":"Read","input":{"file_path":"q.rs"}}],
                "usage":{"input_tokens":5}}}),
            json!({"type":"user","message":{"role":"user","content":[
                {"type":"tool_result","tool_use_id":"u1","content":"AAAA"},
                {"type":"tool_result","tool_use_id":"u2","content":"QQQQQQ"}]}}),
        ];
        // Plain (t2): an ordinary first user message, reads a fresh r.rs (3 B) — no overlap.
        let plain: Vec<Value> = vec![
            json!({"type":"user","message":{"role":"user","content":"go"}}),
            json!({"type":"assistant","message":{"role":"assistant","content":[
                {"type":"tool_use","id":"u1","name":"Read","input":{"file_path":"r.rs"}}],
                "usage":{"input_tokens":2}}}),
            json!({"type":"user","message":{"role":"user","content":[
                {"type":"tool_result","tool_use_id":"u1","content":"RRR"}]}}),
        ];
        for (name, lines) in [("agent-1", &briefed), ("agent-2", &plain)] {
            std::fs::write(
                subs.join(format!("{name}.jsonl")),
                lines
                    .iter()
                    .map(|v| line(v.clone()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
        }
        std::fs::write(
            subs.join("agent-1.meta.json"),
            r#"{"agentType":"general-purpose","toolUseId":"t1"}"#,
        )
        .unwrap();
        std::fs::write(
            subs.join("agent-2.meta.json"),
            r#"{"agentType":"general-purpose","toolUseId":"t2"}"#,
        )
        .unwrap();

        let parsed = jsonl::parse_path(&dir.join("sess.jsonl")).unwrap();
        let parents = [Parent::new(&dir.join("sess.jsonl"), &parsed)];
        let out = collect(&parents, SystemTime::UNIX_EPOCH).unwrap();

        assert_eq!(
            (
                out.with_brief.count,
                out.with_brief.read_bytes,
                out.with_brief.reread_bytes
            ),
            (1, 10, 4),
            "{:?}",
            out.with_brief
        );
        assert_eq!(out.with_brief.share_read, 100.0 * 4.0 / 10.0);
        assert_eq!(out.with_brief.usage_input, 5);

        assert_eq!(
            (
                out.without_brief.count,
                out.without_brief.read_bytes,
                out.without_brief.reread_bytes
            ),
            (1, 3, 0),
            "{:?}",
            out.without_brief
        );
        assert_eq!(out.without_brief.share_read, 0.0);
        assert_eq!(out.without_brief.usage_input, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
