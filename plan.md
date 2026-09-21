# rtok

https://github.com/listepo/rtok

Token-reduction CLI for AI coding agents: hooks, MCP server, API proxy; measured reductions, pluggable methods.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T79 | todo | P1 | 3 | 0% | |
| T86 | todo | P1 | 3 | 0% | |
| T91 | todo | P1 | 3 | 0% | |
| T92 | todo | P1 | 3 | 40% | |

### T79. `agents install zed` aborts on a real settings.json (JSONC)

Found by T78 on its first run, against this machine's own files. Zed writes **JSONC**: its `settings.json` carries `//` comments and trailing commas. `rtok_agent_sdk::read_json` is strict `serde_json::from_str`, so `edit_json` fails and `rtok agents install zed` exits with `trailing comma at line 44 column 3` and writes nothing. Every synthetic test passes because every synthetic config is strict JSON. VS Code's settings.json is JSONC by the same rule and shares the risk — this machine's happens to be strict JSON, so `vscode_keeps_the_real_settings_json` passes here and is not proof either way.

Reproduce: `rtok agents install zed` with a `~/.config/zed/settings.json` Zed itself wrote. The repro is kept as the `#[ignore]`d `zed_keeps_the_real_settings_json` in `tests/agents_real_config.rs` — un-ignore it as the check.

Plan (needs a decision first): a JSONC read alone is not enough — writing back through `serde_json` would silently delete the user's comments, which for Zed is most of the file. So either (a) edit the JSONC hosts in place with a comment-preserving editor, or (b) keep strict parsing and refuse with a message naming the file and the line the user can paste in themselves. Ask the creator which before starting; (b) is small and honest, (a) is the one users want and likely needs a dependency.

Check: un-ignore `zed_keeps_the_real_settings_json` in `tests/agents_real_config.rs` and it passes against a Zed-written `settings.json` — under (a) the comments and trailing commas are still in the file afterwards, under (b) the command exits non-zero with the message and the test asserts that instead. `just check` green either way.

### T86. `rtok agents install kimi` offers the plugin, and the plugin is the singleton

After T85. Precondition, by the creator on a live Kimi (agents cannot drive its TUI): `/plugins install <repo>/plugins/kimi`, `/reload`, then call the rtok MCP `tree` tool with no path — it must list the session project, not `plugins/managed/rtok/`. If it lists the plugin copy, Kimi starts plugin MCP servers in the plugin root and the singleton rule below must keep `mcp.json` and strip only the hooks. `support("plugin")` stops saying "no": install prints the exact `/plugins install <resolved plugins/kimi path>` line (dry-run and apply alike; rtok never writes `plugins/managed/` or `installed.json` — that format is Kimi's and undocumented). `installed()` reports `plugin` when `<kimi home>/plugins/managed/rtok/kimi.plugin.json` exists. D21 singleton: while the plugin is installed, setup strips rtok's own `[[hooks]]` tables and `mcpServers.rtok` from `config.toml` / `mcp.json` instead of adding them (Cursor's `plugin_is_mcp` rule, for hooks too), so no event fires twice and one `rtok mcp` serves the store. A `Desktop` variant (`Kimi Code.app`) joins `VARIANTS` with the same files. `src/agents/kimi/README.md` module table, `docs/agents.md` (bless), `research.md` §15 sentence listing Kimi among hosts without a plugin directory.

Check: unit tests — offer names `plugins/kimi` and `/plugins install`; with a seeded `managed/rtok/kimi.plugin.json` a second install removes the nine tables and `mcpServers.rtok` and reports `plugin`; remove leaves the managed copy alone and says how to remove it (`/plugins remove rtok`); `agents_doc` blessed; `just check`.

### T91. `rtok agents install antigravity` — CLI and desktop, the plugin is the only unit

After T90 (done): `plugins/antigravity/` carries `plugin.json` + `mcp_config.json` and **no hooks** — creator decisions 2026-09-21: the plugin is the only install path (no direct edit of `~/.gemini/config/mcp_config.json`), and per https://antigravity.google/docs/hooks/ `PreToolUse` cannot rewrite tool input and `PostToolUse` cannot add context. New host `antigravity` in `src/agents/antigravity/` (`mod.rs` + `README.md` with the module table and `## Docs`), registered in `HOSTS` and `host()`. Variants: CLI (`agy` on PATH) and Desktop (`Antigravity.app`), same files. `support`: `plugin` → `Flag("--yes")`; `mcp` → through the plugin only; `hooks` → `No` with the reason from T90; `proxy` → `No` (no documented base-URL override). `apply` is `HostPlugin::offer` — `plugins/antigravity` linked to `<plugins_path>/rtok` (default `~/.gemini/config/plugins/rtok`) — plus `skill::sync` of the hub skill into Antigravity's documented user skill root. Missing `rtok`: the offer names `ketch install listepo/rtok`.

Plan:
1. `src/agents/antigravity/mod.rs` on the `pi` pattern (one `HostPlugin`, `installed()` reads `PLUGIN.ours`), `[setup.antigravity] plugins_path` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md`; `skill::dest` / `label` arms for `antigravity`.
2. Unit tests: dry-run offer names `plugins/antigravity` and the ketch line and writes nothing; `--yes` links, a second apply is `NO_CHANGES`, remove takes back exactly ours; a foreign directory at the dest is left alone and not reported as installed.
3. `docs/agents.md` via `RTOK_BLESS=1` on `tests/agents_doc.rs`; `tests/trycmd/agents-list*.toml` re-blessed; `research.md` host sentence ("Antigravity on request" → listed).
Verify first: (a) whether Antigravity's plugin loader follows a symlinked plugin directory — the docs do not say; if it does not, the offer prints the exact `agy plugin install <resolved plugins/antigravity path>` line instead of linking (the Kimi rule from T86) and `installed()` reads `<plugins_path>/rtok/plugin.json`; (b) resolved 2026-09-21 from https://antigravity.google/docs/skills: the global skill root differs per surface — `~/.gemini/config/skills/<name>/` for Antigravity 2.0 and IDE, `~/.gemini/antigravity-cli/skills/<name>/` for the CLI — and the CLI also loads plugin-provided skills from `plugins/<name>/skills/`; so `skill::sync` needs two dests for this host (or one, if plugin-provided skills turn out to load on desktop too — then the hub skill is copied into the installed plugin instead, never duplicated in `plugins/antigravity/`). `agy` and the desktop app are not installed on this machine (`~/.gemini/config/` exists), so the creator runs both checks or installs `agy`.
Over the ≤200 LOC / ≤3 files limit as written — split into T91.1 (host + plugin offer + config) and T91.2 (skill root + docs bless) when claiming.

Check: the unit tests above; `rtok agents list` shows `antigravity`; `agents_doc`, `host_docs`, `config_coverage` green; `just check`.

### T92. `rtok agents install omp` — oh my pi: the shared pi extension plus native MCP

Creator request 2026-09-21: a host plugin for oh my pi CLI + desktop. oh my pi (https://github.com/can1357/oh-my-pi, binary `omp`) is a fork of pi with no desktop app — a TUI plus Zed ACP, which runs the same binary and config — so the host has one CLI variant. Its extension loader accepts `package.json` `omp.extensions` **or legacy `pi.extensions`**, treats symlinked directories as discovery targets, scans `~/.omp/agent/extensions` (not `~/.pi/agent/extensions`), and delivers the events `plugins/pi/extensions/rtok.ts` already subscribes to (`tool_call`, `tool_result`, `context`, `session_start`, `session_compact`); the extension imports nothing from upstream pi. So `plugins/pi` is reused as is — no `plugins/omp/` tree. Unlike pi, omp has native MCP (`~/.omp/agent/mcp.json`, `mcpServers.{command,args,env}`). Evidence: `docs/extension-loading.md`, `docs/extensions.md`, `docs/mcp-config.md` in that repo (fetched 2026-09-21).

Creator decision 2026-09-21: extension + native MCP. The extension owns the bash call path, context and compaction; tools come from `rtok mcp` registered in `mcp.json`; `registerTool` stays off under omp — one call path per capability (D21).

Plan:
1. `src/agents/omp/mod.rs` + `README.md` (`## Docs`: extensions, extension loading, hooks, MCP config, skills, marketplace): `HostPlugin { src_rel: "plugins/pi", host: "oh my pi", dest: <extensions_path>/rtok }` and `rtok_agent_sdk::register_mcp` on `mcp_path`; `support`: `plugin` → `Flag("--yes")`, `mcp` → yes, `hooks` → `No` (omp hooks are in-process TS modules; the extension owns that path), `proxy` → `No` (`models.yml` is not edited by setup, the pi rule). `[setup.omp] extensions_path = "~/.omp/agent/extensions"`, `mcp_path = "~/.omp/agent/mcp.json"` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md` (a named profile is a path override). Registered in `HOSTS` and `host()`.
2. `plugins/pi/README.md` gains the omp section and links; `tests/pi_plugin.rs` asserts the manifest still declares `pi.extensions` (the key omp's loader falls back to).
3. Unit tests: offer names `plugins/pi` and the ketch line; `--yes` links and writes `mcpServers.rtok`, second apply `NO_CHANGES`, remove takes back exactly ours and leaves foreign servers; `docs/agents.md` blessed; `tests/trycmd/agents-list*.toml` re-blessed.
Verified 2026-09-21 on omp 18.1.14 (probe extension in a scratch `PI_CODING_AGENT_DIR`, nothing written to `~/.omp`): (a) a **symlinked** directory whose `package.json` declares only legacy `pi.extensions` is discovered and its factory runs — the exact mechanism `plugins/pi` uses; (b) host signal: omp injects its SDK as `pi.pi` (an object with `getAgentDir()` and `VERSION`; `process.title` is `omp`), upstream pi's `ExtensionAPI` has no `pi` member — so `registerPiTools` returns early when `pi.pi` is an object, because that host has native MCP; without it a machine with pi (`[setup.pi] tools = true`) and omp would get every tool twice under omp (D21); (c) by source (`src/capability/mcp.ts` `key: server => server.name`, `src/capability/index.ts` first-wins dedupe in provider-priority order, native config highest): a native `rtok` entry and one imported from a Claude Code / Cursor config collapse into **one** server. Not verified: a real model turn whose bash call goes through `rtok run` — the only model key in this environment has no credit (`credit_balance_exhausted`); the creator runs one `omp -p` turn after install.
Also found by reading omp's source (`src/session/agent-session.ts` `#beforeToolCall`, `src/extensibility/extensions/wrapper.ts`): omp applies a revised input only when the handler **returns** `{ input }`; upstream pi documents mutating `event.input` in place. The bash rewrite reaches omp today only because omp hands `bash` handlers the live args object — an undocumented alias.

Split (each ≤200 LOC / ≤3 files):
- T92.1 (done, see `done.md`) — `plugins/pi/extensions/rtok.ts`: the bash rewrite mutates `event.input` in place **and** returns `{ input: event.input }` (pi ignores the extra field; omp's documented path); `registerPiTools` returns early when `pi.pi` is an object (omp — native MCP owns the tools). `plugins/pi/tests/rtok.test.ts`: two cases pin both. Check: `pi_plugin` green (it runs the Node test file).
- T92.2 — host `src/agents/omp/` (`mod.rs` + `README.md`), `HOSTS` / `host()` in `src/agents/mod.rs`, `[setup.omp]` in `config/default.toml` / `src/config/mod.rs` / `docs/config.md`, unit tests from Plan step 3.
- T92.3 — `docs/agents.md` bless, `tests/trycmd/*` re-bless, `plugins/pi/README.md` omp section.

Check: the unit tests above; `rtok agents list` shows `omp`; `agents_doc`, `host_docs`, `config_coverage`, `pi_plugin` green; `just check`.

## Reference

Historical phase notes (P0–P39) live in `done.md`. Companion evidence: `research.md`, `architecture.md`. Per-plugin plan: `roadmap.md`. Unapproved propositions: `ideas.md`.

Claim a `todo` row before work: set Status to `in progress` and Agent to `Provider / model`. Before stopping unfinished work, set Status to `todo` and clear Agent. When the Check passes, move the task entirely to `done.md` (Do/Check + Check result) and drop it from this table, its card, and `todo.md`.

### Decisions (read before any task)

| # | Decision | Why (evidence in `research.md`) |
| --- | --- | --- |
| D1 | **Rust, one static binary, in-tree plugins behind one trait + Cargo features.** No WASM, no subprocess plugins, no daemon **on the hook path in v0.1** (`rtok demon` supervises the long-running surfaces only, D22). A later WASM plugin host is P32 (landed); this repo still does not wrap third-party tools (D6). | Hooks run on every tool call; Rust cold start vs Python hooks. |
| D2 | **One binary, three surfaces:** `rtok hook` (Claude Code hooks), `rtok mcp` (MCP server), `rtok proxy` (`ANTHROPIC_BASE_URL`). | PostToolUse hooks cannot modify tool results. Only PreToolUse rewrite, MCP tool replacement, or a proxy can shrink what the model sees. |
| D3 | **Measurement first.** Nothing ships until `rtok stats` reads real usage from session logs and the proxy. Every plugin logs before/after. Metric = *context-token-turns* (tokens × turns they stay in context), plus output tokens. | Vendor claims vs measured savings; nobody in the stack measures end to end. |
| D4 | **Lossless by default.** Every compression keeps the original retrievable via `rtok expand <id>` / MCP `expand`. Lossy only where the source is regenerable (re-run the command). | Trust is the product. |
| D5 | **Injection budget.** All SessionStart/UserPromptSubmit injections go through one plugin with a per-turn token cap (default 800) and a byte-stable prefix. | Injections are re-read (cached) every turn. |
| D6 | **Every plugin is native, written from scratch in this repo. No third-party plugins.** A plugin never spawns, links, imports, or reads the data of another tool. Third parties extend rtok from outside through the public plugin API (`rtok-plugin-sdk`, `Registry::from_plugins`, `docs/plugin-authoring.md`), never through this repo. | One code path per method is what `Measurement` can attribute. |
| D7 | **Prompt "modes" (terse, YAGNI) are data files, not code.** | Measured effect must be A/B tested, not assumed. |
| D8 | **One SQLite file** (`~/.rtok/rtok.db`, WAL). Schema is D13. Raw archived payloads on disk under `~/.rtok/archive/` (D4); the DB holds indexes and inline JSON under a size cap. | Field tools converge on SQLite (+FTS5). |
| D9 | **Agents are provider-agnostic.** Route by job: low-cost for mechanical work; mid-tier for coding; high-performance for research only after the user confirms. Host names are products, not the implementer. | User constraint: cost-aware routing, any provider. |
| D10 | **Retire, don't stack.** Phase 9 replaces the legacy hook pile with ≤ 8 and drops every tool the A/B bench cannot justify. | Duplicated responsibilities across bash/read/memory/graph tools. |
| D11 | **The proxy speaks both wire formats.** Anthropic Messages and OpenAI Chat Completions / Responses are `Wire` adapters behind one proxy. Hosts point `ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL` at rtok. | One proxy, two parsers is cheaper than two proxies. |
| D12 | **One config file holds every setting; every CLI flag is a config key.** Precedence: defaults < user file < `<git root>/.rtok.toml` < `RTOK_<SECTION>_<KEY>` env < flags. `rtok config show --sources` tells where each value came from. | Hooks, long-running servers, and benches need one precedence rule. |
| D13 | **Core persists through a sync ORM on bundled SQLite.** Diesel (`sqlite` + bundled `libsqlite3-sys` with FTS5). Plugins never write SQL; `Store` is the only DB owner. Hook path: metadata always, body only if under the inline cap — never archive, never fail the hook (D1). | Diesel is sync, so the ≤ 10 ms hook path stays blocking and fail-open. |
| D14 | **CLI is clap 4 (derive); config layers are figment; TOML writes are toml_edit.** Env is `RTOK_<SECTION>_<KEY>` looked up in a leaf table from `Config::default()`. | Clap owns the subcommand tree; Figment tracks per-key provenance. |
| D15 | **Every plugin is designed against alternatives before it is built.** Each catalogue plugin has `src/plugins/<id>/PLAN.md`. | From-scratch only pays if the design beats what it retires. |
| D16 | **Git is `main` only.** Agents commit on `main`. Do not create feature branches. One task is still one commit (`<task-id>: <title>`). | `main` is the single line of work. |
| D18 | **The graph index lives in SQLite with the ledgers (D8).** LadybugDB and Grafeo were gated, frozen, then removed (P39). No live `lbug` / `graph-lbug` / `symbols_lbug.rs` / `grafeo` feature flags. SQL for symbols lives only in `src/store/symbols.rs`. D6 holds: no spawned graph tool. | Both graph-store candidates were priced and deleted per the gate. Survey archive: `src/plugins/graph/PLAN.md`. |
| D19 | **Observability is a projection of the ledgers, never a second recorder.** OpenTelemetry export reads existing rows and posts OTLP/HTTP JSON. Nothing runs on the hook path. | Delivery is at-least-once behind a per-stream watermark. |
| D20 | **Local web UI is an operator surface, not a catalogue plugin.** `rtok web` serves axum + a Slint WASM UI (`crates/rtok-webui`). Slint is not linked into the hook binary. | Linking Slint into `rtok hook` would fail the size/latency gate. |
| D21 | **Every new plugin is plugin and MCP as one unit, a singleton, with one call path per capability.** Host plugins load in that host's desktop app and its CLI. Missing `rtok`: fail open and print that it must be installed with ketch. | Duplicate MCP processes and duplicate call paths break D18 and measurement. |
| D22 | **`rtok demon` supervises rtok's own long-running surfaces**, not a catalogue plugin. Allow-list names (`proxy`, `mcp`, `dashboard`). Nothing in it is on the hook path. | The proxy is the wire hop; when it dies every host silently loses it. |
| D23 | **`rtok tui` and `rtok web` are two renderings of one operator model.** A page that exists on one surface and not the other is a defect. | Two independently built surfaces drift. |
| D24 | **`rtok report` renders; it never computes a number of its own.** It reads the D23 operator model. Recommendations are rules over those rows, never an LLM call. | A saving that is not a `Measurement` row does not exist (D3). |
| D25 | **The plugin contract is `rtok-plugin-sdk`.** Required methods are explicit; event methods keep no-op defaults. `rtok` is the only dispatcher. | One contract, no in-tree shortcut. |
| D26 | **One log with two readers:** a rotating text file and the `logs` table OTel exports. Bounded (`max_bytes` 1 MiB, `files` 5). | An unbounded log on a long-running proxy is a disk-full bug. |
| D27 | **Anything a command prints, or the store keeps, is a page on `rtok web` and `rtok tui`.** Writing commands stay CLI-only. | The two surfaces plus CLI must not disagree about what a session is. |
| D28 | **The agent-host contract is `rtok-agent-sdk`.** Installers go through it; host-specific code stays in `src/setup/<host>.rs`. | One write cycle, one plugin-offer body. |
| D29 | **Unit tests prefer a virtual filesystem (`testutil::Vfs`) over host TempDir/std::fs.** Pure path/content/size logic must not require real disk; Windows/macOS quirks are simulated in Vfs. Migrate hottest suites first (read/search/cmd/setup) as T56.x — not a big-bang rewrite of e2e. | Hermetic tests; reproducible CI; path-case and spaced-path bugs (T55) need a simulated FS. |
| D30 | **HTTPS uses webpki Mozilla roots (`use_preconfigured_tls`); one binary.** Corporate CAs via `SSL_CERT_FILE` (curl parity, fail closed). reqwest 0.13 `rustls` still links `rustls-platform-verifier`; `otool` showed Security.framework still present (T53.3). A second hook binary was rejected. | I-32: 1.3–1.5 ms dyld; dropping the `rustls` feature does not compile. |

### Architecture

```
                    ┌──────────────── rtok (one binary) ────────────────┐
 Claude Code ──hook─┤ hook <event>  → EventBus → plugins (Pre/Post/...) │
 Cursor/OpenCode ───┤ mcp           → tools: read search tree run expand │
                    │                 mem_save mem_search symbol callers │
 ANTHROPIC_BASE_URL ┤ proxy         → usage capture, live-zone rewrite   │──▶ api.anthropic.com
                    │ stats | bench | doctor | setup | run | expand      │
                    └──────────┬─────────────────────┬──────────────────┘
                         ~/.rtok/rtok.db        ~/.rtok/archive/
```

`Ctx` gives every plugin: the DB handle, the token estimator, the archive store, config, session id. `Measurement { plugin, kind, before_bytes, after_bytes, est_before, est_after, ref_id }` is the only way savings enter the DB.

Plugin catalogue (v0.1). Every plugin is native Rust written from scratch here (D6).

| id | spec (replaces; evidence in research.md) | surface | mechanism |
| --- | --- | --- | --- |
| `measure` | rtk gain, headroom savings, lean-ctx gain | `stats`, `bench`, proxy | session JSONL ingest + proxy `usage`; context-token-turns |
| `cmd` | rtk hook, lean-ctx ctx_shell | PreToolUse(Bash) → `rtok run` | archive raw output; per-family formatters + TOML rules; pointer trailer |
| `read` | lean-ctx ctx_read/search/tree | MCP `read`,`search`,`tree` + PreToolUse(Read) | modes via tree-sitter-tags; re-read dedup |
| `archive` | token-optimizer archive_result, headroom CCR | proxy live zone + `expand` | replace old large `tool_result` with pointer + head/tail |
| `proxy` | headroom proxy | `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` | passthrough + SSE; usage capture; never touches system, tools, or last 2 turns |
| `inject` | caveman shrink-hook, ponytail/caveman modes | SessionStart, UserPromptSubmit | budgeted, byte-stable prefix; modes as markdown |
| `guard` | token-optimizer refetch_guard | PreToolUse | identical read/command within N turns → deny |
| `memory` | engram, claude-mem | MCP `mem_save/search/get` | agent-written notes, SQLite FTS5 |
| `graph` | codebase-memory-mcp, serena, codegraph | MCP `symbol`,`callers`,`outline` | tree-sitter-tags index in SQLite; optional LSP is T30.2 |
| `toon` (off by default) | caveman toon, TOON | proxy/MCP | tabular JSON → TOON |

### Working agreement

- Graph backends: do not claim tasks that reintroduce `lbug` / `graph-lbug` / `symbols_lbug.rs` / `grafeo` / `graph-grafeo` / cmake-for-liblbug. Symbol index is SQLite only.
- One task = one commit on `main` only. Never skip the Check.
- Read `research.md` §3 (hook contract) before any hook task. Hook input is JSON on stdin; output is JSON on stdout; exit 0. Exit 2 blocks (PreToolUse only). PostToolUse can only add context.
- Fail open: any plugin error → log to DB and return the unmodified input/empty output. A hook that crashes must still exit 0 in ≤ 10 ms.
- No new dependency without a one-line justification in the commit message.
- Don't duplicate code or logic: find the existing helper and reuse it, or extract one shared helper at the responsible layer.
- Code style: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run` green before every Check.
- Anything unmeasurable is a bug in the plan: add a `Measurement` before adding a feature.
- Every new CLI flag gets a key in `config/default.toml` and a row in `docs/config.md` in the same commit (D12).
- No plugin shells out to, links, imports from, or reads the data of a third-party tool (D6).
- Every new plugin obeys D21.
---

## Review 2026-09-17 — bug hunt (post #37–#47)

Scope: `origin/main` after cross-platform agent fixes #37–#47. Local WIP from other agents was stashed (`preserve-other-agents-wip-before-docs-review-bugs-plan`) and not reviewed. No code fixes in this pass — findings tracked as T55.x.

### Blockers

None for the macOS/Linux happy path on current main. Windows correctness gaps below are P1.

### Should fix

1. ~~**T55.1**~~ — done in #49 (`display_rel` case-insensitive strip).
2. ~~**T55.2**~~ — done in #49 (`never_wrap` case-insensitive stem).
3. ~~**T55.3**~~ — done in #49 (`file_uri` percent-encoding).
4. ~~**T55.4**~~ — done in #49 (host-shell-safe wrap / cmd quoting).
5. ~~**T55.5**~~ — done in #49 (`search_max_bytes`).
6. ~~**T55.6**~~ — done in #49 (`call` in mcp.cmd).

### Nits

1. ~~**T55.7**~~ — done (`skip_word`; quoted `cd` paths bucket by family).
2. **`expand::parse_range` when start > line count** — empty slice quietly; optional clamp.
3. **`guard::strip_wrap`** — updated in #49 for PowerShell `''`.
4. **T55.8 / T55.9 / T55.10** — filed from the T55.7 code read: guard `read:` keys survive a mutating Bash, guard Bash key cwd-blind, three copies of `cmd_stem`.

### Residual still open (called out before)

- T55.1–T55.6 closed by #49.
- PATH / single-quote parsers — rtok: T55.7 closed; larger residual in ketch.
- Guard false denies — T55.8 (P2), T55.9; flag-aware read-only classes promoted from I-38 as T57.1.
- Test VFS migration — D29 / T56.x (`Vfs` helper in #49).
- T48.3 still todo: Cursor plugin mcp.json still bare `rtok mcp`.

### Second pass (2026-09-17, later the same day)

Scope: hook dispatcher/types, guard, cmd (hook/run/rules/formatters), read (mod/cache/hook/search), archive, proxy (mod/wire/anthropic/openai_chat/semantic_cache), expand, store (archive/decisions/read_cache paths), mcp session handling. Four findings reproduced with a scratch integration test (written, run, deleted — `cargo nextest run --test zz_review_repro` → 4 failed exactly as predicted); two filed from code read. No code fixes in this pass — findings tracked as T55.11–T55.16, propositions as I-39/I-40.

- ~~**T55.11 (P2)**~~ — done: `expand` never froze the owning session's pointer (every expand caller runs under session `expand` / `mcp-<pid>`, decisions belong to the proxy session; expand rate stayed 0, toon attribution dead). Reproduced.
- **T55.12 (P2)** — Windows `wrap_quote` `''` quoting is wrong under Git Bash (Claude Code's Windows shell): apostrophes silently dropped from the rewritten command. Code read.
- ~~**T55.13 (P3)**~~ — done: Copilot `Read` inputs use `path`; `guard::cache_key` and `read::hook` match `file_path` only → dedup and advice never fire on that host. Reproduced.
- ~~**T55.14 (P3)**~~ — done: the semantic-cache key ignored tool_result text; two requests differing only in tool results hashed identically and were both eligible under defaults. Reproduced.
- **T55.15 (P3)** — `live_blobs` overwrites image/document `source.data` / `image_url.url` with pointer text → invalid request when the flag is on. Reproduced.
- ~~**T55.16 (P3)**~~ — done: the guard deny read the full archived body (and estimated over it) on the ≤ 10 ms PreToolUse path. Code read.

### Out of scope this pass

Concurrent agent WIP on local `main` (stashed as `preserve-other-agents-wip-before-docs-review-bugs-plan`). Half-finished T48–T53 cards on that WIP were not judged as shipped bugs.

---

## Note 2026-09-17 — testing library candidates

Shared catalog: [`listepo/rust.md`](../../rust.md) → *Testing candidates*.
Catalog only — no blanket `Cargo.toml` adds (Working agreement: justify each dep).

Fits for rtok (1–3):

1. `vfs` (crates.io) — evaluate against in-house T56 `src/testutil.rs` `Vfs`
   before adopting; mandate stays "prefer VFS over host TempDir".
2. `mockall` — trait mocks for provider / plugin host seams when hand fakes
   get noisy (`httpmock` stays for HTTP).
3. `tokio-test` — async unit helpers beyond `#[tokio::test]` where time/task
   control matters.

Already covered: `assert_cmd`, `divan`, `httpmock`, `insta`, `rstest`,
`trycmd`, `similar`. Skip `test-case` / `expect-test` / `mockito` duplicates;
`testcontainers` / `bolero`/`honggfuzz` only if a measured e2e/fuzz gap appears.
