//! The rtok plugin contract: what a plugin is, the events it answers, and what it may
//! hand back to the host.
//!
//! [rtok](https://github.com/listepo/rtok) reduces the tokens an AI coding agent spends, and
//! every method it uses is a plugin. This crate is the contract those plugins implement —
//! the ten that ship inside `rtok` and any written elsewhere — so a plugin author depends on
//! three small crates instead of the whole binary.
//!
//! # The rules a plugin lives by
//!
//! - **Fail open.** A hook exits 0 in ≤ 10 ms even on error, with unmodified input. A plugin
//!   that panics or blocks is a plugin that breaks the user's session.
//! - **Lossless by default.** Anything shortened stays retrievable; put the handle in
//!   [`Measurement::ref_id`].
//! - **A saving that is not a [`Measurement`] row does not exist.** Record what you changed,
//!   before and after, or it did not happen.
//!
//! # Status
//!
//! The [`Plugin`] trait itself still lives in `rtok` while the host capabilities move behind
//! traits (T23.3); this crate already owns every type in its signatures, and `rtok::plugin`
//! re-exports them, so the two paths are one type today.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::Serialize;
use serde_json::Value;

/// Where a plugin is reachable from (decision D2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// `rtok hook <event>`
    Hook,
    /// `rtok mcp`
    Mcp,
    /// `rtok proxy`
    Proxy,
    /// A subcommand such as `rtok run`, `rtok stats`
    Cli,
}

impl Surface {
    /// Lower-case name used in the `rtok plugins` table and in config keys.
    pub fn as_str(self) -> &'static str {
        match self {
            Surface::Hook => "hook",
            Surface::Mcp => "mcp",
            Surface::Proxy => "proxy",
            Surface::Cli => "cli",
        }
    }
}

/// What a plugin declares about itself: its id, the surfaces it answers on, and whether it
/// is on unless configuration says otherwise.
#[derive(Clone, Debug)]
pub struct Manifest {
    /// Catalogue id, e.g. `cmd`. Also the Cargo feature name and the `[plugins.<id>]` key.
    pub id: &'static str,
    /// Surfaces this plugin answers on; anything else uses the trait's no-op defaults.
    pub surfaces: &'static [Surface],
    /// Enabled unless `[plugins.<id>] enabled` says otherwise.
    pub default_on: bool,
}

/// A before/after pair produced by one plugin action (decision D3).
///
/// This is the only evidence of a saving the host accepts. `before`/`after` are the payload
/// the model would have seen; the estimates are the host's token estimate of each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measurement {
    /// Catalogue id of the plugin that made the change.
    pub plugin: &'static str,
    /// Plugin-defined subtype, e.g. `formatter`, `rule`, `raw`, `inject`, `dedup`.
    pub kind: &'static str,
    /// Payload size before the change.
    pub before_bytes: u64,
    /// Payload size after the change.
    pub after_bytes: u64,
    /// Estimated tokens before the change.
    pub est_before: u32,
    /// Estimated tokens after the change.
    pub est_after: u32,
    /// Archive id (or other handle) that makes the saving reversible.
    pub ref_id: Option<String>,
    /// Owning `calls.id` when the plugin has one.
    pub call_id: Option<i32>,
}

/// What a plugin may do to a PreToolUse event. First `Deny` wins; `Rewrite` is last-writer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreToolDecision {
    /// Block the call; `reason` is shown to the model.
    Deny {
        /// Why the call was blocked.
        reason: String,
    },
    /// Replace `tool_input` with something cheaper but equivalent.
    Rewrite {
        /// The replacement `tool_input`.
        input: Value,
        /// Why it was rewritten.
        reason: String,
    },
}

/// Text offered for injection into the model's context; emitted in priority order until the
/// host's per-turn budget is spent (decision D5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Injection {
    /// Catalogue id of the offering plugin.
    pub plugin: &'static str,
    /// The text itself; must be byte-stable across turns, or it busts the prompt cache.
    pub text: String,
    /// Higher first.
    pub priority: u8,
}

/// The page a plugin contributes to the operator surfaces — `rtok web` and `rtok tui` render
/// the same one (decision D23). Stats are attached by the host from `Measurement` rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DashboardPage {
    /// Short human title, e.g. `Bash / cmd`.
    pub title: String,
    /// One sentence on what the plugin does for the operator reading it.
    pub summary: String,
    /// Whether the shared stats widget applies: a plugin with no `Measurement` path shows none.
    pub saves_tokens: bool,
    /// Extra key/value rows, typically the plugin's effective configuration.
    pub fields: Vec<(String, String)>,
}

impl DashboardPage {
    /// Catalogue copy for a built-in id; anything else gets a generic page.
    ///
    /// Kept while the ten catalogue plugins still rely on it. T23.2 moves each plugin's copy
    /// to the plugin and makes `Plugin::dashboard_page` required.
    pub fn from_id(id: &str) -> Self {
        let (title, summary, saves_tokens) = match id {
            "measure" => (
                "Measure",
                "Only Measurement rows count as savings. Stats from transcripts and proxy usage.",
                false,
            ),
            "cmd" => (
                "Bash / cmd",
                "Archive and filter command output; expand the original by id.",
                true,
            ),
            "read" => (
                "Read",
                "Outline, map, and search instead of dumping full files into context.",
                true,
            ),
            "archive" => (
                "Archive",
                "Shrink old tool results in the live zone; pointers expand on demand.",
                true,
            ),
            "proxy" => (
                "Proxy usage",
                "Record provider usage; compress mode runs plugin proxy_filter.",
                true,
            ),
            "inject" => (
                "Inject",
                "Budgeted SessionStart / prompt context that stays byte-stable.",
                true,
            ),
            "guard" => (
                "Guard",
                "Deny duplicate reads and commands inside a sliding window.",
                true,
            ),
            "memory" => (
                "Memory",
                "Recall notes and titles without an LLM extraction step.",
                true,
            ),
            "graph" => (
                "Graph",
                "symbol / callers / impact from a tree-sitter-tags index.",
                true,
            ),
            "toon" => (
                "TOON",
                "Compact tabular JSON. Off by default until it beats the corpus.",
                true,
            ),
            _ => ("Plugin", "External plugin.", false),
        };
        Self {
            title: if title == "Plugin" {
                id.into()
            } else {
                title.into()
            },
            summary: summary.into(),
            saves_tokens,
            fields: vec![],
        }
    }
}

/// An MCP tool exposed by `rtok mcp`.
///
/// Every listed tool costs tokens in every request the host sends, so the description is one
/// short sentence — the host tests it against a token cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDef {
    /// Tool name as the host sees it.
    pub name: &'static str,
    /// One short sentence; every listed tool costs tokens in every request.
    pub description: &'static str,
    /// JSON Schema for the tool arguments.
    pub input_schema: Value,
}

// Event views: borrowed slices of a hook input, built by the host.

/// A tool call about to run. A plugin may deny or rewrite it.
#[derive(Debug)]
pub struct PreToolUse<'a> {
    /// Host tool name, e.g. `Bash`, `Read`.
    pub tool_name: &'a str,
    /// Tool arguments as the host sent them.
    pub tool_input: &'a Value,
}

/// A tool call that has already run. Its result cannot be changed — a plugin may only add
/// context beside it (decision D2).
#[derive(Debug)]
pub struct PostToolUse<'a> {
    /// Host tool name.
    pub tool_name: &'a str,
    /// Tool arguments as the host sent them.
    pub tool_input: &'a Value,
    /// What the tool returned.
    pub tool_response: &'a Value,
}

/// A session starting or resuming.
#[derive(Debug)]
pub struct SessionStart<'a> {
    /// `startup` | `resume` | `clear` | `compact`
    pub source: &'a str,
}

/// A user prompt about to be sent.
#[derive(Debug)]
pub struct PromptSubmit<'a> {
    /// The prompt text.
    pub prompt: &'a str,
}

/// A compaction about to happen; the last chance to persist state.
#[derive(Debug)]
pub struct PreCompact<'a> {
    /// `manual` | `auto`
    pub trigger: &'a str,
    /// Path to the transcript the host is about to compact.
    pub transcript_path: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_names_are_the_config_keys() {
        assert_eq!(Surface::Hook.as_str(), "hook");
        assert_eq!(Surface::Cli.as_str(), "cli");
    }

    #[test]
    fn unknown_id_gets_a_generic_page_titled_after_itself() {
        let p = DashboardPage::from_id("ext");
        assert_eq!(p.title, "ext");
        assert!(!p.saves_tokens);
    }
}
