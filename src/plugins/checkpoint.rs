//! PreCompact checkpoint + compact restore (plan T2.5).

use rtok_plugin_sdk::{Class, Ctx, Injection, Measurement};
use serde_json::Value;
use std::collections::{BTreeSet, VecDeque};
use std::path::Path;

/// Parsed compact snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkpoint {
    pub prompts: Vec<String>,
    pub paths: Vec<String>,
    pub errors: Vec<String>,
    /// Skills the host injected before compaction, `(name, body bytes)` per invocation
    /// (plan T62.2): the body is gone after compaction and the model must know what it
    /// had, not re-invoke everything.
    pub skills: Vec<(String, u64)>,
    /// Archive ids of this session's live-window tool results, newest first (T58.2).
    pub ids: Vec<String>,
    /// `(tool, bytes)` aligned with [`Self::ids`].
    id_meta: Vec<(String, u64)>,
}

impl Checkpoint {
    pub fn render(&self) -> String {
        let mut s = String::from("checkpoint\n");
        for p in &self.prompts {
            s.push_str("- ");
            s.push_str(p);
            s.push('\n');
        }
        if !self.skills.is_empty() {
            let list: Vec<String> = self
                .skills
                .iter()
                .map(|(n, b)| format!("{n} ({:.1} KB)", *b as f64 / 1024.0))
                .collect();
            s.push_str("skills loaded before compaction: ");
            s.push_str(&list.join(", "));
            s.push_str(" — re-invoke only what the next step needs\n");
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
        for (id, (tool, bytes)) in self.ids.iter().zip(&self.id_meta) {
            s.push_str("id ");
            s.push_str(id);
            s.push(' ');
            s.push_str(tool);
            s.push(' ');
            s.push_str(&bytes.to_string());
            s.push('\n');
        }
        s
    }
}

/// Last 20 user prompts (≤ 300 chars), file paths, and the last 8 error lines from a JSONL
/// transcript.
pub fn extract(jsonl: &str) -> Checkpoint {
    let mut prompts = Vec::new();
    let mut paths = BTreeSet::new();
    let mut errors = VecDeque::new();
    let mut skills = Vec::new();
    for line in jsonl.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        walk(&v, &mut paths, &mut |s| {
            // The three spellings that occur in compiler, test and runtime output; no
            // lowercased copy of every transcript string.
            if ["error", "Error", "ERROR"].iter().any(|n| s.contains(n)) {
                if errors.len() == 8 {
                    errors.pop_front();
                }
                errors.push_back(s.chars().take(200).collect());
            }
        });
        if let Some(s) = skill_body(&v) {
            skills.push(s);
        } else if let Some(p) = user_prompt(&v) {
            prompts.push(p);
        }
    }
    if prompts.len() > 20 {
        prompts = prompts.split_off(prompts.len() - 20);
    }
    Checkpoint {
        prompts,
        paths: paths.into_iter().collect(),
        errors: errors.into(),
        skills,
        ..Default::default()
    }
}

/// A skill body the host injected: a `user` record flagged `isMeta` with a
/// `sourceToolUseID` whose text opens with `Base directory for this skill: <dir>`
/// (Claude Code, checked 2026-09-17). The name is the directory's last component so a
/// plugin skill and a user skill resolve the same way; the size is the injected text.
fn skill_body(v: &Value) -> Option<(String, u64)> {
    if v.get("type").and_then(Value::as_str) != Some("user")
        || v.get("isMeta").and_then(Value::as_bool) != Some(true)
        || v.get("sourceToolUseID").is_none()
    {
        return None;
    }
    let text = user_text(v)?;
    let dir = text
        .lines()
        .next()?
        .strip_prefix("Base directory for this skill: ")?
        .trim_end_matches(['/', '\\']);
    let name = dir.rsplit(['/', '\\']).next().filter(|n| !n.is_empty())?;
    Some((name.to_string(), text.len() as u64))
}

fn user_prompt(v: &Value) -> Option<String> {
    if v.get("type").and_then(Value::as_str) != Some("user") {
        return None;
    }
    let raw = user_text(v)?;
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.chars().take(300).collect())
    }
}

/// Text blocks of a user record joined by newlines; `None` when there are none.
fn user_text(v: &Value) -> Option<String> {
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
    Some(raw)
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

/// Note kind per session: compaction keeps the session id, and two hosts compacting at
/// once must not restore each other's checkpoint.
fn kind(cx: &Ctx) -> String {
    format!("checkpoint:{}", cx.session())
}

/// Read `transcript_path`, store a `notes` row `kind=checkpoint:<session>`.
pub fn save(transcript_path: &str, cx: &Ctx) -> anyhow::Result<Checkpoint> {
    write(transcript_path, cx, &kind(cx), Some("rtok"))
}

/// SessionEnd: same extractor and render as [`save`], kind `session:<session>`.
pub fn save_session(transcript_path: &str, cx: &Ctx) -> anyhow::Result<Checkpoint> {
    let project = project_of_cx(cx);
    write(
        transcript_path,
        cx,
        &format!("session:{}", cx.session()),
        project.as_deref(),
    )
}

fn write(
    transcript_path: &str,
    cx: &Ctx,
    kind: &str,
    project: Option<&str>,
) -> anyhow::Result<Checkpoint> {
    let mut cp = extract(&std::fs::read_to_string(Path::new(transcript_path)).unwrap_or_default());
    attach_ids(&mut cp, cx);
    cx.insert_note(project, kind, "compact", &cp.render())?;
    Ok(cp)
}

fn project_of_cx(cx: &Ctx) -> Option<String> {
    crate::config::layers::git_root(Path::new(cx.cwd()?))?
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
}

fn checkpoint_cap(cx: &Ctx) -> u32 {
    cx.plugin_config::<crate::config::Memory>("memory")
        .checkpoint_tokens
        .max(1)
}

/// Newest live archive ids that still fit `plugins.memory.checkpoint_tokens`.
fn attach_ids(cp: &mut Checkpoint, cx: &Ctx) {
    let db: std::path::PathBuf = cx.config("core.db_path");
    if db.as_os_str().is_empty() {
        return;
    }
    let Ok(store) = crate::store::Store::open(&db) else {
        return;
    };
    let Ok(rows) = store.session_live_archives(cx.session()) else {
        return;
    };
    let cap = checkpoint_cap(cx);
    for (id, tool, bytes) in rows {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        cp.ids.push(id);
        cp.id_meta.push((tool, bytes));
        if cx.estimate(&cp.render(), Class::Prose) > cap {
            cp.ids.pop();
            cp.id_meta.pop();
            break;
        }
    }
}

/// Latest checkpoint of this session as an injection, capped at
/// `plugins.memory.checkpoint_tokens`.
pub fn offer(cx: &Ctx) -> Option<Injection> {
    render_offer(cx, cx.latest_note(&kind(cx)).ok().flatten()?)
}

/// Newest `session:*` note of the hook cwd's project, same render and budget as [`offer`].
pub fn offer_session(cx: &Ctx) -> Option<Injection> {
    let db: std::path::PathBuf = cx.config("core.db_path");
    if db.as_os_str().is_empty() {
        return None;
    }
    let store = crate::store::Store::open(&db).ok()?;
    let raw = store
        .latest_session_note(project_of_cx(cx).as_deref())
        .ok()
        .flatten()?;
    let inj = render_offer(cx, raw.clone())?;
    let _ = cx.record(&Measurement {
        plugin: "memory",
        kind: "handoff",
        before_bytes: raw.len() as u64,
        after_bytes: inj.text.len() as u64,
        est_before: cx.estimate(&raw, Class::Prose),
        est_after: cx.estimate(&inj.text, Class::Prose),
        ref_id: None,
        call_id: None,
    });
    Some(inj)
}

fn render_offer(cx: &Ctx, text: String) -> Option<Injection> {
    let cap = checkpoint_cap(cx);
    let text = crate::plugin::fit_budget(cx, &text, Class::Prose, cap);
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
            .latest_note("checkpoint:t25")
            .unwrap()
            .expect("note");
        let restore = |session: &str, out: &mut Vec<u8>| {
            let start = serde_json::json!({"hook_event_name":"SessionStart","session_id":session,"source":"compact"});
            out.clear();
            crate::hooks::run(
                "SessionStart",
                start.to_string().as_bytes(),
                &mut *out,
                &cfg,
            );
            serde_json::from_slice::<serde_json::Value>(out).unwrap()["hookSpecificOutput"]
                ["additionalContext"]
                .as_str()
                .unwrap_or("")
                .to_string()
        };
        let text = restore("t25", &mut out);
        let cx = crate::plugin::Runtime::in_memory("budget").unwrap();
        for p in ["src/a.rs", "src/b.rs", "src/c.rs"] {
            assert!(body.contains(p) && text.contains(p), "{body}\n{text}");
        }
        assert!(cx.estimate(&text, Class::Prose) <= cx.config.plugins.memory.checkpoint_tokens);
        // Another session compacting against the same store gets nothing of t25's.
        let other = restore("t25-other", &mut out);
        assert!(!other.contains("src/a.rs"), "{other}");
    }

    /// The injected body is a skill, never a prompt: it lands on the skills line with
    /// its size, and the prompt list keeps only what the human typed.
    #[test]
    fn injected_skill_bodies_are_listed_not_quoted() {
        let body = "Base directory for this skill: /home/u/.claude/skills/slint\n\n# Slint\n"
            .to_string()
            + &"x".repeat(5000);
        let lines = [
            r#"{"type":"user","message":{"content":"make it blue"}}"#.to_string(),
            serde_json::json!({"type":"user","isMeta":true,"sourceToolUseID":"toolu_1","message":{"content":[{"type":"text","text":body}]}}).to_string(),
            serde_json::json!({"type":"user","isMeta":true,"sourceToolUseID":"toolu_2","message":{"content":[{"type":"text","text":"Base directory for this skill: C:\\u\\.claude\\plugins\\cache\\p\\1.0\\skills\\ponytail\n\n# P"}]}}).to_string(),
            // `isMeta` without a source tool is a plain meta prompt, not a skill.
            r#"{"type":"user","isMeta":true,"message":{"content":"Base directory for this skill: /x/y"}}"#.to_string(),
        ]
        .join("\n");
        let cp = extract(&lines);
        assert_eq!(cp.skills.len(), 2, "{:?}", cp.skills);
        assert_eq!(cp.skills[0].0, "slint");
        assert_eq!(cp.skills[0].1, body.len() as u64);
        assert_eq!(cp.skills[1].0, "ponytail");
        assert_eq!(
            cp.prompts,
            ["make it blue", "Base directory for this skill: /x/y"]
        );
        let text = cp.render();
        assert!(text.contains("skills loaded before compaction: slint (5.0 KB), ponytail (0.1 KB) — re-invoke only what the next step needs\n"), "{text}");
        assert!(!text.contains("xxxx"), "{text}");
    }

    #[test]
    fn errors_keep_the_last_eight() {
        let lines = (0..12)
            .map(|i| format!(r#"{{"type":"assistant","message":{{"content":"Error {i}"}}}}"#))
            .collect::<Vec<_>>()
            .join("\n");
        let cp = extract(&lines);
        assert_eq!(cp.errors.len(), 8);
        assert_eq!(cp.errors[0], "Error 4");
        assert_eq!(cp.errors[7], "Error 11");
    }

    #[test]
    fn three_archived_results_yield_id_lines_on_restore() {
        let dir = std::env::temp_dir().join("rtok-t582-three-ids");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.jsonl"), FIXTURE).unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.core.archive_dir = dir.join("archive");
        let store = crate::store::Store::open(&cfg.core.db_path).unwrap();
        let mut ids = Vec::new();
        for (i, body) in [b"one-result".as_slice(), b"two-result", b"three-result"]
            .into_iter()
            .enumerate()
        {
            let id = store
                .put_archive("t582", body, &cfg.core.archive_dir)
                .unwrap();
            store
                .put_archive_decision(&format!("tu{i}"), &id, "t582", "ptr")
                .unwrap();
            ids.push(id);
        }
        ids.reverse(); // newest first
        let pre = serde_json::json!({
            "hook_event_name":"PreCompact",
            "session_id":"t582",
            "transcript_path":dir.join("t.jsonl").to_str().unwrap(),
            "trigger":"auto"
        });
        let mut out = Vec::new();
        crate::hooks::run("PreCompact", pre.to_string().as_bytes(), &mut out, &cfg);
        assert_eq!(out, b"{}");
        let body = crate::store::Store::open(&cfg.core.db_path)
            .unwrap()
            .latest_note("checkpoint:t582")
            .unwrap()
            .expect("note");
        for id in &ids {
            let line = format!("id {id} - {}", /* bytes filled below */ 0);
            let _ = line;
            assert!(
                body.lines().any(|l| l.starts_with(&format!("id {id} - "))),
                "missing {id} in {body}"
            );
        }
        assert_eq!(
            body.lines().filter(|l| l.starts_with("id ")).count(),
            3,
            "{body}"
        );
        let start = serde_json::json!({
            "hook_event_name":"SessionStart","session_id":"t582","source":"compact"
        });
        out.clear();
        crate::hooks::run("SessionStart", start.to_string().as_bytes(), &mut out, &cfg);
        let text = serde_json::from_slice::<serde_json::Value>(&out).unwrap()["hookSpecificOutput"]
            ["additionalContext"]
            .as_str()
            .unwrap_or("")
            .to_string();
        for id in &ids {
            assert!(text.contains(&format!("id {id} - ")), "{text}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T71.2 e2e: `SessionEnd` saves the transcript under `session:<id>` with the project
    /// from the hook cwd; `startup_recall` off injects nothing at startup, on restores the
    /// newest project note byte-stably within `checkpoint_tokens`, measured as `handoff`.
    #[test]
    fn session_end_note_and_startup_recall() {
        let dir = std::env::temp_dir().join("rtok-t712-session-end");
        let _ = std::fs::remove_dir_all(&dir);
        let repo = dir.join("myproj");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join("t.jsonl"), FIXTURE).unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.plugins.inject.modes.clear();
        let end = serde_json::json!({
            "hook_event_name":"SessionEnd",
            "session_id":"t712",
            "transcript_path":repo.join("t.jsonl").to_str().unwrap(),
            "cwd":repo.display().to_string(),
            "reason":"clear"
        });
        let mut out = Vec::new();
        crate::hooks::run("SessionEnd", end.to_string().as_bytes(), &mut out, &cfg);
        assert_eq!(out, b"{}");
        let body = crate::store::Store::open(&cfg.core.db_path)
            .unwrap()
            .latest_session_note(Some("myproj"))
            .unwrap()
            .expect("session note");
        assert!(body.starts_with("checkpoint\n"), "{body}");
        for p in ["src/a.rs", "src/b.rs", "src/c.rs"] {
            assert!(body.contains(p), "{body}");
        }

        let start = serde_json::json!({
            "hook_event_name":"SessionStart",
            "session_id":"t712-next",
            "source":"startup",
            "cwd":repo.display().to_string()
        });
        let mut off = Vec::new();
        crate::hooks::run("SessionStart", start.to_string().as_bytes(), &mut off, &cfg);
        assert_eq!(off, b"{}", "startup_recall off injects nothing");

        cfg.plugins.memory.startup_recall = true;
        let mut on1 = Vec::new();
        crate::hooks::run("SessionStart", start.to_string().as_bytes(), &mut on1, &cfg);
        let mut on2 = Vec::new();
        crate::hooks::run("SessionStart", start.to_string().as_bytes(), &mut on2, &cfg);
        assert_eq!(on1, on2, "byte-stable for an unchanged store");
        let text = serde_json::from_slice::<serde_json::Value>(&on1).unwrap()["hookSpecificOutput"]
            ["additionalContext"]
            .as_str()
            .unwrap_or("")
            .to_string();
        for p in ["src/a.rs", "src/b.rs", "src/c.rs"] {
            assert!(text.contains(p), "{text}");
        }
        let est = crate::plugin::Runtime::in_memory("t712-budget").unwrap();
        assert!(
            est.estimate(&text, Class::Prose) <= est.config.plugins.memory.checkpoint_tokens,
            "{text}"
        );
        let handoffs = crate::store::Store::open(&cfg.core.db_path)
            .unwrap()
            .list_measurements("memory")
            .unwrap()
            .into_iter()
            .filter(|r| r.kind == "handoff")
            .count();
        assert_eq!(handoffs, 2, "one measurement per startup restore");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The cap is a hard ceiling whatever the text size, and the cut is not one pop per char.
    #[test]
    fn offer_fits_checkpoint_tokens() {
        let cx = crate::plugin::Runtime::in_memory("cap").unwrap();
        let ctx = Ctx::new(&cx);
        let cap = cx.config.plugins.memory.checkpoint_tokens.max(1);
        let big = "checkpoint\n".repeat(4000);
        ctx.insert_note(Some("rtok"), &kind(&ctx), "compact", &big)
            .unwrap();
        let inj = offer(&ctx).expect("injection");
        assert!(cx.estimate(&inj.text, Class::Prose) <= cap);
        assert!(inj.text.starts_with("checkpoint\n"), "{}", inj.text);

        let mut many = Checkpoint {
            prompts: vec!["x".repeat(80)],
            ..Default::default()
        };
        for i in 0..200 {
            many.ids.push(format!("id{i:04}"));
            many.id_meta.push(("Read".into(), 9999));
            if cx.estimate(&many.render(), Class::Prose) > cap {
                many.ids.pop();
                many.id_meta.pop();
                break;
            }
        }
        assert!(!many.ids.is_empty());
        assert!(cx.estimate(&many.render(), Class::Prose) <= cap);
        ctx.insert_note(Some("rtok"), &kind(&ctx), "compact", &many.render())
            .unwrap();
        let inj = offer(&ctx).expect("capped ids");
        assert!(cx.estimate(&inj.text, Class::Prose) <= cap);
    }

    /// T70.6: pi/OpenCode plugins call the same PreCompact/SessionStart path as Claude,
    /// so restore bytes match for the same store. T58.2 covers hook hosts only.
    #[test]
    fn pi_and_opencode_compact_restore_bytes_match_claude() {
        let dir = std::env::temp_dir().join("rtok-t706-host-bytes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.jsonl"), FIXTURE).unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.core.db_path = dir.join("rtok.db");
        cfg.plugins.inject.modes.clear();
        let pre = serde_json::json!({
            "hook_event_name":"PreCompact",
            "session_id":"t706",
            "transcript_path":dir.join("t.jsonl").to_str().unwrap(),
            "trigger":"auto"
        });
        let mut out = Vec::new();
        crate::hooks::run("PreCompact", pre.to_string().as_bytes(), &mut out, &cfg);
        let mut restore = |host: &str| {
            cfg.hook.host = host.into();
            let start = serde_json::json!({
                "hook_event_name":"SessionStart",
                "session_id":"t706",
                "source":"compact"
            });
            let mut buf = Vec::new();
            crate::hooks::run("SessionStart", start.to_string().as_bytes(), &mut buf, &cfg);
            serde_json::from_slice::<serde_json::Value>(&buf).unwrap()["hookSpecificOutput"]
                ["additionalContext"]
                .as_str()
                .unwrap_or("")
                .to_string()
        };
        let claude = restore("claude");
        let pi = restore("pi");
        let opencode = restore("opencode");
        assert!(!claude.is_empty(), "{claude}");
        assert_eq!(claude, pi);
        assert_eq!(claude, opencode);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
