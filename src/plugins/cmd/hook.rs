//! PreToolUse(Bash) rewrite to `rtok run --` (plan T3.4).

use rtok_plugin_sdk::{Ctx, PreToolDecision, PreToolUse};
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
    // AGENTS: trailing `&` (background) — also `sleep 10&` with no space before `&`.
    // Do not treat `&&` as background.
    let t = cmd.trim_end();
    if t.ends_with('&') && !t.ends_with("&&") {
        return true;
    }
    false
}

/// Wrap a Bash command unless the skip rules fire.
pub fn pre_tool(ev: &PreToolUse<'_>, cx: &Ctx) -> Option<PreToolDecision> {
    let cfg = cx.plugin_config::<crate::config::Cmd>("cmd");
    if ev.tool_name != "Bash" || !cfg.rewrite {
        return None;
    }
    let cmd = ev.tool_input.get("command")?.as_str()?;
    if skip_wrap(cmd, &cfg.never_wrap) {
        return None;
    }
    let mut input = ev.tool_input.clone();
    // One argv so the outer shell cannot split on `&&`, `|`, `;`, or redirects.
    input["command"] = json!(format!("rtok run -- {}", super::run::sh_quote(cmd)));
    Some(PreToolDecision::Rewrite {
        input,
        reason: "wrapped by rtok".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Ctx;
    fn decide(command: &str) -> Option<PreToolDecision> {
        let cx = crate::plugin::Runtime::in_memory("wrap").unwrap();
        let input = json!({"command": command, "description": "t"});
        let ev = PreToolUse {
            tool_name: "Bash",
            tool_input: &input,
        };
        pre_tool(&ev, &Ctx::new(&cx))
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
        assert_eq!(wrapped(&d), "rtok run -- 'git status'");
    }

    #[test]
    fn heredoc_and_sudo_untouched() {
        assert!(decide("cat <<EOF").is_none());
        assert!(decide("sudo ls").is_none());
    }

    #[test]
    fn metacharacters_stay_inside_one_quoted_argument() {
        for cmd in [
            "true && false",
            "git status | head",
            "foo; bar",
            "echo x >/tmp/x",
        ] {
            let d = decide(cmd).unwrap();
            assert_eq!(
                wrapped(&d),
                format!("rtok run -- {}", super::super::run::sh_quote(cmd))
            );
        }
    }

    #[test]
    fn trailing_ampersand_untouched() {
        assert!(decide("sleep 10 &").is_none());
        assert!(decide("sleep 10&").is_none());
        assert!(decide("true && false").is_some(), "&& is not background");
    }
}
