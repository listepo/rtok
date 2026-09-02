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
    /// Cursor `beforeShellExecution`: top-level `command` + `conversation_id`.
    /// Claude PreToolUse: `tool_name=Bash` + `tool_input.command`.
    pub fn adapt_cursor(&mut self, event: &str) {
        if self.session_id.is_empty()
            && let Some(id) = self.extra.get("conversation_id").and_then(|v| v.as_str())
        {
            self.session_id = id.to_string();
        }
        if self.tool_name.is_some() {
            if self.hook_event_name.is_empty() {
                self.hook_event_name = event.to_string();
            }
            return;
        }
        if let Some(cmd) = self
            .extra
            .get("command")
            .and_then(|v| v.as_str())
            .map(str::to_string)
        {
            self.tool_name = Some("Bash".into());
            self.tool_input = Some(serde_json::json!({"command": cmd}));
            self.hook_event_name = "PreToolUse".into();
        } else if self.hook_event_name.is_empty() {
            self.hook_event_name = event.to_string();
        }
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

    pub fn pre_compact(&self) -> Option<PreCompact<'_>> {
        (self.hook_event_name == "PreCompact").then_some(PreCompact {
            trigger: self.trigger.as_deref().unwrap_or("auto"),
            transcript_path: self.transcript_path.as_deref()?,
        })
    }
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
        assert_eq!(fx.len(), 7, "expected 7 event fixtures");
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
}
