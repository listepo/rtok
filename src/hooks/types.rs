//! Claude Code hook I/O (plan T0.6). Contract: research.md §3 and
//! <https://code.claude.com/docs/en/hooks>. Input is JSON on stdin, output JSON on stdout.
//!
//! Unknown fields round-trip through `extra` so a newer host never breaks parsing.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::plugin::{
    PostToolUse, PreCompact, PreToolUse, PromptSubmit, SessionStart, SubagentStart,
};

/// Union of every hook event's input. Event-specific fields are `Option`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HookInput {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session_id: String,
    /// T129: hooks fired inside a sub-agent carry the parent's `session_id` plus these
    /// (`research.md` §17.2). Absent in the parent window and on hosts without sub-agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    /// SubagentStart's short description of the spawned task, when the host sends one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
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
    /// `postToolUse` → PostToolUse (`tool_output` / `result_json` → `tool_response`).
    pub fn adapt_cursor(&mut self, event: &str) {
        self.take_transcript_path_alias();
        if matches!(event, "afterMCPExecution") || self.hook_event_name == "afterMCPExecution" {
            self.hook_event_name = "AfterMCPExecution".into();
            if self.tool_name.is_none()
                && let Some(n) = self.extra.get("tool_name").and_then(|v| v.as_str())
            {
                self.tool_name = Some(n.to_string());
            }
            if self.tool_response.is_none()
                && let Some(r) = self.extra.get("result_json").and_then(|v| v.as_str())
            {
                self.tool_response = Some(serde_json::Value::String(r.to_string()));
            }
        }
        if self.session_id.is_empty()
            && let Some(id) = self.extra.get("conversation_id").and_then(|v| v.as_str())
        {
            self.session_id = id.to_string();
        }
        if self.tool_response.is_none() {
            if let Some(v) = take_jsonish(&mut self.extra, "tool_output") {
                self.tool_response = Some(v);
            } else if let Some(v) = take_jsonish(&mut self.extra, "result_json") {
                self.tool_response = Some(v);
            }
        }
        if self.tool_name.is_some() {
            if self.tool_input.is_none() {
                self.tool_input = Some(Value::Object(Map::new()));
            }
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
        let after = matches!(cursor_event(event, &self.hook_event_name), "PostToolUse");
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

    /// Devin (CLI and Desktop) sends Claude's snake_case keys with its own values: tools are
    /// `exec`, `read`, `edit`, `write`; a finished call reports `{success, output, error}`;
    /// compaction is the single event `PostCompaction`; the project root is the environment's
    /// `DEVIN_PROJECT_DIR`, passed in as `project_dir`. The reply needs no mapping — Devin reads
    /// `hookSpecificOutput` as Claude writes it.
    pub fn adapt_devin(&mut self, event: &str, project_dir: Option<String>) {
        if let Some(name) = self.tool_name.take() {
            self.tool_name = Some(canonical_tool_name(&name));
        }
        if let Some(Value::Object(resp)) = self.tool_response.as_mut()
            && !resp.contains_key("stdout")
            && let Some(out) = resp.get("output").filter(|v| v.is_string()).cloned()
        {
            resp.insert("stdout".into(), out);
        }
        if self.cwd.is_none() {
            self.cwd = project_dir.filter(|d| !d.is_empty());
        }
        if self.hook_event_name.is_empty() {
            self.hook_event_name = event.to_string();
        }
        if self.hook_event_name == "PostCompaction" {
            self.hook_event_name = "PostCompact".into();
        }
    }

    /// Gemini CLI (https://geminicli.com/docs/hooks/reference/, fetched 2026-09-22) speaks
    /// Claude's field names already (`tool_input`, `prompt`, `trigger`) and its tools' own
    /// path/command keys match Claude's too (verified against gemini-cli's tool source:
    /// `read_file`/`write_file`/`replace` use `file_path`, `run_shell_command` uses `command`)
    /// — only event and tool *names* need mapping (`gemini_event`/`canonical_tool_name`).
    /// `tool_response` is `{llmContent, returnDisplay, error}`; `llmContent` aliases to
    /// `stdout` for plugins keyed on Claude's flat shape.
    pub fn adapt_gemini(&mut self, event: &str) {
        let name = if self.hook_event_name.is_empty() {
            event
        } else {
            self.hook_event_name.as_str()
        };
        self.hook_event_name = gemini_event(name).to_string();
        if let Some(t) = self.tool_name.take() {
            self.tool_name = Some(canonical_tool_name(&t));
        }
        if let Some(Value::Object(resp)) = self.tool_response.as_mut()
            && !resp.contains_key("stdout")
            && let Some(content) = resp.get("llmContent").filter(|v| v.is_string()).cloned()
        {
            resp.insert("stdout".into(), content);
        }
    }

    /// CodeWhale (https://github.com/Hmbown/Codewhale/blob/main/docs/HOOKS.md, fetched
    /// 2026-09-24): of its 15 hook events only `message_submit` sends real stdin JSON *and*
    /// lets a hook steer the outcome — `tool_call_before`/`shell_env` are env-var-only (no
    /// stdin at all) and the seven JSON-bearing observer events (`turn_end`, `subagent_*`,
    /// `session_busy`/`idle`/`error`, `waiting_for_user`) are read-only, Codewhale never acts
    /// on their reply. `message_submit`'s stdin is `{event, text, session_id, workspace, mode,
    /// model, total_tokens}`; only `text` (→ Claude's `prompt`) matters here. See
    /// `src/agents/codewhale/mod.rs` for why every other event stays unwired.
    pub fn adapt_codewhale(&mut self, event: &str) {
        self.hook_event_name = event.to_string();
        if self.prompt.is_none() {
            self.prompt = self.extra.remove("text").and_then(as_string);
        }
    }

    pub fn adapt_cline(&mut self, event: &str) {
        let hook_name = self
            .extra
            .remove("hookName")
            .and_then(as_string)
            .filter(|s| !s.is_empty());
        if self.session_id.is_empty()
            && let Some(id) = self.extra.remove("taskId").and_then(as_string)
        {
            self.session_id = id;
        } else {
            self.extra.remove("taskId");
        }
        if self.cwd.is_none()
            && let Some(Value::Array(roots)) = self.extra.remove("workspaceRoots")
            && let Some(first) = roots.iter().find_map(|r| r.as_str())
        {
            let first = first.trim();
            if !first.is_empty() {
                self.cwd = Some(first.to_string());
            }
        } else {
            self.extra.remove("workspaceRoots");
        }
        if self.tool_name.is_none()
            && let Some(Value::Object(mut call)) = self.extra.remove("tool_call")
        {
            let name = call.remove("name").and_then(as_string);
            let mut input = call.remove("input").unwrap_or(Value::Null);
            let use_id = call.remove("id").and_then(as_string);
            if !call.is_empty() {
                for (k, v) in call {
                    self.extra.insert(k, v);
                }
            }
            if let Some(name) = name {
                self.tool_name = Some(cline_tool_name(&name));
                if self.tool_name.as_deref() == Some("Bash") {
                    single_command(&mut input);
                }
                self.tool_input = Some(input);
            }
            if self.tool_use_id.is_none() {
                self.tool_use_id = use_id;
            }
        }
        if self.tool_response.is_none()
            && let Some(Value::Object(mut result)) = self.extra.remove("tool_result")
        {
            let name = result.remove("name").and_then(as_string);
            let input = result.remove("input");
            let output = result.remove("output");
            let error = result.remove("error");
            let use_id = result.remove("id").and_then(as_string);
            if !result.is_empty() {
                for (k, v) in result {
                    self.extra.insert(k, v);
                }
            }
            if self.tool_name.is_none()
                && let Some(name) = name
            {
                self.tool_name = Some(cline_tool_name(&name));
            }
            if self.tool_input.is_none()
                && let Some(input) = input
            {
                if self.tool_name.as_deref() == Some("Bash") {
                    let mut owned = input;
                    single_command(&mut owned);
                    self.tool_input = Some(owned);
                } else {
                    self.tool_input = Some(input);
                }
            }
            if self.tool_use_id.is_none() {
                self.tool_use_id = use_id;
            }
            let response = match (output, error) {
                (Some(Value::String(o)), Some(Value::String(e))) if !e.is_empty() => {
                    serde_json::json!({"stdout": o, "stderr": e})
                }
                (Some(Value::String(o)), _) => Value::String(o),
                (Some(o), _) => o,
                (None, Some(e)) => e,
                (None, None) => Value::String(String::new()),
            };
            self.tool_response = Some(response);
        }
        let name = hook_name.as_deref().unwrap_or("");
        let name = if name.is_empty() {
            if self.hook_event_name.is_empty() {
                event
            } else {
                self.hook_event_name.as_str()
            }
        } else {
            name
        };
        self.hook_event_name = cline_event(name).to_string();
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

    pub fn subagent_start(&self) -> Option<SubagentStart<'_>> {
        (self.hook_event_name == "SubagentStart").then_some(SubagentStart {
            agent_type: self.agent_type.as_deref().unwrap_or(""),
            task_description: self.task_description.as_deref().unwrap_or(""),
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

    /// Grok Build sends camelCase keys (`sessionId`, `toolName`, `toolInput`, `toolResult`,
    /// `toolUseId`, `permissionMode`, `workspaceRoot`) and its own tool names; only
    /// `hook_event_name` keeps Claude's key and value. The reply needs no mapping — Grok reads
    /// `hookSpecificOutput` as Claude writes it. Only `run_terminal_command` becomes `Bash`:
    /// Grok blocks a call whose `updatedInput` fails the tool schema, and a `Read` rewrite has
    /// not been checked against `read_file`, so file reads keep Grok's name (plan T98, T100).
    pub fn adapt_grok(&mut self, event: &str) {
        let mut lift = |key: &str| self.extra.remove(key);
        let session = lift("sessionId").and_then(as_string);
        let name = lift("toolName").and_then(as_string);
        let input = lift("toolInput");
        let result = lift("toolResult");
        let use_id = lift("toolUseId").and_then(as_string);
        let mode = lift("permissionMode").and_then(as_string);
        let root = lift("workspaceRoot").and_then(as_string);
        if self.session_id.is_empty() {
            self.session_id = session.unwrap_or_default();
        }
        self.tool_name = self.tool_name.take().or(name).map(|n| {
            if n == "run_terminal_command" {
                "Bash".into()
            } else {
                n
            }
        });
        self.tool_input = self.tool_input.take().or(input);
        self.tool_response = self.tool_response.take().or(result);
        self.tool_use_id = self.tool_use_id.take().or(use_id);
        self.permission_mode = self.permission_mode.take().or(mode);
        self.cwd = self.cwd.take().or(root);
        if let Some(Value::Object(resp)) = self.tool_response.as_mut()
            && !resp.contains_key("stdout")
            && let Some(out) = resp
                .get("output_for_prompt")
                .filter(|v| v.is_string())
                .cloned()
        {
            resp.insert("stdout".into(), out);
        }
        if self.hook_event_name.is_empty() {
            self.hook_event_name = event.to_string();
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

/// Cursor stdin `tool_output` / `result_json` is often a JSON string of the result object.
fn take_jsonish(extra: &mut Map<String, Value>, key: &str) -> Option<Value> {
    let v = extra.remove(key)?;
    match v {
        Value::String(s) => serde_json::from_str(&s).ok().or(Some(Value::String(s))),
        other => Some(other),
    }
}

/// Cursor hook names to Claude's; a Claude name passes through.
fn cursor_event<'a>(cli: &'a str, stdin: &'a str) -> &'a str {
    let name = if stdin.is_empty() { cli } else { stdin };
    match name {
        "sessionStart" => "SessionStart",
        "beforeSubmitPrompt" => "UserPromptSubmit",
        "postToolUse" | "PostToolUse" | "afterShellExecution" => "PostToolUse",
        "preToolUse" | "PreToolUse" | "beforeShellExecution" => "PreToolUse",
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

/// Gemini's event names to Claude's (`BeforeAgent`/etc rtok has no plugin hook for pass
/// through unmatched, falling into `dispatch`'s `_` arm); an already-Claude name passes too.
fn gemini_event(name: &str) -> &str {
    match name {
        "BeforeTool" => "PreToolUse",
        "AfterTool" => "PostToolUse",
        "BeforeAgent" => "UserPromptSubmit",
        "PreCompress" => "PreCompact",
        other => other,
    }
}

/// Host tool names (`bash`, `read_file`, `edit`, …) to the Claude names `plugins::guard` matches.
pub(crate) fn canonical_tool_name(name: &str) -> String {
    let l = name.to_ascii_lowercase();
    if l == "exec"
        || l == "run_commands"
        || ["bash", "shell", "terminal", "powershell"]
            .iter()
            .any(|k| l.contains(k))
    {
        "Bash".into()
    } else if l.starts_with("read") || l.starts_with("view") {
        "Read".into()
    } else if l == "edit" || l == "replace" {
        "Edit".into()
    } else if l == "write" || l == "write_file" {
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

/// Cline file hooks send camelCase keys (`hookName`, `taskId`, `workspaceRoots`) with the
/// tool payload nested under `tool_call` / `tool_result` — never Claude's flat keys. The
/// reply needs the same shape translated back (see `cline_output` in `hooks/mod.rs`):
/// Cline never reads `hookSpecificOutput`.
fn cline_tool_name(name: &str) -> String {
    canonical_tool_name(name)
}

/// Cline's event names — both the `hookName` values file hooks send on stdin and the
/// executable file names (`PreToolUse`, `PostToolUse`, `TaskStart`, …) — to Claude's.
/// Tool calls become the tool events; lifecycle events become session/prompt starts and ends.
/// `agent_error`, `agent_abort` and unknown names are no-ops: they reach no plugin.
fn cline_event(name: &str) -> &str {
    match name {
        "tool_call" | "PreToolUse" => "PreToolUse",
        "tool_result" | "PostToolUse" => "PostToolUse",
        "agent_start" | "agent_resume" | "TaskStart" | "TaskResume" | "SessionStart" => {
            "SessionStart"
        }
        "prompt_submit" | "UserPromptSubmit" => "UserPromptSubmit",
        "agent_end" | "session_shutdown" | "TaskComplete" | "SessionShutdown" | "SessionEnd" => {
            "SessionEnd"
        }
        _ => "Noop",
    }
}

/// A one-entry Cline `commands: [cmd]` becomes `command: cmd` so the Bash plugins match.
/// Multi-entry calls pass through untouched: rtok rewrites one command only.
fn single_command(input: &mut Value) {
    if let Value::Object(map) = input
        && !map.contains_key("command")
        && let Some(Value::Array(commands)) = map.get("commands")
        && commands.len() == 1
        && let Some(cmd) = commands.first().cloned()
    {
        map.insert("command".into(), cmd);
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
    /// Cursor `postToolUse` only: replaces an MCP tool result (`updated_mcp_tool_output`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_mcp_tool_output: Option<Value>,
}

/// Shared by hook-output tests here and in `hooks::tests`: one `PreToolUse`-shaped `HookOutput`
/// — a permission decision, an input rewrite, or the bare event name.
#[cfg(test)]
pub(crate) fn pre_out(
    decision: Option<&str>,
    reason: Option<&str>,
    input: Option<Value>,
) -> HookOutput {
    HookOutput {
        hook_specific_output: Some(HookSpecificOutput {
            hook_event_name: "PreToolUse".into(),
            permission_decision: decision.map(Into::into),
            permission_decision_reason: reason.map(Into::into),
            updated_input: input,
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// One `PostToolUse`-shaped `HookOutput` carrying `additionalContext`.
#[cfg(test)]
pub(crate) fn post_out(ctx: &str) -> HookOutput {
    HookOutput {
        hook_specific_output: Some(HookSpecificOutput {
            hook_event_name: "PostToolUse".into(),
            additional_context: Some(ctx.into()),
            ..Default::default()
        }),
        ..Default::default()
    }
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

    /// T129: a sub-agent's hook events carry `agent_id` / `agent_type` beside the parent's
    /// `session_id`; both parse and round-trip, and a payload without them stays as it was.
    #[test]
    fn agent_identity_parses_and_round_trips() {
        let v = serde_json::json!({
            "session_id": "s-1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Read",
            "tool_input": {"file_path": "/p.rs"},
            "agent_id": "a00bd472273a674a6",
            "agent_type": "general-purpose"
        });
        let input: HookInput = serde_json::from_value(v).unwrap();
        assert_eq!(input.agent_id.as_deref(), Some("a00bd472273a674a6"));
        assert_eq!(input.agent_type.as_deref(), Some("general-purpose"));
        let round: HookInput =
            serde_json::from_str(&serde_json::to_string(&input).unwrap()).unwrap();
        assert_eq!(round, input);
        let plain: HookInput =
            serde_json::from_str(r#"{"session_id":"s-2","hook_event_name":"PreToolUse"}"#).unwrap();
        assert_eq!(plain.agent_id, None);
        assert_eq!(plain.agent_type, None);
    }

    /// CodeWhale's `message_submit` stdin has no `hook_event_name`/`prompt` — the CLI arg
    /// supplies the Claude event, `text` becomes `prompt`, everything else round-trips as
    /// `extra` (T185).
    #[test]
    fn adapt_codewhale_maps_text_to_prompt_and_sets_the_event() {
        let mut input: HookInput = serde_json::from_value(serde_json::json!({
            "text": "hello",
            "session_id": "sess_1",
            "workspace": "/w",
            "mode": "ACT"
        }))
        .unwrap();
        input.adapt_codewhale("UserPromptSubmit");
        assert_eq!(input.hook_event_name, "UserPromptSubmit");
        assert_eq!(input.prompt.as_deref(), Some("hello"));
        assert!(input.extra.contains_key("workspace"));
        assert!(!input.extra.contains_key("text"), "text is consumed");
    }

    #[test]
    fn empty_output_is_empty_object() {
        assert_eq!(serde_json::to_string(&HookOutput::default()).unwrap(), "{}");
        let out = pre_out(Some("deny"), Some("dup"), None);
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

    /// Payloads as https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks gives them.
    #[test]
    fn devin_maps_tool_names_result_project_dir_and_compaction() {
        let mut pre: HookInput = serde_json::from_value(serde_json::json!({
            "session_id": "dv-1",
            "prompt_id": "p-1",
            "tool_name": "exec",
            "tool_input": {"command": "git status", "shell_id": "main"}
        }))
        .unwrap();
        pre.adapt_devin("PreToolUse", Some("/work/app".into()));
        assert_eq!(pre.hook_event_name, "PreToolUse");
        assert_eq!(pre.tool_name.as_deref(), Some("Bash"));
        assert_eq!(pre.tool_input.as_ref().unwrap()["shell_id"], "main");
        assert_eq!(pre.cwd.as_deref(), Some("/work/app"));
        assert_eq!(pre.extra.get("prompt_id"), Some(&serde_json::json!("p-1")));
        assert!(pre.pre_tool().is_some());

        let mut post: HookInput = serde_json::from_value(serde_json::json!({
            "session_id": "dv-1",
            "cwd": "/from/stdin",
            "tool_name": "exec",
            "tool_input": {"command": "ls"},
            "tool_response": {"success": true, "output": "a\nb\n", "error": null}
        }))
        .unwrap();
        post.adapt_devin("PostToolUse", Some("/work/app".into()));
        let resp = post.tool_response.as_ref().unwrap();
        assert_eq!(resp["stdout"], "a\nb\n");
        assert_eq!(resp["output"], "a\nb\n");
        assert_eq!(resp["success"], true);
        assert_eq!(post.cwd.as_deref(), Some("/from/stdin"));
        assert!(post.post_tool().is_some());

        let mut other: HookInput = serde_json::from_value(serde_json::json!({
            "tool_name": "mcp__github__execute_query",
            "tool_input": {}
        }))
        .unwrap();
        other.adapt_devin("PreToolUse", None);
        assert_eq!(
            other.tool_name.as_deref(),
            Some("mcp__github__execute_query")
        );
        assert!(other.cwd.is_none());

        let mut compact: HookInput =
            serde_json::from_value(serde_json::json!({"session_id": "dv-1", "summary": null}))
                .unwrap();
        compact.adapt_devin("PostCompaction", None);
        assert_eq!(compact.hook_event_name, "PostCompact");
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

    #[test]
    fn cursor_post_tool_use_maps_mcp_tool_output() {
        let raw = serde_json::json!({
            "hook_event_name": "postToolUse",
            "tool_name": "MCP:list_issues",
            "tool_input": {"team": "eng"},
            "tool_output": "{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}",
            "conversation_id": "sess-mcp",
            "mcp_server_name": "linear"
        });
        let mut input: HookInput = serde_json::from_value(raw).unwrap();
        input.adapt_cursor("PostToolUse");
        assert_eq!(input.session_id, "sess-mcp");
        assert_eq!(input.hook_event_name, "PostToolUse");
        assert_eq!(input.tool_name.as_deref(), Some("MCP:list_issues"));
        assert_eq!(input.tool_input.as_ref().unwrap()["team"], "eng");
        assert_eq!(
            input.tool_response.as_ref().unwrap()["content"][0]["text"],
            "ok"
        );
        assert!(input.post_tool().is_some());
    }

    /// Payloads as Grok Build's hooks guide (`~/.grok/docs/user-guide/10-hooks.md`) gives them.
    #[test]
    fn grok_lifts_camel_case_and_maps_only_the_terminal() {
        let mut pre: HookInput = serde_json::from_value(serde_json::json!({
            "hookEventName": "pre_tool_use",
            "hook_event_name": "PreToolUse",
            "sessionId": "abc-123",
            "cwd": "/Users/you/project",
            "workspaceRoot": "/Users/you/project",
            "permissionMode": "default",
            "toolName": "run_terminal_command",
            "toolInput": {"command": "npm test"},
            "toolUseId": "tu-1",
            "timestamp": "2026-04-14T12:00:00Z"
        }))
        .unwrap();
        pre.adapt_grok("PreToolUse");
        assert_eq!(pre.hook_event_name, "PreToolUse");
        assert_eq!(pre.session_id, "abc-123");
        assert_eq!(pre.tool_name.as_deref(), Some("Bash"));
        assert_eq!(pre.tool_input.as_ref().unwrap()["command"], "npm test");
        assert_eq!(pre.tool_use_id.as_deref(), Some("tu-1"));
        assert_eq!(pre.permission_mode.as_deref(), Some("default"));
        assert_eq!(pre.cwd.as_deref(), Some("/Users/you/project"));
        assert!(pre.extra.contains_key("timestamp"));
        assert!(pre.pre_tool().is_some());

        let mut post: HookInput = serde_json::from_value(serde_json::json!({
            "hook_event_name": "PostToolUse",
            "sessionId": "abc-123",
            "workspaceRoot": "/w",
            "toolName": "run_terminal_command",
            "toolInput": {"command": "ls"},
            "toolResult": {"type": "Bash", "command": "ls", "exit_code": 0, "output_for_prompt": "a\nb\n"}
        }))
        .unwrap();
        post.adapt_grok("SessionStart");
        assert_eq!(post.hook_event_name, "PostToolUse");
        assert_eq!(post.cwd.as_deref(), Some("/w"));
        assert_eq!(post.tool_response.as_ref().unwrap()["stdout"], "a\nb\n");
        assert!(post.post_tool().is_some());

        let mut read: HookInput = serde_json::from_value(serde_json::json!({
            "toolName": "read_file",
            "toolInput": {"path": "src/main.rs"}
        }))
        .unwrap();
        read.adapt_grok("PreToolUse");
        assert_eq!(read.hook_event_name, "PreToolUse");
        assert_eq!(read.tool_name.as_deref(), Some("read_file"));
    }

    /// Payloads as https://geminicli.com/docs/hooks/reference/ gives them.
    #[test]
    fn gemini_before_after_tool_map_event_tool_name_and_alias_llm_content_to_stdout() {
        let mut pre: HookInput = serde_json::from_value(serde_json::json!({
            "session_id": "g-1",
            "hook_event_name": "BeforeTool",
            "cwd": "/work",
            "tool_name": "run_shell_command",
            "tool_input": {"command": "ls"}
        }))
        .unwrap();
        pre.adapt_gemini("BeforeTool");
        assert_eq!(pre.hook_event_name, "PreToolUse");
        assert_eq!(pre.tool_name.as_deref(), Some("Bash"));
        assert!(pre.pre_tool().is_some());

        let mut post: HookInput = serde_json::from_value(serde_json::json!({
            "session_id": "g-1",
            "hook_event_name": "AfterTool",
            "tool_name": "run_shell_command",
            "tool_input": {"command": "ls"},
            "tool_response": {"llmContent": "a\nb\n", "returnDisplay": "a\nb\n", "error": null}
        }))
        .unwrap();
        post.adapt_gemini("AfterTool");
        assert_eq!(post.hook_event_name, "PostToolUse");
        assert_eq!(post.tool_name.as_deref(), Some("Bash"));
        assert_eq!(post.tool_response.as_ref().unwrap()["stdout"], "a\nb\n");
        assert_eq!(post.tool_response.as_ref().unwrap()["llmContent"], "a\nb\n");
        assert!(post.post_tool().is_some());

        let mut read: HookInput = serde_json::from_value(serde_json::json!({
            "hook_event_name": "BeforeTool",
            "tool_name": "read_file",
            "tool_input": {"file_path": "/p.rs"}
        }))
        .unwrap();
        read.adapt_gemini("BeforeTool");
        assert_eq!(read.tool_name.as_deref(), Some("Read"));
        assert!(read.pre_tool().is_some());

        assert_eq!(canonical_tool_name("write_file"), "Write");
        assert_eq!(canonical_tool_name("replace"), "Edit");
    }

    #[test]
    fn gemini_before_agent_and_pre_compress_map_to_claude_names() {
        let mut prompt: HookInput = serde_json::from_value(serde_json::json!({
            "hook_event_name": "BeforeAgent",
            "prompt": "fix the bug"
        }))
        .unwrap();
        prompt.adapt_gemini("BeforeAgent");
        assert_eq!(prompt.hook_event_name, "UserPromptSubmit");
        assert!(prompt.prompt_submit().is_some());

        let mut compress: HookInput = serde_json::from_value(serde_json::json!({
            "hook_event_name": "PreCompress",
            "trigger": "auto"
        }))
        .unwrap();
        compress.adapt_gemini("PreCompress");
        assert_eq!(compress.hook_event_name, "PreCompact");
        assert!(compress.pre_compact().is_some());
    }

    /// Payloads as Cline's file-hook README gives them: `hookName` + `taskId` +
    /// `workspaceRoots` with the tool nested under `tool_call` / `tool_result`.
    #[test]
    fn cline_maps_tool_call_result_and_lifecycle() {
        let mut pre: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "tool_call",
            "taskId": "task-1",
            "workspaceRoots": ["/work/app"],
            "tool_call": {"id": "tc-1", "name": "run_commands", "input": {"commands": ["git status"]}}
        }))
        .unwrap();
        pre.adapt_cline("PreToolUse");
        assert_eq!(pre.hook_event_name, "PreToolUse");
        assert_eq!(pre.session_id, "task-1");
        assert_eq!(pre.cwd.as_deref(), Some("/work/app"));
        assert_eq!(pre.tool_name.as_deref(), Some("Bash"));
        assert_eq!(pre.tool_input.as_ref().unwrap()["command"], "git status");
        assert_eq!(
            pre.tool_input.as_ref().unwrap()["commands"][0],
            "git status"
        );
        assert_eq!(pre.tool_use_id.as_deref(), Some("tc-1"));
        assert!(pre.pre_tool().is_some());
        assert!(!pre.extra.contains_key("hookName"));
        assert!(!pre.extra.contains_key("taskId"));

        let mut multi: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "tool_call",
            "taskId": "task-1",
            "workspaceRoots": ["/work/app"],
            "tool_call": {"id": "tc-2", "name": "run_commands", "input": {"commands": ["a", "b"]}}
        }))
        .unwrap();
        multi.adapt_cline("PreToolUse");
        assert_eq!(multi.tool_name.as_deref(), Some("Bash"));
        assert!(multi.tool_input.as_ref().unwrap().get("command").is_none());

        let mut read: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "tool_call",
            "taskId": "task-1",
            "tool_call": {"id": "tc-3", "name": "read_files", "input": {"paths": ["src/main.rs"]}}
        }))
        .unwrap();
        read.adapt_cline("PreToolUse");
        assert_eq!(read.tool_name.as_deref(), Some("Read"));

        let mut foreign: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "tool_call",
            "taskId": "task-1",
            "tool_call": {"id": "tc-4", "name": "mcp__linear__list", "input": {}}
        }))
        .unwrap();
        foreign.adapt_cline("PreToolUse");
        assert_eq!(foreign.tool_name.as_deref(), Some("mcp__linear__list"));

        let mut post: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "tool_result",
            "taskId": "task-1",
            "workspaceRoots": ["/work/app"],
            "tool_result": {
                "id": "tc-1",
                "name": "run_commands",
                "input": {"commands": ["git status"]},
                "output": "clean\n",
                "durationMs": 12
            }
        }))
        .unwrap();
        post.adapt_cline("PostToolUse");
        assert_eq!(post.hook_event_name, "PostToolUse");
        assert_eq!(post.tool_name.as_deref(), Some("Bash"));
        assert_eq!(post.tool_response.as_ref().unwrap(), "clean\n");
        assert!(post.post_tool().is_some());

        let mut err: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "tool_result",
            "taskId": "task-1",
            "tool_result": {
                "id": "tc-1",
                "name": "run_commands",
                "input": {"commands": ["git status"]},
                "output": "out\n",
                "error": "boom"
            }
        }))
        .unwrap();
        err.adapt_cline("PostToolUse");
        assert_eq!(err.tool_response.as_ref().unwrap()["stdout"], "out\n");
        assert_eq!(err.tool_response.as_ref().unwrap()["stderr"], "boom");

        let mut start: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "agent_start",
            "taskId": "task-1",
            "workspaceRoots": ["/work/app"]
        }))
        .unwrap();
        start.adapt_cline("TaskStart");
        assert_eq!(start.hook_event_name, "SessionStart");

        let mut abort: HookInput = serde_json::from_value(serde_json::json!({
            "hookName": "agent_abort",
            "taskId": "task-1"
        }))
        .unwrap();
        abort.adapt_cline("TaskCancel");
        assert_eq!(abort.hook_event_name, "Noop");

        // The executable file name works when `hookName` is absent (fail open never panics).
        let mut bare: HookInput = serde_json::from_value(serde_json::json!({
            "taskId": "task-9",
            "tool_call": {"id": "tc-9", "name": "run_commands", "input": {"commands": ["ls"]}}
        }))
        .unwrap();
        bare.adapt_cline("PreToolUse");
        assert_eq!(bare.hook_event_name, "PreToolUse");
        assert_eq!(bare.session_id, "task-9");
    }
}
