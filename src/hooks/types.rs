//! Claude Code hook I/O (plan T0.6). Contract: research.md §3 and
//! <https://code.claude.com/docs/en/hooks>. Input is JSON on stdin, output JSON on stdout.
//!
//! Unknown fields round-trip through `extra` so a newer host never breaks parsing.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::plugin::{PostToolUse, PreCompact, PreToolUse, PromptSubmit, SessionStart};

/// Union of every hook event's input. Event-specific fields are `Option`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HookInput {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hook_event_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    // PreToolUse / PostToolUse
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_response: Option<Value>,
    // UserPromptSubmit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    // SessionStart
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    // PreCompact / PostCompact
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl HookInput {
    /// Cursor shell hooks: top-level `command` + `conversation_id`.
    /// `beforeShellExecution` → Claude PreToolUse (`tool_name=Bash`, `tool_input.command`).
    /// `afterShellExecution` → Claude PostToolUse (+ `tool_response` from `output`/`stdout`).
    pub fn adapt_cursor(&mut self, event: &str) {
        self.take_transcript_path_alias();
        if matches!(event, "afterMCPExecution") || self.hook_event_name == "afterMCPExecution" {
            self.hook_event_name = "AfterMCPExecution".into();
            if self.tool_name.is_none() {
                if let Some(n) = self.extra.get("tool_name").and_then(|v| v.as_str()) {
                    self.tool_name = Some(n.to_string());
                }
            }
            if self.tool_response.is_none() {
                if let Some(r) = self.extra.get("result_json").and_then(|v| v.as_str()) {
                    self.tool_response = Some(serde_json::Value::String(r.to_string()));
                }
            }
        }
        if self.session_id.is_empty()
            && let Some(id) = self.extra.get("conversation_id").and_then(|v| v.as_str())
        {
            self.session_id = id.to_string();
        }
        if self.tool_name.is_some() {
            self.hook_event_name = cursor_event(event, &self.hook_event_name).into();
            return;
        }
        let Some(cmd) = self
            .extra
            .get("command")
            .and_then(|v| v.as_str())
            .map(str::to_string)
        else {
            self.hook_event_name = cursor_event(event, &self.hook_event_name).into();
            return;
        };
        self.tool_name = Some("Bash".into());
        self.tool_input = Some(serde_json::json!({"command": cmd}));
        let after = matches!(event, "PostToolUse" | "afterShellExecution")
            || self.hook_event_name == "afterShellExecution";
        if after {
            self.hook_event_name = "PostToolUse".into();
            if self.tool_response.is_none() {
                self.tool_response = Some(
                    self.extra
                        .get("output")
                        .or_else(|| self.extra.get("stdout"))
                        .cloned()
                        .unwrap_or_else(|| Value::String(String::new())),
                );
            }
        } else {
            self.hook_event_name = "PreToolUse".into();
        }
    }

    /// GitHub Copilot CLI hooks speak camelCase: stdin `sessionId`, `cwd`, `toolName`,
    /// `toolArgs` (`toolResult` after the call); the Claude event is the one `rtok hook <Event>`
    /// was invoked with. Shell tools become `Bash` and file reads `Read`, so the plugins see the
    /// names they match on. Fields that are not mapped stay in `extra` and round-trip.
    pub fn adapt_copilot(&mut self, event: &str) {
        if self.session_id.is_empty()
            && let Some(id) = self.extra.remove("sessionId").and_then(as_string)
        {
            self.session_id = id;
        }
        if self.tool_name.is_none()
            && let Some(name) = self.extra.remove("toolName").and_then(as_string)
        {
            self.tool_name = Some(copilot_tool_name(&name));
        }
        if self.tool_input.is_none()
            && let Some(args) = self.extra.remove("toolArgs")
        {
            self.tool_input = Some(args);
        }
        if self.tool_response.is_none()
            && let Some(result) = self.extra.remove("toolResult")
        {
            self.tool_response = Some(result);
        }
        let name = if self.hook_event_name.is_empty() {
            event
        } else {
            self.hook_event_name.as_str()
        };
        self.hook_event_name = claude_event(name).to_string();
    }

    pub fn pre_tool(&self) -> Option<PreToolUse<'_>> {
        (self.hook_event_name == "PreToolUse").then_some(PreToolUse {
            tool_name: self.tool_name.as_deref()?,
            tool_input: self.tool_input.as_ref()?,
        })
    }

    pub fn post_tool(&self) -> Option<PostToolUse<'_>> {
        (self.hook_event_name == "PostToolUse").then_some(PostToolUse {
            tool_name: self.tool_name.as_deref()?,
            tool_input: self.tool_input.as_ref()?,
            tool_response: self.tool_response.as_ref()?,
        })
    }

    pub fn session_start(&self) -> Option<SessionStart<'_>> {
        (self.hook_event_name == "SessionStart").then_some(SessionStart {
            source: self.source.as_deref().unwrap_or("startup"),
        })
    }

    pub fn prompt_submit(&self) -> Option<PromptSubmit<'_>> {
        (self.hook_event_name == "UserPromptSubmit").then_some(PromptSubmit {
            prompt: self.prompt.as_deref()?,
        })
    }

    pub fn mcp_server_name(&self) -> Option<&str> {
        self.extra.get("mcp_server_name").and_then(|v| v.as_str())
    }

    pub fn take_transcript_path_alias(&mut self) {
        if self.transcript_path.is_none()
            && let Some(path) = self.extra.remove("transcriptPath").and_then(as_string)
        {
            self.transcript_path = Some(path);
        }
    }

    pub fn pre_compact(&self) -> Option<PreCompact<'_>> {
        (self.hook_event_name == "PreCompact").then_some(PreCompact {
            trigger: self.trigger.as_deref().unwrap_or("auto"),
            transcript_path: self.transcript_path.as_deref().unwrap_or(""),
        })
    }
}

fn as_string(v: Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s),
        _ => None,
    }
}

/// Cursor hook names to Claude's; a Claude name passes through.
fn cursor_event<'a>(cli: &'a str, stdin: &'a str) -> &'a str {
    let name = if stdin.is_empty() { cli } else { stdin };
    match name {
        "sessionStart" => "SessionStart",
        "beforeSubmitPrompt" => "UserPromptSubmit",
        "afterShellExecution" => "PostToolUse",
        "beforeShellExecution" => "PreToolUse",
        "afterMCPExecution" => "AfterMCPExecution",
        "preCompact" => "PreCompact",
        other => other,
    }
}

/// Copilot's event names, camelCase, to Claude's; a Claude name passes through.
fn claude_event(name: &str) -> &str {
    match name {
        "preToolUse" => "PreToolUse",
        "postToolUse" => "PostToolUse",
        "sessionStart" => "SessionStart",
        "sessionEnd" => "SessionEnd",
        "userPromptSubmitted" => "UserPromptSubmit",
        "preCompact" => "PreCompact",
        other => other,
    }
}

/// Host tool names (`bash`, `read_file`, `edit`, …) to the Claude names `plugins::guard` matches.
pub(crate) fn canonical_tool_name(name: &str) -> String {
    let l = name.to_ascii_lowercase();
    if ["bash", "shell", "terminal", "powershell"]
        .iter()
        .any(|k| l.contains(k))
    {
        "Bash".into()
    } else if l.starts_with("read") || l.starts_with("view") {
        "Read".into()
    } else if l == "edit" {
        "Edit".into()
    } else if l == "write" {
        "Write".into()
    } else {
        name.to_string()
    }
}

/// Copilot's tool names are its own (`bash`, `run_in_terminal`, `read_file`, `view`, …); the
/// plugins match on Claude's `Bash` and `Read`. Anything else keeps its name.
fn copilot_tool_name(name: &str) -> String {
    canonical_tool_name(name)
}

/// Hook stdout. `HookOutput::default()` serialises to `{}` (= no opinion, fail open).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    #[serde(rename = "continue", default, skip_serializing_if = "Option::is_none")]
    pub continue_: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_mcp_tool_output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    pub hook_event_name: String,
    /// `allow` | `deny` | `ask` (PreToolUse only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,
    /// Replacement `tool_input` (PreToolUse only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> Vec<(String, Value)> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hooks");
        let mut out: Vec<(String, Value)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .map(|p| {
                let text = std::fs::read_to_string(&p).unwrap();
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    serde_json::from_str(&text).unwrap(),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[test]
    fn every_fixture_round_trips_unchanged() {
        let fx = fixtures();
        assert_eq!(fx.len(), 9, "expected 9 event fixtures");
        for (name, original) in fx {
            let parsed: HookInput = serde_json::from_value(original.clone()).unwrap();
            let back = serde_json::to_value(&parsed).unwrap();
            assert_eq!(back, original, "{name} changed after round trip");
        }
    }

    #[test]
    fn event_views_match_event_name() {
        for (name, v) in fixtures() {
            let input: HookInput = serde_json::from_value(v).unwrap();
            let seen = [
                input.pre_tool().is_some(),
                input.post_tool().is_some(),
                input.session_start().is_some(),
                input.prompt_submit().is_some(),
                input.pre_compact().is_some(),
            ]
            .iter()
            .filter(|b| **b)
            .count();
            let expected = usize::from(input.hook_event_name != "PostCompact");
            assert_eq!(
                seen, expected,
                "{name}: exactly one view (none for PostCompact)"
            );
        }
    }

    #[test]
    fn empty_output_is_empty_object() {
        assert_eq!(serde_json::to_string(&HookOutput::default()).unwrap(), "{}");
        let out = HookOutput {
            hook_specific_output: Some(HookSpecificOutput {
                hook_event_name: "PreToolUse".into(),
                permission_decision: Some("deny".into()),
                permission_decision_reason: Some("dup".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(json.get("continue").is_none());
        assert_eq!(serde_json::from_value::<HookOutput>(json).unwrap(), out);
    }

    #[test]
    fn cursor_after_shell_maps_to_post_tool_use() {
        let raw = serde_json::json!({
            "hook_event_name": "afterShellExecution",
            "command": "ls -la",
            "output": "total 0\n",
            "conversation_id": "sess-after",
            "cwd": "/tmp"
        });
        let mut input: HookInput = serde_json::from_value(raw).unwrap();
        input.adapt_cursor("PostToolUse");
        assert_eq!(input.session_id, "sess-after");
        assert_eq!(input.hook_event_name, "PostToolUse");
        assert_eq!(input.tool_name.as_deref(), Some("Bash"));
        assert_eq!(input.tool_input.as_ref().unwrap()["command"], "ls -la");
        assert_eq!(input.tool_response.as_ref().unwrap(), "total 0\n");
        assert!(input.post_tool().is_some());
        assert!(input.pre_tool().is_none());
    }

    #[test]
    fn copilot_pre_tool_use_maps_camel_case_and_tool_names() {
        let raw = serde_json::json!({
            "sessionId": "cp-1",
            "timestamp": 1,
            "cwd": "/tmp",
            "toolName": "run_in_terminal",
            "toolArgs": {"command": "git status"}
        });
        let mut input: HookInput = serde_json::from_value(raw).unwrap();
        input.adapt_copilot("PreToolUse");
        assert_eq!(input.session_id, "cp-1");
        assert_eq!(input.hook_event_name, "PreToolUse");
        assert_eq!(input.tool_name.as_deref(), Some("Bash"));
        assert_eq!(input.tool_input.as_ref().unwrap()["command"], "git status");
        assert_eq!(input.extra.get("timestamp"), Some(&serde_json::json!(1)));
        assert!(input.pre_tool().is_some());

        let mut after: HookInput = serde_json::from_value(serde_json::json!({
            "sessionId": "cp-1",
            "toolName": "view",
            "toolArgs": {"path": "a.rs"},
            "toolResult": {"textResultForLlm": "fn main() {}"}
        }))
        .unwrap();
        after.adapt_copilot("postToolUse");
        assert_eq!(after.hook_event_name, "PostToolUse");
        assert_eq!(after.tool_name.as_deref(), Some("Read"));
        assert!(after.post_tool().is_some());
        assert_eq!(copilot_tool_name("web_search"), "web_search");
    }

    #[test]
    fn cursor_before_shell_still_maps_to_pre_tool_use() {
        let raw = serde_json::json!({
            "hook_event_name": "beforeShellExecution",
            "command": "pwd",
            "conversation_id": "sess-before"
        });
        let mut input: HookInput = serde_json::from_value(raw).unwrap();
        input.adapt_cursor("PreToolUse");
        assert_eq!(input.hook_event_name, "PreToolUse");
        assert!(input.pre_tool().is_some());
        assert!(input.post_tool().is_none());
        assert!(input.tool_response.is_none());
    }
}
