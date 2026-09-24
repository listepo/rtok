# Batch, Flex, and routing

rtok is a middleware surface (hooks, MCP, API proxy) — not an agent and not a
model router by default. **Batch**, **Flex**, and **model routing** are three
different cost levers. This page records what the proxy does today, what is
**planned**, and how each interacts with prompt caching.

Source of intent: `next.md` (token-saving notes), `docs/prompt-cache.md`,
`research.md` (sticky routing / D9). Behaviour that is not wired in
`src/proxy/` is marked **planned**.

## Three different things

| Lever | What it is | Sync agent turn? | Rewrites the body? |
|-------|------------|------------------|--------------------|
| **Batch** | Provider async APIs (`/v1/batches`, `/v1/messages/batches`, …) | No — submit, poll, fetch results | **Pass-through** (no sync→Batch conversion) |
| **Flex** | Same sync endpoints, cheaper/slower tier via `service_tier` | Yes | **Rewrite** in `prepare` (**planned**) |
| **Model routing** | Pick a cheaper model for easy turns (D9) | Yes | **Rewrite** `model` (**planned**) |

rtok does **not** transparently convert a live agent `POST /v1/messages` (or
`/v1/chat/completions`) into a Batch job. Agents need the sync reply on the
same HTTP request; Batch is a separate client workflow that happens to share
the proxy hop.

MCP tools and Claude Code hooks never see LLM HTTP traffic — only the proxy
does. Batch/Flex/routing therefore live entirely under `rtok proxy`
(`src/proxy/`), not under `src/hooks/` or `src/mcp.rs`.

## Pass-through vs rewrite

The proxy pipeline for a non-plain request (`src/proxy/mod.rs` `shape_request`):

1. **record** — `calls` / `call_io` rows (fail-open)
2. **compress** — only when `proxy.mode = "compress"`; runs `Plugin::proxy_filter`
3. **tools_rewrite** — opt-in `[proxy.tools_rewrite]`
4. **prepare** — wire-specific request shaping (today: OpenAI
   `stream_options.include_usage`; Anthropic context-management beta when armed)
5. forward upstream; tee the response for usage parsing when a `Wire` matches

Unknown paths still hit the axum **fallback** and are forwarded byte-identical
to the provider upstream. `Wire::matches` is exact for sync chat shapes
(`/v1/messages`, `/v1/chat/completions`, `/v1/responses`, Gemini
`:generateContent`) — Batch paths do **not** match a wire today, so they skip
compress / prepare / usage parsing and behave as a plain reverse hop with a
`calls` row when bookkeeping is on.

| Path class | Today | Intended |
|------------|-------|----------|
| Sync chat wires | record + optional compress/prepare + usage | unchanged |
| Batch endpoints | fallback forward; no `Wire`; no usage row | **planned**: observe batch lifecycle + result usage |
| Flex (`service_tier`) | client may set it; rtok does not inject | **planned**: `prepare` may set/override per `[proxy.flex]` |
| Model routing | single upstream URL; model left as client sent | **planned**: policy rewrite under `[proxy.routing]` |

## Batch API endpoints

Provider Batch is a **client-initiated** async API. Point the same base URL at
`rtok proxy`; the fallback forwards the path.

### OpenAI

| Method | Path | Role |
|--------|------|------|
| `POST` | `/v1/batches` | create a batch from an uploaded JSONL file |
| `GET` | `/v1/batches/{id}` | poll status |
| `GET` | `/v1/batches` | list |
| `POST` | `/v1/batches/{id}/cancel` | cancel |
| (files) | `/v1/files` | upload input / download output (also fallback-forwarded) |

Supported batch member endpoints (provider docs): `/v1/chat/completions`,
`/v1/responses`, `/v1/embeddings`, and others. rtok does not rewrite the JSONL
lines inside a batch file.

### Anthropic

| Method | Path | Role |
|--------|------|------|
| `POST` | `/v1/messages/batches` | create a Message Batch |
| `GET` | `/v1/messages/batches/{id}` | poll |
| `GET` | `/v1/messages/batches` | list |
| `GET` | `/v1/messages/batches/{id}/results` | stream/download results |
| `DELETE` | `/v1/messages/batches/{id}` | delete (after cancel if in progress) |

Exact match on `/v1/messages` means `/v1/messages/batches…` never enters the
Anthropic `Wire` adapter — no `proxy_filter`, no context-management injection.

### What Batch is **not**

- Not a substitute for Flex on a live agent turn.
- Not automatic: rtok will not enqueue sync traffic into Batch (**planned**
  policy remains “never”, unless an explicit opt-in tool/CLI is designed later).
- Not visible to hooks/MCP.

### Planned CLI skeleton (opt-in, not implemented)

A thin **opt-in** surface for humans/scripts that already speak provider Batch —
outside the agent loop. It does **not** replace an agent's `ANTHROPIC_BASE_URL` /
`OPENAI_BASE_URL` (those stay on sync chat wires). Commands talk to the same
`rtok proxy` hop and **pass through** provider Batch paths (`/v1/batches`,
`/v1/messages/batches`, …); no sync→Batch conversion.

**Planned** (not in the binary yet):

| Command | Role |
|---------|------|
| `rtok batch submit <jsonl>` | upload/create a batch (JSONL → provider create) |
| `rtok batch status <id>` | poll batch status |
| `rtok batch fetch <id> <out>` | download results to `<out>` |

See also `next.md` (token-saving backlog). Promote into `plan.md` before coding.

## Flex API (sync, cheaper tier)

OpenAI Flex processing is the same Chat Completions / Responses HTTP surface
with `"service_tier": "flex"` (provider docs). Latency is higher; price is
lower; prompt caching can still apply on that tier when the provider supports
it.

**Today:** if the client already sends `service_tier`, the proxy forwards it
unchanged (passthrough / compress do not strip unknown fields on the wire
parse path that re-serialises only after a filter mutates the body).

**Planned:** `[proxy.flex]` drives `prepare` on OpenAI wires:

- `enabled = true` → set `service_tier = "flex"` when the client omitted it
- `force = true` → overwrite a client value
- respect an explicit client `"default"` / `"auto"` when `force = false`
- on `429` resource-unavailable from Flex, fail open to the client (no silent
  retry onto default tier unless `[proxy.flex] fallback = "default"` — **TODO**:
  confirm retry policy before implementing)

Anthropic has no `service_tier` twin; Flex here means OpenAI (and any future
provider that exposes an equivalent field on a `Wire`).

## Cache interaction

See also [prompt-cache.md](prompt-cache.md).

| Mechanism | Batch | Flex | Notes |
|-----------|-------|------|-------|
| Provider **prompt cache** (prefix-exact) | Provider-dependent; OpenAI documents Flex + cache stacking; Batch JSONL lines are separate requests | Same prefix rules as sync; sticky upstream still matters | rtok must not churn `system` / `tools` / live turns — already true for sync compress |
| rtok **archive** live-zone rewrite | N/A on Batch create (no Messages `Wire`) | Same as sync when Flex rides Chat/Responses | Batch member bodies inside a file are not rewritten |
| **Semantic** response cache (P31) | Must stay off for Batch create/poll (**planned** guard) | Same eligibility rules as sync | A hit is still a wrong answer for agents |

Sticky routing (pin the same upstream pod/region for a session) is about
**prompt-cache hits**, not Batch vs Flex. Batch jobs are async and usually
cannot share a live session KV with the interactive agent; do not expect
Batch discount **and** the interactive session's cache-read rate on the same
bytes.

## Measurement / metrics

**Today (sync wires):** each proxied request can write `calls`, `call_io`,
`tokens`, and `usage` (four counters). `rtok stats` / `rtok stats --price`
read those rows. Paths without a `Wire` (Batch, `/v1/models`, …) record the
call when bookkeeping is on but skip usage parsing (`src/proxy/mod.rs`).

**Planned for Batch:**

- tag `calls.kind` / metadata so Batch create/poll/results are distinguishable
  from sync `api_request`
- when results are fetched, parse per-line usage into `usage` rows (or a
  dedicated rollup) so `--price` can show Batch discounts
- surface Batch vs sync vs Flex in `rtok report` / stats breakdowns

**Planned for Flex:**

- record the effective `service_tier` on the `calls` row
- price Flex tokens with the Flex rate rows once `[stats.prices]` gains them
  (**TODO**: dated Flex USD/MTok rows)

Until those land, treat Batch/Flex savings as provider-console numbers, not
rtok ledger numbers.

## Configuration

Keys below are documented for the design; they are **not** loaded by the
binary yet (unknown keys fail `rtok config validate`). See [config.md](config.md).

```toml
# Planned — not parsed today
[proxy.batch]
enabled = true          # forward Batch paths (always true via fallback today)
observe = true          # record create/poll/results into the ledger
parse_results = false   # when true, expand result files into usage rows

[proxy.flex]
enabled = false         # prepare may set service_tier = "flex"
force = false           # overwrite client service_tier
fallback = "none"       # none | default — behaviour on Flex 429 (**TODO**)

[proxy.routing]
enabled = false         # model / tier routing (D9); off until a policy + Check exists
sticky = true           # prefer one upstream for prompt-cache affinity (I-84)
default_model = ""      # empty = leave client model
# policy table / classifier: not specified yet (**TODO**)
```

## Related

- [prompt-cache.md](prompt-cache.md) — why rewrites stay outside the live edge
- [config.md](config.md) — `[proxy.*]` reference
- [comparison.md](comparison.md) — vs bifrost / Portkey / LiteLLM gateways
- `architecture.md` §12 — where Batch/Flex sit in the proxy pipeline
- [model-routing.md](model-routing.md) — D9 / §16.3 #3 (separate planned line from Batch/Flex)
- `next.md` — Batch/Flex and model routing on the token-saving backlog
