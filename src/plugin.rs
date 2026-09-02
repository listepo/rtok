//! The plugin contract (plan §1, T0.4). Every token-reduction method implements [`Plugin`].
//!
//! Rules (AGENTS.md): fail open, lossless by default, and a saving that is not a
//! [`Measurement`] row does not exist. Default method bodies do nothing, so a plugin
//! implements only the surfaces it declares in its [`Manifest`].

use anyhow::Result;
use serde_json::Value;

use crate::config::Config;
use crate::proxy::wire::WireRequest;
use crate::store::Store;
use crate::tokens::{self, Class};

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

/// What a plugin declares about itself. Every plugin is native Rust written here (D6),
/// so there is nothing to distinguish beyond id, surfaces and default state.
#[derive(Clone, Debug)]
pub struct Manifest {
    /// Catalogue id, e.g. `cmd`. Also the Cargo feature name and the `[plugins.<id>]` key.
    pub id: &'static str,
    /// Surfaces this plugin answers on; anything else uses the trait's no-op defaults.
    pub surfaces: &'static [Surface],
    /// Enabled unless `[plugins.<id>] enabled` says otherwise.
    pub default_on: bool,
}

/// Everything a plugin may touch: config, the store, and the session id.
/// The archive store is added in T3.1.
pub struct Ctx {
    /// Merged configuration for this run.
    pub config: Config,
    /// The one SQLite file (D8).
    pub store: Store,
    /// Host session id; every measurement is attributed to it.
    pub session: String,
    /// The `calls` row this dispatch runs under (the API request in the proxy), when the
    /// surface has one. `record_call` / `record_plugin_run` nest their rows under it.
    pub call_id: Option<i32>,
}

impl Ctx {
    /// Open the store at `config.core.db_path`.
    pub fn open(config: Config, session: impl Into<String>) -> Result<Self> {
        let store = Store::open(&config.core.db_path)?;
        Ok(Self {
            config,
            store,
            session: session.into(),
            call_id: None,
        })
    }

    /// Default config + in-memory store, for tests and examples.
    pub fn in_memory(session: impl Into<String>) -> Result<Self> {
        Ok(Self {
            config: Config::default(),
            store: Store::open_in_memory()?,
            session: session.into(),
            call_id: None,
        })
    }

    /// Estimated token count for `text` (±15 %, no tokenizer, no network).
    pub fn estimate(&self, text: &str, class: Class) -> u32 {
        tokens::estimate(text, class, &self.config.estimator)
    }

    /// Persist a measurement for this session (the only path for savings into the DB).
    pub fn record(&self, m: &Measurement) -> Result<()> {
        self.store.insert_measurement(&self.session, m)
    }

    pub fn record_call(&self, surface: &str, kind: &str, name: Option<&str>) -> Result<i32> {
        self.insert_call(surface, kind, None, name)
    }

    /// A `plugin_run` row for `plugin`, nested under [`Ctx::call_id`] when set.
    pub fn record_plugin_run(&self, surface: &str, plugin: &str) -> Result<i32> {
        self.insert_call(surface, "plugin_run", Some(plugin), None)
    }

    fn insert_call(
        &self,
        surface: &str,
        kind: &str,
        plugin: Option<&str>,
        name: Option<&str>,
    ) -> Result<i32> {
        self.store
            .upsert_session(&self.session, None, None, None, None)?;
        let id =
            self.store
                .insert_call(&self.session, surface, kind, None, None, None, plugin, name)?;
        if let Some(parent) = self.call_id {
            self.store.set_call_parent(id, parent)?;
        }
        Ok(id)
    }

    pub fn record_tokens(
        &self,
        call_id: i32,
        plugin: Option<&str>,
        phase: &str,
        source: &str,
        tokens: i64,
    ) -> Result<()> {
        self.store
            .insert_tokens(call_id, plugin, phase, source, tokens)
    }

    /// Never returns `Err` to a plugin (fail open). On DB error, append to `log_file`.
    pub fn log(&self, level: &str, source: &str, name: &str, message: &str) {
        if let Err(e) = self.store.insert_log(
            level,
            source,
            name,
            message,
            Some(&self.session),
            None,
            None,
        ) {
            let path = &self.config.core.log_file;
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                use std::io::Write;
                let _ = writeln!(f, "{level} {source}/{name}: {message} ({e})");
            }
        }
    }
}

/// A before/after pair produced by one plugin action (decision D3).
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
    /// Owning `calls.id` when the plugin has one (T13.3).
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

/// Text offered to the `inject` plugin; emitted in priority order until the budget (D5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Injection {
    /// Catalogue id of the offering plugin.
    pub plugin: &'static str,
    /// The text itself; must be byte-stable across turns (D5).
    pub text: String,
    /// Higher first.
    pub priority: u8,
}

/// An MCP tool exposed by `rtok mcp`. Description ≤ 60 tokens (T4.1 test).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDef {
    /// Tool name as the host sees it.
    pub name: &'static str,
    /// One short sentence; every listed tool costs tokens in every request.
    pub description: &'static str,
    /// JSON Schema for the tool arguments.
    pub input_schema: Value,
}

// Event views: borrowed slices of a hook input (built by `hooks::types::HookInput`).

/// A tool call about to run.
pub struct PreToolUse<'a> {
    /// Host tool name, e.g. `Bash`, `Read`.
    pub tool_name: &'a str,
    /// Tool arguments as the host sent them.
    pub tool_input: &'a Value,
}

/// A tool call that has already run. Its result cannot be changed (D2).
pub struct PostToolUse<'a> {
    /// Host tool name.
    pub tool_name: &'a str,
    /// Tool arguments as the host sent them.
    pub tool_input: &'a Value,
    /// What the tool returned.
    pub tool_response: &'a Value,
}

/// A session starting or resuming.
pub struct SessionStart<'a> {
    /// `startup` | `resume` | `clear` | `compact`
    pub source: &'a str,
}

/// A user prompt about to be sent.
pub struct PromptSubmit<'a> {
    /// The prompt text.
    pub prompt: &'a str,
}

/// A compaction about to happen; the last chance to persist state.
pub struct PreCompact<'a> {
    /// `manual` | `auto`
    pub trigger: &'a str,
    pub transcript_path: &'a str,
}

/// One token-reduction method. Implement the surfaces your [`Manifest`] declares and leave
/// the rest to the no-op defaults. External crates implement this too and register through
/// [`Registry::from_plugins`](crate::plugins::Registry::from_plugins).
pub trait Plugin: Send + Sync {
    /// Id, surfaces and default state. Called on every dispatch; keep it cheap.
    fn manifest(&self) -> Manifest;

    /// May deny or rewrite the tool call. `None` = no opinion.
    fn pre_tool(&self, _ev: &PreToolUse, _cx: &Ctx) -> Option<PreToolDecision> {
        None
    }

    /// May only add `additionalContext`; tool results cannot be changed here.
    fn post_tool(&self, _ev: &PostToolUse, _cx: &Ctx) -> Option<String> {
        None
    }

    /// Text to offer at session start; the `inject` plugin decides what fits the budget.
    fn session_start(&self, _ev: &SessionStart, _cx: &Ctx) -> Option<Injection> {
        None
    }

    /// Text to offer with a user prompt; budgeted the same way as [`Plugin::session_start`].
    fn prompt_submit(&self, _ev: &PromptSubmit, _cx: &Ctx) -> Option<Injection> {
        None
    }

    /// Last chance to persist state before the transcript is compacted.
    fn pre_compact(&self, _ev: &PreCompact, _cx: &Ctx) {}

    /// Tools this plugin adds to `rtok mcp`.
    fn mcp_tools(&self) -> Vec<ToolDef> {
        Vec::new()
    }

    /// Rewrite the selected wire's normalised tool results; return one measurement per change.
    fn proxy_filter(&self, _req: &mut WireRequest<'_>, _cx: &Ctx) -> Vec<Measurement> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_survives_db_failure() {
        let cx = Ctx::in_memory("s").unwrap();
        cx.store.set_query_only().unwrap();
        cx.log("error", "plugin", "read", "boom");
    }
}
