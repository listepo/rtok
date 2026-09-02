//! PreToolUse(Bash) rewrite to `rtok run --` (plan T3.4).

use crate::plugin::{Ctx, PreToolDecision, PreToolUse};
use serde_json::json;

fn first_word(cmd: &str) -> &str {
    cmd.split_whitespace().next().unwrap_or("")
}

fn skip_wrap(cmd: &str, never_wrap: &[String]) -> bool {
    let first = first_word(cmd);
    let base = first.rsplit('/').next().unwrap_or(first);
    if never_wrap.iter().any(|w| w == base) {
        return true;
    }
    if cmd.contains("<<") {
        return true;
    }
    let toks: Vec<&str> = cmd.split_whitespace().collect();
    if toks
        .iter()
        .any(|t| *t == "&" || *t == "-i" || *t == "--interactive")
    {
        return true;
    }
    false
}

/// Wrap a Bash command unless the skip rules fire.
pub fn pre_tool(ev: &PreToolUse<'_>, cx: &Ctx) -> Option<PreToolDecision> {
    if ev.tool_name != "Bash" || !cx.config.plugins.cmd.rewrite {
        return None;
    }
    let cmd = ev.tool_input.get("command")?.as_str()?;
    if skip_wrap(cmd, &cx.config.plugins.cmd.never_wrap) {
        return None;
    }
    let mut input = ev.tool_input.clone();
    input["command"] = json!(format!("rtok run -- {cmd}"));
    Some(PreToolDecision::Rewrite {
        input,
        reason: "wrapped by rtok".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Ctx;

    fn decide(command: &str) -> Option<PreToolDecision> {
        let cx = Ctx::in_memory("wrap").unwrap();
        let input = json!({"command": command, "description": "t"});
        let ev = PreToolUse {
            tool_name: "Bash",
            tool_input: &input,
        };
        pre_tool(&ev, &cx)
    }

    fn wrapped(d: &PreToolDecision) -> &str {
        match d {
            PreToolDecision::Rewrite { input, reason } => {
                assert_eq!(reason, "wrapped by rtok");
                input["command"].as_str().unwrap()
            }
            _ => panic!("{d:?}"),
        }
    }

    #[test]
    fn git_status_is_wrapped() {
        let d = decide("git status").unwrap();
        assert_eq!(wrapped(&d), "rtok run -- git status");
    }

    #[test]
    fn heredoc_and_sudo_untouched() {
        assert!(decide("cat <<EOF").is_none());
        assert!(decide("sudo ls").is_none());
    }
}
