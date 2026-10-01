//! Hook stdin/stdout shaping, before any plugin runs: the `HookInput` JSON every host sends,
//! each host adapter (`adapt_cursor`, `adapt_grok`, …), the typed event views, host tool-name
//! mapping plus the guard's duplicate key, and the per-host reply encoders. In-process only:
//! `hooks::dispatch` itself opens the store and plugins read files, so it stays out.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rtok::hooks::types::{HookInput, HookOutput};
use rtok::hooks::{cline_output, codewhale_output, copilot_output, cursor_output, gemini_output};

const EVENTS: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "SessionStart",
    "UserPromptSubmit",
    "SubagentStart",
    "PreCompact",
    "Stop",
    "beforeShellExecution",
    "afterShellExecution",
    "postToolUse",
    "afterMCPExecution",
    "BeforeTool",
    "AfterTool",
    "PreCompress",
    "preToolUse",
];

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    stdin: &'a [u8],
    reply: &'a [u8],
    event: u8,
    free_event: Option<&'a str>,
    project_dir: Option<String>,
    prompt: Option<&'a str>,
}

fuzz_target!(|i: Input<'_>| {
    let event = i
        .free_event
        .unwrap_or(EVENTS[usize::from(i.event) % EVENTS.len()]);
    if let Ok(input) = serde_json::from_slice::<HookInput>(i.stdin) {
        let adapters: [&dyn Fn(&mut HookInput); 9] = [
            &|h| h.adapt_cursor(event),
            &|h| h.adapt_copilot(event),
            &|h| h.adapt_devin(event, i.project_dir.clone()),
            &|h| h.adapt_gemini(event),
            &|h| h.adapt_codewhale(event),
            &|h| h.adapt_cline(event),
            &|h| h.adapt_commandcode(event, i.project_dir.clone()),
            &|h| h.adapt_grok(event),
            &|h| h.take_transcript_path_alias(),
        ];
        for adapt in adapters {
            let mut h = input.clone();
            adapt(&mut h);
            let _ = (
                h.pre_tool(),
                h.post_tool(),
                h.session_start(),
                h.prompt_submit(),
            );
            let _ = (h.subagent_start(), h.pre_compact(), h.mcp_server_name());
            if let (Some(tool), Some(args)) = (&h.tool_name, &h.tool_input) {
                let _ = rtok::fuzzing::tool_names(tool, args);
            }
            let _ = serde_json::to_vec(&h);
        }
    }
    if let Ok(out) = serde_json::from_slice::<HookOutput>(i.reply) {
        let _ = gemini_output(&out, event);
        let _ = codewhale_output(&out, i.prompt);
        let _ = copilot_output(&out);
        let _ = cline_output(&out);
        let _ = cursor_output(&out);
    }
});
