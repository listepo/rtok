//! The plugin contract (plan §1, T0.4). Every token-reduction method implements [`Plugin`].
//!
//! Rules (AGENTS.md): fail open, lossless by default, and a saving that is not a
//! [`Measurement`] row does not exist. Default method bodies do nothing, so a plugin
//! implements only the surfaces it declares in its [`Manifest`].

use anyhow::Result;
use serde_json::Value;

use crate::config::{Config, PluginCfg};
use crate::store::Store;
use crate::tokens::{self, Class};

/// Native = implemented in Rust here; Adapter = drives an installed tool (decision D6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Native,
    Adapter,
}

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
    pub fn as_str(self) -> &'static str {
        match self {
            Surface::Hook => "hook",
            Surface::Mcp => "mcp",
            Surface::Proxy => "proxy",
            Surface::Cli => "cli",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub id: &'static str,
    pub kind: Kind,
    pub surfaces: &'static [Surface],
    pub default_on: bool,
}

/// Everything a plugin may touch: config, the store, and the session id.
/// The archive store is added in T3.1.
pub struct Ctx {
    pub config: Config,
    pub store: Store,
    pub session: String,
}

impl Ctx {
    /// Open the store at `config.core.db_path`.
    pub fn open(config: Config, session: impl Into<String>) -> Result<Self> {
        let store = Store::open(&config.core.db_path)?;
        Ok(Self {
            config,
            store,
            session: session.into(),
        })
    }

    /// Default config + in-memory store, for tests and examples.
    pub fn in_memory(session: impl Into<String>) -> Result<Self> {
        Ok(Self {
            config: Config::default(),
            store: Store::open_in_memory()?,
            session: session.into(),
        })
    }

    pub fn estimate(&self, text: &str, class: Class) -> u32 {
        tokens::estimate(text, class, &self.config.estimator)
    }

    /// `[plugins.<id>]` table, if the user wrote one.
    pub fn plugin_cfg(&self, id: &str) -> Option<&PluginCfg> {
        self.config.plugins.get(id)
    }

    /// Persist a measurement for this session (the only path for savings into the DB).
    pub fn record(&self, m: &Measurement) -> Result<()> {
        self.store.insert_measurement(&self.session, m)
    }
}

/// A before/after pair produced by one plugin action (decision D3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measurement {
    pub plugin: &'static str,
    /// Plugin-defined subtype, e.g. `rtk`, `rule`, `raw`, `inject`, `dedup`.
    pub kind: &'static str,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub est_before: u32,
    pub est_after: u32,
    /// Archive id (or other handle) that makes the saving reversible.
    pub ref_id: Option<String>,
}

/// What a plugin may do to a PreToolUse event. First `Deny` wins; `Rewrite` is last-writer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreToolDecision {
    Deny { reason: String },
    Rewrite { input: Value, reason: String },
}

/// Text offered to the `inject` plugin; emitted in priority order until the budget (D5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Injection {
    pub plugin: &'static str,
    pub text: String,
    /// Higher first.
    pub priority: u8,
}

/// An MCP tool exposed by `rtok mcp`. Description ≤ 60 tokens (T4.1 test).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// `POST /v1/messages` body. Typed in T5.1; a raw JSON value until then.
pub type MessagesRequest = Value;

// Event views: borrowed slices of a hook input (built by `hooks::types::HookInput`).

pub struct PreToolUse<'a> {
    pub tool_name: &'a str,
    pub tool_input: &'a Value,
}

pub struct PostToolUse<'a> {
    pub tool_name: &'a str,
    pub tool_input: &'a Value,
    pub tool_response: &'a Value,
}

pub struct SessionStart<'a> {
    /// `startup` | `resume` | `clear` | `compact`
    pub source: &'a str,
}

pub struct PromptSubmit<'a> {
    pub prompt: &'a str,
}

pub struct PreCompact<'a> {
    /// `manual` | `auto`
    pub trigger: &'a str,
    pub transcript_path: &'a str,
}

pub trait Plugin: Send + Sync {
    fn manifest(&self) -> Manifest;

    /// May deny or rewrite the tool call. `None` = no opinion.
    fn pre_tool(&self, _ev: &PreToolUse, _cx: &Ctx) -> Option<PreToolDecision> {
        None
    }

    /// May only add `additionalContext`; tool results cannot be changed here.
    fn post_tool(&self, _ev: &PostToolUse, _cx: &Ctx) -> Option<String> {
        None
    }

    fn session_start(&self, _ev: &SessionStart, _cx: &Ctx) -> Option<Injection> {
        None
    }

    fn prompt_submit(&self, _ev: &PromptSubmit, _cx: &Ctx) -> Option<Injection> {
        None
    }

    fn pre_compact(&self, _ev: &PreCompact, _cx: &Ctx) {}

    fn mcp_tools(&self) -> Vec<ToolDef> {
        Vec::new()
    }

    /// Rewrite an outgoing API request in place; return one measurement per change.
    fn proxy_filter(&self, _req: &mut MessagesRequest, _cx: &Ctx) -> Vec<Measurement> {
        Vec::new()
    }
}
