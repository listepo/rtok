# Cloud MCP mode for the Grok API

Research and design note (2026-09-26). Status: proposal, nothing built yet. xAI facts were
checked against docs.x.ai on 2026-09-26; rtok paths were checked against `origin/main`
(8f318517).

## 1. Summary

rtok has two ways to work today: `rtok hook <event>`, and `rtok mcp` over stdio. The Grok API
(`api.x.ai`) can use neither. It has no hooks, and it only talks to remote MCP servers over
Streamable HTTP or SSE, which it calls from xAI's own servers. This note proposes a second
launch mode, the "cloud" mode, with two paths:

- **Remote MCP.** rtok serves its existing tools over HTTPS (Streamable HTTP, SSE optional).
  xAI calls them server-side. rtok can shape what it returns, but the client never sees the
  outputs.
- **Client-side functions.** rtok exports the same tools as ordinary `function` tools. The
  client runs the `function_call` → `function_call_output` loop, and rtok executes each
  call. On this path rtok controls every tool result that enters the context.

The current local mode (stdio) stays the default and does not change.

## 2. Grok API findings

Remote MCP
- Tools entry `{"type": "mcp", "server_url", "server_label", "server_description",
  "allowed_tools", "authorization", "headers"}`. The native SDK spells the last ones
  `allowed_tool_names` and `extra_headers`. "Only Streaming HTTP and SSE transports are
  supported." `authorization` is "a token that will be set in the Authorization header".
  https://docs.x.ai/developers/tools/remote-mcp
- xAI connects to the server itself ("xAI manages the MCP server connection"), so the URL
  must be reachable from the public internet. A server on a laptop needs a tunnel. Stdio MCP
  servers work only in the Grok Build CLI (`/mcps`), not in the API.
  https://docs.x.ai/build/features/skills-plugins-marketplaces
- Server-side outputs are invisible to the client: "Only the tool call invocations are
  shown — server-side tool call outputs are not returned in the API response." MCP calls
  show up in Responses output as `mcp_call`, and client tools as `function_call`.
  https://docs.x.ai/developers/tools/tool-usage-details

Token spend
- "When you configure a Remote MCP Tool without specifying `allowed_tools`, all tool
  definitions exposed by the MCP server are automatically injected into the model's
  context." https://docs.x.ai/developers/tools/remote-mcp
- `prompt_tokens` "represents the cumulative input tokens across all inference requests
  made during the agentic process. Each request includes the full conversation history up
  to that point." So a tool result is paid for again at every later step.
  https://docs.x.ai/developers/tools/tool-usage-details
- Prompt caching is automatic. "The `x-grok-conv-id` HTTP header routes requests with the
  same conversation ID to the same server." The Responses API has the body field
  `prompt_cache_key` for the same purpose.
  https://docs.x.ai/developers/advanced-api-usage/prompt-caching/maximizing-cache-hits
- Billing: "For Remote MCP tools, you will not be charged for the tool invocation but will
  be charged for any tokens used." https://docs.x.ai/developers/pricing

Hooks
- The Grok API has no hooks. The only point where a client can intercept a tool result is
  client-side function calling: "The model requests the call, you execute it locally, and
  return the result." https://docs.x.ai/developers/tools/function-calling
- Hooks exist only in Grok Build (the CLI). They live in `~/.grok/hooks/*.json` (project
  hooks in `.grok/hooks/*.json`, which need `/hooks-trust`). Events: `SessionStart`,
  `SessionEnd`, `UserPromptSubmit`, `PreToolUse` ("the only blocking event"),
  `PostToolUse`, `PostToolUseFailure`, `PermissionDenied`, `Stop`, `StopFailure`,
  `Notification`, `SubagentStart`, `SubagentStop`, `PreCompact`, `PostCompact`.
  https://docs.x.ai/build/features/hooks

## 3. Key files

- `src/main.rs`, `src/cli.rs`: the `Cmd::Mcp { call, json, wrap }` subcommand, which
  dispatches to `crate::mcp::run`, `crate::mcp::call` or `crate::mcp::wrap::run`.
- `src/mcp.rs`: the rtok MCP server (`Server::new`, `handle_line`, `handle_value`,
  `call_tool`, `invoke`, `cap_result`, `record`).
- `src/mcp/wrap.rs`: lossless stdio wrapper around a foreign MCP server
  (`rtok mcp -- <argv>`). It cuts long results with `src/plugins/cmd/rules.rs`.
- `src/plugin.rs` (`ToolDef`). Tool providers:
  - `src/plugins/read/mod.rs`
  - `src/plugins/memory/mod.rs`
  - `src/plugins/graph/mod.rs`
  - `src/plugins/wasm.rs`
- `src/config/mod.rs`, `src/config/layers.rs` (`proxy_flags`), `config/default.toml`,
  `docs/config.md`: the `[mcp]` and `[proxy]` sections.
- `src/agents/mod.rs`, `src/agents/grok/mod.rs`, `plugins/grok/.mcp.json`: how hosts
  register `rtok mcp` (for example `[mcp_servers.rtok]` in `~/.grok/config.toml`).
- `src/proxy/mod.rs`, `src/proxy/openai_responses.rs`, `src/proxy/tools_rewrite.rs`:
  existing axum/reqwest HTTP code and the Responses wire. `function_call_output` by
  `call_id` is already parsed there.
- `src/demon.rs`: the `mcp` service for `rtok demon`.

## 4. Current MCP architecture (stdio only)

- Transport: newline-delimited JSON-RPC on stdin/stdout. `run` reads lines and passes each
  one to `Server::handle_line`, then `handle_value`. rmcp 3.2 supplies the types only
  (default features, no transport).
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`, plus the one server-to-client
  request `roots/list`.
- Tool list: `expand` (always listed), plus the `mcp_tools()` of every enabled built-in
  plugin:
  - `read`, `search`, `tree`
  - `mem_save`, `mem_search`, `mem_get`, `mem_update`, `handoff`
  - `symbol`, `callers`, `impact`, `outline`, `explore`

  `[mcp] tools` narrows the list.
- Dispatch: `call_tool` checks the allow-list and the schema's `required` fields, then
  `invoke` matches on the tool name. `record` writes the call and the before/after token
  estimates to SQLite. Results over `mcp.max_result_chars` are cut by `cap_result` and
  point at `expand(<id>)`.
- `rtok mcp --call <name> --json <args>` runs one call without MCP. It goes through the
  same `invoke_text` as `tools/call`.

## 5. Minimal changes

1. **Separate the stdio loop from the core.** Keep `Server` and `handle_value`
   transport-free. `run` keeps only the stdin reader for `transport = "stdio"`.
2. **HTTP transport.** Add `src/mcp/http.rs` on the axum already in the tree. `POST /mcp`
   is Streamable HTTP: a JSON body in, then a JSON or `text/event-stream` reply. SSE on
   `GET /mcp` is optional. Every message goes to the same `handle_value`. The alternative
   is rmcp's `transport-streamable-http-server` feature. Adding it needs a one-line reason
   in `Cargo.toml` and rows in `toolchain.md` and the shared `rust.md`.
3. **Client-side functions mode.** `rtok mcp --functions` prints the tool list as xAI
   Responses `{"type": "function", "name", "description", "parameters"}` entries, taken
   from `ToolDef.input_schema`. The client loop:
   - the model returns `function_call`;
   - the client runs `rtok mcp --call` (or `POST /call` on the HTTP server);
   - the client sends `{"type": "function_call_output", "call_id", "output"}` with
     `previous_response_id`.

   Later, `rtok proxy` with an `api.x.ai` upstream could run rtok's own `function_call`s
   itself, reusing `src/proxy/openai_responses.rs`.
4. **Config switch.** In `[mcp]` (`src/config/mod.rs`, `config/default.toml`):
   - `transport = "stdio" | "http"` (default `"stdio"`);
   - `bind = "127.0.0.1"`;
   - `port`, defaulting to a named const `DEFAULT_MCP_PORT`;
   - `token = ""`;
   - `public_url = ""`.

   Add flags `rtok mcp --transport http --port N` through a `mcp_flags` beside
   `proxy_flags` in `src/config/layers.rs`. `RTOK_MCP_*` env vars come from figment.
5. **Docs and tests.**
   - A user page `docs/grok.md` (setup, tunnel, the xAI `tools` entry), plus `docs/config.md`.
   - `tests/mcp_http.rs`: `initialize`, `tools/list` and `tools/call` over HTTP, and a 401
     without the token.

## 6. Token optimization notes (cloud MCP)

- Narrow the surface. Set `allowed_tools` in the xAI request and `[mcp] tools` in rtok,
  because every listed definition is injected into the context.
- Keep results compact. A result is re-counted as prompt tokens at every later step. Reuse
  the compact outputs rtok already has: `read` modes and dedup, `cap_result` with
  `expand(<id>)` for losslessness, and the cmd rules.
- Keep the prefix stable. Tool definitions and descriptions should be byte-stable across
  requests, and the client should send `x-grok-conv-id` (or `prompt_cache_key`) so prompt
  caching hits.
- MCP invocations are free and tokens are billed. Fewer, smaller results are the lever,
  not fewer calls.
- Remote-MCP outputs are invisible to the client. rtok can only compress what it returns
  as the server. Anything beyond that (history trimming, dropping stale results) needs the
  client-side functions path.

## 7. Security notes

- Require `Authorization: Bearer <token>` on every HTTP request. Take the token from
  `[mcp] token` or `RTOK_MCP_TOKEN` and compare it in constant time. Put the same token
  into the xAI `authorization` field.
- Bind to `127.0.0.1` and expose the server only through a tunnel, for example
  `cloudflared tunnel --url http://127.0.0.1:<port>`. For local container tests, use Colima
  (`docs/colima.md`), not Docker Desktop.
- `read`, `search` and `tree` expose the filesystem of the machine that runs rtok to
  requests from the internet. Keep the tool list narrow and the roots tight.

## 8. Limitations

- The Grok API has no hooks. `rtok hook` works only in Grok Build.
- Remote MCP supports only Streamable HTTP and SSE, and needs a public HTTPS URL.
- Remote-MCP tool outputs are never returned to the client.
- `src/mcp/wrap.rs` (foreign servers) stays stdio-only.
