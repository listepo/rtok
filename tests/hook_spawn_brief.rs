//! T130: `SubagentStart` spawn brief. `research.md` §17.3(2) picked `SubagentStart`'s
//! `additionalContext` over `PreToolUse`'s `updatedInput`, which
//! <https://code.claude.com/docs/en/hooks> (checked 2026-09-22) documents as ignored by the
//! `Agent`/`Task` tools — see `plan.md` T130.1.

use assert_cmd::Command as AssertCmd;
use serde_json::{Value, json};
use std::path::PathBuf;

struct Home(PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(name: &str) -> Home {
    let t = std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos();
    let d = std::env::temp_dir().join(format!("rtok-t130-{name}-{}-{t}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Home(d)
}

fn hook(home: &Home, event: &str, body: &Value) -> Value {
    hook_as(home, &[], event, body)
}

/// `rtok hook <event> [extra…]`, e.g. `--host copilot`.
fn hook_as(home: &Home, extra: &[&str], event: &str, body: &Value) -> Value {
    let out = AssertCmd::cargo_bin("rtok")
        .unwrap()
        .args(["hook", event])
        .args(extra)
        .env("RTOK_HOME", &home.0)
        .env("HOME", &home.0)
        .write_stdin(serde_json::to_vec(body).unwrap())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out)))
}

fn enable_spawn_brief(home: &Home) {
    std::fs::write(
        home.0.join("config.toml"),
        "[plugins.memory]\nspawn_brief = true\nspawn_brief_tokens = 60\n",
    )
    .unwrap();
}

fn read_tool(session: &str, path: &str) -> Value {
    json!({
        "session_id": session,
        "cwd": "/tmp",
        "hook_event_name": "PreToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": path},
    })
}

fn subagent_start(session: &str) -> Value {
    json!({
        "session_id": session,
        "cwd": "/tmp",
        "hook_event_name": "SubagentStart",
        "agent_id": "agent-1",
        "agent_type": "general-purpose",
        "task_description": "look at /repo/a.rs",
    })
}

fn brief_of(v: &Value) -> Option<&str> {
    v["hookSpecificOutput"]["additionalContext"].as_str()
}

#[test]
fn flag_off_is_a_passthrough() {
    let home = tmp("off");
    let session = "s-off";
    let _ = hook(&home, "PreToolUse", &read_tool(session, "/repo/a.rs"));
    let out = hook(&home, "SubagentStart", &subagent_start(session));
    assert_eq!(out, json!({}), "spawn_brief defaults to off");
}

#[test]
fn empty_ledger_is_a_passthrough() {
    let home = tmp("empty");
    enable_spawn_brief(&home);
    let out = hook(&home, "SubagentStart", &subagent_start("s-empty"));
    assert_eq!(out, json!({}), "nothing read or edited yet");
}

#[test]
fn brief_carries_pointers_an_expand_id_and_stays_under_budget() {
    let home = tmp("brief");
    enable_spawn_brief(&home);
    let session = "s-brief";
    let _ = hook(&home, "PreToolUse", &read_tool(session, "/repo/a.rs"));
    let _ = hook(&home, "PreToolUse", &read_tool(session, "/repo/b.rs"));
    let first = hook(&home, "SubagentStart", &subagent_start(session));
    let text = brief_of(&first).expect("a non-empty ledger must offer a brief");
    assert!(text.contains("/repo/a.rs"), "{text}");
    assert!(text.contains("rtok expand"), "{text}");
    assert!(text.contains("rtok read(mode=lines"), "{text}");

    let second = hook(&home, "SubagentStart", &subagent_start(session));
    assert_eq!(first, second, "an unchanged ledger must be byte-stable");
}

#[test]
fn non_subagent_events_are_untouched() {
    let home = tmp("other");
    enable_spawn_brief(&home);
    let session = "s-other";
    let _ = hook(&home, "PreToolUse", &read_tool(session, "/repo/a.rs"));
    let out = hook(
        &home,
        "PreToolUse",
        &json!({
            "session_id": session,
            "cwd": "/tmp",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "echo hi"},
        }),
    );
    assert!(
        out.get("hookSpecificOutput").is_none()
            || out["hookSpecificOutput"]["additionalContext"].is_null(),
        "{out}"
    );
}

/// T262.5: a host adapter's input reaches the ledger. Copilot sends camelCase (`toolName: view`,
/// `toolArgs.path`); the call row used to keep that raw body, which the ledger's `tool_name`
/// scan never matched, so a Copilot session's brief was always empty.
#[test]
fn copilot_reads_reach_the_brief() {
    let home = tmp("copilot");
    enable_spawn_brief(&home);
    let session = "s-copilot";
    let read = json!({"sessionId": session, "cwd": "/tmp", "toolName": "view", "toolArgs": {"path": "/repo/c.rs"}});
    let _ = hook_as(&home, &["--host", "copilot"], "PreToolUse", &read);
    let out = hook(&home, "SubagentStart", &subagent_start(session));
    let text = brief_of(&out).expect("the Copilot read must reach the ledger");
    assert!(text.contains("/repo/c.rs"), "{text}");

    // T262.4: Copilot's own `subagentStart` payload gets the brief as flat `additionalContext`.
    let spawn =
        json!({"sessionId": session, "timestamp": 1, "cwd": "/tmp", "agentName": "explore"});
    let out = hook_as(&home, &["--host", "copilot"], "SubagentStart", &spawn);
    let text = out["additionalContext"]
        .as_str()
        .unwrap_or_else(|| panic!("{out}"));
    assert!(text.contains("/repo/c.rs"), "{text}");
}
