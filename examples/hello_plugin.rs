//! The smallest complete plugin, end to end. Run: `just example`.
//!
//! `Hello` denies destructive `rm -rf` commands, injects one line at session start,
//! and records a `Measurement` — because a saving that is not a row does not exist.
//! Everything a real plugin does is one of these three moves.

use anyhow::Result;
use rtok::hooks::types::{HookInput, HookOutput, HookSpecificOutput};
use rtok::plugin::{
    Ctx, Injection, Manifest, Measurement, Plugin, PreToolDecision, PreToolUse, SessionStart,
    Surface,
};
use rtok::tokens::Class;

struct Hello;

impl Plugin for Hello {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "hello",
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        let cmd = ev.tool_input.get("command")?.as_str()?;
        if ev.tool_name == "Bash" && cmd.contains("rm -rf") {
            // What the model would have seen had the command run (illustrative).
            let before = cx.estimate("removed 12 files\n", Class::Code);
            cx.record(&Measurement {
                plugin: "hello",
                kind: "deny",
                before_bytes: 17,
                after_bytes: 0,
                est_before: before,
                est_after: 0,
                ref_id: None,
                call_id: None,
            })
            .ok()?;
            return Some(PreToolDecision::Deny {
                reason: "hello: rm -rf is blocked in this example".into(),
            });
        }
        None
    }

    fn session_start(&self, ev: &SessionStart, _cx: &Ctx) -> Option<Injection> {
        Some(Injection {
            plugin: "hello",
            text: format!("hello plugin active (source={})", ev.source),
            priority: 1,
        })
    }
}

/// What the dispatcher (plan T2.1) will do for one event: run the plugin, shape the output.
fn run(plugin: &dyn Plugin, cx: &Ctx, input: &HookInput) -> HookOutput {
    let mut out = HookOutput::default();
    if let Some(ev) = input.pre_tool() {
        if let Some(PreToolDecision::Deny { reason }) = plugin.pre_tool(&ev, cx) {
            out.hook_specific_output = Some(HookSpecificOutput {
                hook_event_name: "PreToolUse".into(),
                permission_decision: Some("deny".into()),
                permission_decision_reason: Some(reason),
                ..Default::default()
            });
        }
    } else if let Some(ev) = input.session_start()
        && let Some(inj) = plugin.session_start(&ev, cx)
    {
        out.hook_specific_output = Some(HookSpecificOutput {
            hook_event_name: "SessionStart".into(),
            additional_context: Some(inj.text),
            ..Default::default()
        });
    }
    out
}

fn main() -> Result<()> {
    let cx = Ctx::in_memory("example-session")?;
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hooks");

    let mut bash: HookInput = serde_json::from_str(&std::fs::read_to_string(format!(
        "{fixtures}/pre_tool_bash.json"
    ))?)?;
    println!(
        "git status      → {}",
        serde_json::to_string(&run(&Hello, &cx, &bash))?
    );

    bash.tool_input = Some(serde_json::json!({ "command": "rm -rf build" }));
    println!(
        "rm -rf build    → {}",
        serde_json::to_string(&run(&Hello, &cx, &bash))?
    );

    let start: HookInput = serde_json::from_str(&std::fs::read_to_string(format!(
        "{fixtures}/session_start.json"
    ))?)?;
    println!(
        "SessionStart    → {}",
        serde_json::to_string(&run(&Hello, &cx, &start))?
    );

    let rows: i64 = cx.store.measurement_count("hello")?;
    println!("measurement rows: {rows}");
    assert_eq!(rows, 1, "the deny must leave exactly one measurement");
    Ok(())
}
