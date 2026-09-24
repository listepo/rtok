# Model routing (D9)

**Status: planned / Later** — not the default v0.1 path. Config stub and
docs only until a measurement Check exists. **Separate line of work from
Batch and Flex** (`docs/batch-flex.md`): those change *how* or *when* a
request is billed on the same (or async) surface; model routing changes
*which* `model` string the sync chat wires forward.

Source of intent: `plan.md` decision **D9**, `research.md` §16.3 option
**#3** (and sticky-upstream #1 / I-84), `next.md` token-saving backlog.
Behaviour that is not wired in `src/proxy/` is marked **planned**.

## What it is

D9 (*Agents are provider-agnostic*): route by job — low-cost models for
mechanical work, mid-tier for coding, high-performance for research only
after the user confirms. Host product names are not the implementer.

`research.md` §16.3 #3 (*Model / tier routing by job*):

- Cheap model for format / classify / expand-prep
- Mid for edit
- Expensive only after confirm
- Impact is mostly **$** (small raw-token change)
- Needs a **router policy + measurement** so claimed “savings” are dollars,
  not vibes

rtok today is a middleware surface (hooks, MCP, API proxy) — **not** a
model router by default. Agents (or the host) still pick the model; the
proxy leaves `model` as the client sent it.

## How it would work in rtok

Only under `rtok proxy` (`src/proxy/`), on **sync** chat wires
(`/v1/messages`, `/v1/chat/completions`, `/v1/responses`, Gemini
`:generateContent`). Hooks and MCP never see LLM HTTP bodies, so they
cannot implement D9.

**Today**

- Single upstream URL; `model` left as the client sent
- No classifier; no policy table
- Sticky *upstream pod/region* affinity (I-84 / §16.3 #1) is about
  **prompt-cache hits**, not picking a cheaper model — related config flag
  only

**Planned** (after a D9 Check)

1. Optional policy / classifier decides a target model (or tier) for this
   turn
2. `shape_request` → **prepare** may **rewrite** the JSON `model` field
   under `[proxy.routing]` when `enabled = true`
3. Measurement Check attributes $ before/after so the policy stays honest
4. Policy table / classifier shape: **not specified yet** (**TODO**)

No transparent sync→Batch conversion; routing does not enqueue Batch or
set Flex `service_tier`. Those stay under `[proxy.batch]` / `[proxy.flex]`.

## Configuration (planned — not loaded by the binary yet)

Unknown keys fail `rtok config validate` until the `Config` fields ship.
Same stub as in `docs/batch-flex.md`:

```toml
# Planned — not parsed today
[proxy.routing]
enabled = false         # model / tier routing (D9); off until a policy + Check exists
sticky = true           # prefer one upstream for prompt-cache affinity (I-84)
default_model = ""      # empty = leave client model
# policy table / classifier: not specified yet (**TODO**)
```

MVP for Batch/Flex may parse this as a **stub only** (`enabled = false`,
no rewrite). Full policy + classifier is a follow-on plan (see roadmap
S6 / T256), not part of Batch/Flex definition of done.

## Status

| Piece | Status |
|-------|--------|
| Decision D9 | recorded in `plan.md` |
| Research §16.3 #3 | Later / Decision-shaped option |
| `[proxy.routing]` keys documented | yes (this page + `docs/batch-flex.md`) |
| Config parse stub | planned (optional S6) |
| `model` rewrite in `prepare` | **planned / Later** — blocked on D9 Check |
| Classifier / policy table | **TODO** — separate plan |
| Sticky upstream (I-84) | Decision-shaped; may land with prompt-cache work independently |

## Related

- [batch-flex.md](batch-flex.md) — Batch / Flex levers (separate from routing)
- [prompt-cache.md](prompt-cache.md) — why sticky upstream matters for $
- [config.md](config.md) — `[proxy.*]` reference (when fields land)
- `plan.md` — D9; Batch/Flex roadmap S6 / T256 stub
- `research.md` §16.3 — options #1 (sticky) and #3 (model / tier routing)
- `next.md` — token-saving backlog card
- `ideas.md` I-84 — sticky upstream + prompt-cache affinity
