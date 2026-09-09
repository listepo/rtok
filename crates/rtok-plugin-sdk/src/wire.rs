//! What a request-rewriting plugin sees of a provider request.
//!
//! The proxy speaks three provider dialects and the host owns all three. A plugin sees only
//! what they have in common: the tool results in the request, normalised, and mutable.

use serde_json::Value;

/// One wire-normalised tool result.
pub struct ToolResultRef<'a> {
    /// Provider-stable result id — the key an archive decision is recorded under.
    pub id: String,
    /// The provider's mutable result payload. Rewrite it in place.
    pub content: &'a mut Value,
    /// How many user turns follow this result. The live tail is the expensive part of a
    /// transcript to touch, so a plugin that shortens history leaves low turns alone.
    pub turn: usize,
}

/// The one thing [`WireRequest`] needs from a provider dialect. The host implements it;
/// a plugin never names it.
pub trait ToolResults: Send + Sync {
    /// Every mutable tool-result payload in `req`, with its id and turn distance.
    fn tool_results<'a>(&self, req: &'a mut Value) -> Vec<ToolResultRef<'a>>;
}

/// A provider request, seen through the wire that owns it.
///
/// Deliberately narrow: a plugin can reach the tool results and nothing else, so a rewrite
/// cannot reorder messages, change the system prompt or touch the model choice.
pub struct WireRequest<'a> {
    wire: &'a dyn ToolResults,
    body: &'a mut Value,
}

impl<'a> WireRequest<'a> {
    /// Pair a parsed request body with the wire that understands it.
    pub fn new(wire: &'a dyn ToolResults, body: &'a mut Value) -> Self {
        Self { wire, body }
    }

    /// Every tool result in the request, mutable.
    pub fn tool_results(&mut self) -> Vec<ToolResultRef<'_>> {
        self.wire.tool_results(self.body)
    }
}
