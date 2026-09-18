//! PreToolUse(Bash) rewrite to `rtok run --` (plan T3.4).

use rtok_plugin_sdk::{Ctx, PreToolDecision, PreToolUse};
use serde_json::json;

fn skip_wrap(cmd: &str, never_wrap: &[String]) -> bool {
    skip_wrap_host(cfg!(windows), cmd, never_wrap)
}

/// `windows` is a parameter so both host contracts stay tested on one toolchain
/// (T55.12): a PowerShell `''` rewrite must never reach a POSIX shell, where
/// `'echo it''s fine'` concatenates to `echo its fine` and silently drops the
/// apostrophe — so any command containing `'` stays unwrapped there.
fn skip_wrap_host(windows: bool, cmd: &str, never_wrap: &[String]) -> bool {
    let mut toks = cmd.split_whitespace();
    let first = toks.next().unwrap_or("");
    // Same stem rules as formatters::cmd_stem / run::shell_kind: Windows argv may
    // be `C:\…\sudo.exe` while never_wrap lists bare `sudo`.
    let base = super::formatters::cmd_stem(first);
    if never_wrap.iter().any(|w| w.eq_ignore_ascii_case(base)) {
        return true;
    }
    if cmd.contains("<<") {
        return true;
    }
    // T55.12: the PowerShell `''` form is lossy if the rewritten command reaches a
    // POSIX shell — Claude Code on Windows runs Bash through Git Bash, where
    // `'echo it''s fine'` concatenates to `echo its fine`. Nothing containing an
    // apostrophe is wrapped there; staying whole is the safe direction.
    if windows && cmd.contains('\'') {
        return true;
    }
    if std::iter::once(first)
        .chain(toks)
        .any(|t| matches!(t, "&" | "-i" | "--interactive"))
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
    input["command"] = json!(format!("rtok run -- {}", super::run::wrap_quote(cmd)));
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
                format!("rtok run -- {}", super::super::run::wrap_quote(cmd))
            );
        }
    }

    #[test]
    fn trailing_ampersand_untouched() {
        assert!(decide("sleep 10 &").is_none());
        assert!(decide("sleep 10&").is_none());
        assert!(decide("true && false").is_some(), "&& is not background");
    }

    #[test]
    fn windows_path_and_exe_honor_never_wrap() {
        assert!(decide(r"C:\Windows\System32\sudo.exe ls").is_none());
        assert!(decide(r"C:\tools\rtok.exe run -- true").is_none());
        assert!(decide("sudo.exe ls").is_none());
        // Still wrap a normal command with a Windows-looking path.
        assert!(decide(r"C:\Program Files\Git\cmd\git.exe status").is_some());
    }

    #[test]
    fn sudo_exe_case_insensitive_never_wrap() {
        assert!(decide("Sudo.exe ls").is_none());
        assert!(decide("SUDO.EXE ls").is_none());
        assert!(decide("RTOK.EXE run -- true").is_none());
        assert!(decide("Sudo ls").is_none());
    }

    #[test]
    fn wrap_keeps_apostrophe_host_safe() {
        let cmd = "echo it's fine";
        match decide(cmd) {
            // Windows (T55.12): nothing is wrapped, so no PowerShell `''` form
            // can reach a POSIX host shell. The host contracts are pinned by
            // `windows_apostrophe_commands_stay_unwrapped`; here the wrapped
            // POSIX path only has to round-trip through the real quoter.
            None => {}
            Some(d) => {
                let w = wrapped(&d);
                assert!(w.starts_with("rtok run -- "), "{w}");
                let q = &w["rtok run -- ".len()..];
                assert_eq!(q, &super::super::run::wrap_quote(cmd));
            }
        }
    }

    #[test]
    fn windows_apostrophe_commands_stay_unwrapped() {
        assert!(skip_wrap_host(true, "echo it's fine", &[]));
        assert!(skip_wrap_host(true, "git commit -m 'fix it'", &[]));
        // The rule fires regardless of position; other skips still apply first.
        assert!(skip_wrap_host(true, "jq '.' data.json", &[]));
        // A POSIX host keeps wrapping apostrophe commands: sh quoting round-trips.
        assert!(!skip_wrap_host(false, "echo it's fine", &[]));
        assert!(!skip_wrap_host(false, "git commit -m 'fix it'", &[]));
    }

    /// Parse-simulation of the loss the fix removes: the PowerShell `''` form,
    /// split as POSIX sh words, concatenates the adjacent quotes into one word
    /// and the apostrophe is gone — the command that runs is not the command
    /// the model asked for.
    #[test]
    fn ps_quoting_does_not_round_trip_under_sh() {
        fn sh_words(input: &str) -> Vec<String> {
            let mut words = Vec::new();
            let mut word = String::new();
            let mut in_word = false;
            let mut chars = input.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\'' => {
                        in_word = true;
                        for q in chars.by_ref() {
                            if q == '\'' {
                                break;
                            }
                            word.push(q);
                        }
                    }
                    // A minimal double-quote pass so the POSIX `'"'"'` embedding
                    // (an apostrophe inside `"'"`) parses the way sh reads it.
                    '"' => {
                        in_word = true;
                        while let Some(q) = chars.next() {
                            if q == '"' {
                                break;
                            }
                            if q == '\\' {
                                if let Some(esc) = chars.next() {
                                    word.push(esc);
                                }
                            } else {
                                word.push(q);
                            }
                        }
                    }
                    c if c.is_whitespace() => {
                        if in_word {
                            words.push(std::mem::take(&mut word));
                            in_word = false;
                        }
                    }
                    c => {
                        in_word = true;
                        word.push(c);
                    }
                }
            }
            if in_word {
                words.push(word);
            }
            words
        }
        let cmd = "echo it's fine";
        let ps_form = format!("'{}'", cmd.replace('\'', "''"));
        assert_eq!(sh_words(&ps_form), ["echo its fine"]);
        assert_ne!(sh_words(&ps_form), [cmd]);
        // The POSIX form does round-trip, which is why only the Windows path skips.
        assert_eq!(sh_words(&super::super::run::sh_quote(cmd)), [cmd]);
    }
}
