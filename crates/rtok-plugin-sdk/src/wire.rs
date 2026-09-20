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

/// One shrinkable non-result payload in the live zone: a nested JSON dump or a
/// `data:` blob inside a user content block (plan T51.1). Unlike [`ToolResultRef`]
/// there is no provider-stable id, so a plugin keys its decision by content hash —
/// which is also what keeps the rewrite byte-stable across turns.
pub struct BlobRef<'a> {
    /// The mutable string payload. Rewrite it in place.
    pub content: &'a mut Value,
    /// How many user turns follow this block.
    pub turn: usize,
}

/// Injected skill body in a user message (T61.2).
pub struct SkillRef<'a> {
    /// Preceding `Skill` tool_use id.
    pub id: String,
    /// Last path component of the skill directory.
    pub name: String,
    /// Mutable skill body text block.
    pub content: &'a mut Value,
    /// User turns since this block was injected.
    pub turn: usize,
}

/// The one thing [`WireRequest`] needs from a provider dialect. The host implements it;
/// a plugin never names it.
pub trait ToolResults: Send + Sync {
    /// Every mutable tool-result payload in `req`, with its id and turn distance.
    fn tool_results<'a>(&self, req: &'a mut Value) -> Vec<ToolResultRef<'a>>;

    /// Skill bodies the archive plugin may shrink (T61.2). Empty by default.
    fn skills<'a>(&self, _req: &'a mut Value) -> Vec<SkillRef<'a>> {
        Vec::new()
    }

    /// Large non-result payloads a proxy pass may shrink losslessly (T51.1).
    /// Empty by default; wires with user content blocks override it.
    fn live_blobs<'a>(&self, _req: &'a mut Value) -> Vec<BlobRef<'a>> {
        Vec::new()
    }

    /// Injected skill bodies in `req` (T61.2). Empty by default.
    fn skill_refs<'a>(&self, _req: &'a mut Value) -> Vec<SkillRef<'a>> {
        Vec::new()
    }
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

    /// Mutable skill bodies in the request.
    pub fn skills(&mut self) -> Vec<SkillRef<'_>> {
        self.wire.skills(self.body)
    }

    /// Every shrinkable non-result payload in the request, mutable.
    pub fn live_blobs(&mut self) -> Vec<BlobRef<'_>> {
        self.wire.live_blobs(self.body)
    }

    /// Every injected skill body in the request, mutable.
    pub fn skill_refs(&mut self) -> Vec<SkillRef<'_>> {
        self.wire.skill_refs(self.body)
    }
}
