//! rtok's own MCP servers — the one list [`McpSpec::entry`](crate::spec::McpSpec::entry) builds
//! from, so a host installer never hand-assembles a `command`/`args` pair again (T277: that
//! duplication is what let host modules drift from each other in the first place).

/// A stdio MCP server rtok can serve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Server {
    pub name: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    /// Extra environment variables the entry sets, if any (most hosts' entries carry none).
    pub env: &'static [(&'static str, &'static str)],
}

/// rtok's own MCP server: `rtok mcp` on `PATH`. The only entry any [`crate::spec::McpSpec`]
/// points at today; a graph or plugin-served server (roadmap) joins this list, not a second one.
pub const RTOK: Server = Server {
    name: "rtok",
    command: "rtok",
    args: &["mcp"],
    env: &[],
};
