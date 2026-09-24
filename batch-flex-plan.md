# Batch / Flex API pass — implementation plan

Implementation plan for the cost levers documented in [`docs/batch-flex.md`](docs/batch-flex.md)
and [`architecture.md`](architecture.md) §12. Design-first branch:
`docs/batch-flex-pass`. Code lives under `src/proxy/` only (hooks/MCP never see
LLM HTTP).

Status: **plan** (no implementation in this document). Ground truth for
“today” is `origin/main` as of the design pass (`bcb03778` docs commit on this
branch; proxy behaviour matches current `src/proxy/`).

## Goals

1. **Batch observe** — when clients already use OpenAI `/v1/batches*` or
   Anthropic `/v1/messages/batches*` through `rtok proxy`, rtok records those
   hops in the ledger and can attribute usage/price once results are fetched.
2. **Flex inject** — optional `[proxy.flex]` so OpenAI sync Chat/Responses
   requests get `service_tier = "flex"` via `prepare` without the agent knowing
   about Flex.
3. **Keep boundaries** — never convert a live sync agent turn into a Batch job;
   never put Batch/Flex logic in hooks or MCP; do not break prompt-cache sticky
   behaviour documented in [`docs/prompt-cache.md`](docs/prompt-cache.md).
4. **Measurable savings** — `rtok stats` / `rtok report` can distinguish sync vs
   Flex vs Batch once observe + Flex pricing rows exist.

Non-goals for this feature (separate tracks):

- Full **model routing** (D9 / `[proxy.routing]` policy + classifier) — stub
  config only until a Check exists; called out as a later stage, not a Blocker
  for Batch/Flex MVP.
- Automatic sync→Batch enqueue, Batch JSONL rewriting, or a new MCP “submit
  batch” tool (optional follow-on, explicit opt-in only).
- Semantic response cache (P31) behaviour beyond a hard skip on Batch paths.

## Scope

| In scope | Out of scope |
|----------|--------------|
| Config parse for `[proxy.batch]` / `[proxy.flex]` (and stub `[proxy.routing]`) | Transparent sync→Batch |
| Path recognition for Batch create/poll/list/cancel/results + related `/v1/files` | Rewriting Batch JSONL member bodies |
| Ledger tags + optional result→`usage` parsing | Anthropic Flex twin (none exists) |
| OpenAI `prepare` Flex injection / force / documented 429 policy | Changing compress/`proxy_filter` semantics for Batch |
| Docs already on this branch stay authoritative; update when behaviour ships | Porting Flex/Batch into host plugins |

Primary modules (today):

- `src/proxy/mod.rs` — `handle` / `shape_request` / fallback / bookkeeping
- `src/proxy/wire.rs` + `openai_chat.rs` / `openai_responses.rs` / `anthropic.rs` —
  exact `Wire::matches` for sync shapes only
- Config load / validate (same path as other `[proxy.*]` tables)
- Store: `calls`, `call_io`, `usage`, `tokens`

## Current baseline (do not regress)

- Batch paths miss every `Wire::matches` → axum **fallback** byte-forward;
  optional `calls` row; **no** compress / prepare / usage parse.
- Client-set `service_tier` on OpenAI sync bodies already forwards.
- `prepare` today shapes OpenAI `stream_options.include_usage` (and related),
  not Flex.
- MCP / Claude hooks never see LLM HTTP.

## Stages

### S0 — Align plan + config surface (docs done on this branch)

**Done when:** `docs/batch-flex.md`, `docs/config.md` stubs, architecture §12,
proxy `AGENTS.md`, and this plan agree on semantics.

**Work:** keep this file updated if decisions change (especially Flex `429`
fallback and Batch `calls.kind` naming).

**Exit:** no code yet; `rtok config validate` still rejects uncommented
`[proxy.batch|flex|routing]` until S1.

### S1 — Config types + validate (no behaviour change)

**Work:**

- Add `ProxyBatch`, `ProxyFlex`, `ProxyRouting` structs with defaults matching
  [`docs/batch-flex.md`](docs/batch-flex.md) (batch observe defaulting carefully:
  `enabled` is effectively always-on via fallback today; prefer `observe` /
  `parse_results` as the real switches).
- Wire into `Config` deserialize + `rtok config validate`.
- Document that unknown-key failure goes away for these tables.

**Tests:** unit parse; golden TOML fixtures; validate accepts commented→live
examples from `docs/config.md`.

**Exit:** config loads; proxy behaviour **unchanged** with defaults.

### S2 — Batch path observe (create / poll / list / cancel)

**Work:**

- Detect Batch path classes (OpenAI + Anthropic tables in
  `docs/batch-flex.md`) without folding them into sync `Wire` adapters.
- Tag `calls` (kind/metadata) so Batch is distinguishable from sync
  `api_request`.
- Keep bodies pass-through; do **not** run compress/`proxy_filter` on Batch
  create payloads or JSONL file bytes.
- Guard: if semantic cache ever runs on the proxy hop, **skip** Batch paths.

**Tests:** proxy integration — POST/GET Batch paths record tagged rows; body
byte-identical; sync wires still match and parse usage.

**Exit:** `rtok stats` can filter or show Batch hop counts (even without
token totals yet).

### S3 — Batch results → usage (opt-in `parse_results`)

**Work:**

- On Anthropic results fetch / OpenAI output file download through the proxy,
  when `parse_results = true`, parse per-line usage into `usage` (or a rollup
  linked to the batch `calls` row).
- Fail-open on parse errors; never block the client download.
- Price rows: use existing provider rates; Batch discount factors as provider
  documents (document any hard-coded assumptions).

**Tests:** fixture JSONL / results streams → expected `usage` rows; disabled
flag → no parse; malformed line → fail-open.

**Exit:** `rtok stats --price` reflects Batch traffic when observe+parse are on.

### S4 — Flex inject via `prepare` (OpenAI wires)

**Work:**

- In OpenAI Chat + Responses `prepare`: if `[proxy.flex] enabled`, set
  `service_tier = "flex"` when omitted; if `force`, overwrite; if client sent
  `default`/`auto` and `force = false`, leave alone.
- Record effective `service_tier` on the `calls` row.
- **429 policy:** implement only after product decision:
  - `fallback = "none"` (default): surface error to client;
  - `fallback = "default"`: one retry without Flex (**confirm before coding** —
    still marked TODO in design docs).
- Add Flex USD/MTok rows under `[stats.prices]` when rates are chosen (dated).

**Tests:** prepare unit tests for omit/force/respect; integration with mock
upstream; 429 behaviour matrix once policy locked.

**Exit:** enabling `[proxy.flex]` changes request JSON only on OpenAI sync
wires; Anthropic unchanged; prompt-cache docs still accurate.

### S5 — Reporting + docs polish

**Work:**

- Breakdown in `rtok report` / stats: sync vs Flex vs Batch.
- Update `docs/batch-flex.md` “planned” → “today” for shipped pieces; keep
  routing as planned.
- Optional: pointer from `next.md` / plan cards when promoting to `plan.md`.

**Exit:** operator can prove savings from rtok ledger without provider console.

### S6 — Model routing stub only (optional track)

**Work:** parse `[proxy.routing]` (`enabled=false`, `sticky`, `default_model`);
no classifier. Sticky upstream flag may land with prompt-cache work (I-84)
independently.

**Exit:** config ready; no model rewrite until D9 Check exists (follow-on
plan, not part of Batch/Flex MVP definition of done).

## Suggested order / sizing

| Stage | Approx. effort | Risk |
|-------|----------------|------|
| S1 Config | 0.5–1 d | Low — validate regressions |
| S2 Batch observe | 1–2 d | Medium — path matching false positives (`/v1/messages` vs `/v1/messages/batches`) |
| S3 Results parse | 1–2 d | Medium — provider result shapes drift |
| S4 Flex prepare | 1–2 d | Medium — 429 fallback / cache interaction |
| S5 Report + docs | 0.5–1 d | Low |
| S6 Routing stub | 0.5 d | Low |

Batch observe (S2) and Flex (S4) can proceed in parallel after S1; S3 depends
on S2.

## Risks

| Risk | Mitigation |
|------|------------|
| Accidental sync→Batch or treating Batch as a Wire | Explicit non-goal; exact path tables; tests that `/v1/messages` still wires and `/v1/messages/batches` does not |
| Compress/`proxy_filter` mutates Batch JSONL | Never attach Batch paths to sync Wire; integration asserts byte identity |
| Flex 429 silent fallback surprises agents | Default `fallback = "none"`; document; require explicit config for retry |
| Prompt-cache miss from Flex/routing churn | Flex only sets `service_tier`; no system/tools rewrite; sticky stays separate |
| Ledger noise / double-count usage | Tag Batch distinctly; parse_results opt-in; fail-open |
| Config validate breaks existing TOMLs | Defaults off; ship behind this branch’s documented keys only |
| Scope creep into D9 routing | S6 stub only; classifier is a separate plan |

## Tests (summary)

- **Unit:** config parse; Batch path classifier; Flex `prepare` field matrix;
  result-line usage parser fixtures.
- **Integration (proxy):** sync wire regression (usage still parsed); Batch
  create/poll tagged + pass-through; Flex enabled request body; semantic-cache
  skip if that code path exists.
- **Manual / live (optional):** one OpenAI Flex call and one small Anthropic
  Message Batch through `rtok proxy` against a throwaway key — document in PR,
  not required for CI.

Reuse existing proxy test harness patterns under `src/proxy/` / `tests/` (same
style as current prepare / wire tests). Prefer named constants over magic
numbers per listepo Rust norms.

## Definition of done (MVP)

MVP = **S1 + S2 + S4 + S5** (S3 strongly recommended for “savings in rtok”;
S6 not required).

- [ ] `[proxy.batch]` / `[proxy.flex]` parse and validate
- [ ] Batch hops tagged in `calls`; bodies unchanged
- [ ] Flex inject/force on OpenAI Chat/Responses via `prepare`; effective tier
      recorded
- [ ] Flex `429` policy implemented as documented (no silent default unless
      configured)
- [ ] Stats/report can separate Flex vs sync (Batch counts at minimum; usage if
      S3 shipped)
- [ ] `docs/batch-flex.md` updated for shipped behaviour; no sync→Batch
- [ ] CI green for new unit/integration tests on Mac; no CloudAgent for listepo
      work

## Related

- [`docs/batch-flex.md`](docs/batch-flex.md) — semantics and endpoint tables
- [`docs/config.md`](docs/config.md) — planned TOML keys
- [`docs/prompt-cache.md`](docs/prompt-cache.md) — sticky vs Batch/Flex
- [`architecture.md`](architecture.md) §12 — pipeline placement
- [`src/plugins/proxy/AGENTS.md`](src/plugins/proxy/AGENTS.md) — agent notes
- `next.md` (PR #266) — token-saving backlog card for Batch/Flex/routing
