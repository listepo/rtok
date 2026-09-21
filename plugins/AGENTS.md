# AGENTS.md — `plugins/`

Rules for AI agents editing host plugin packages under `plugins/`. Humans: see [`README.md`](README.md).

## Scope

- **In scope:** `plugins/<host>/` marketplace / local plugin packages.
- **Out of scope:** `src/plugins/` (rtok reduction plugins). Do not move those here.
- **Language:** English only in all files under this tree (project rule).

## Mandatory package docs

Every plugin directory under `plugins/` **MUST** contain:

| File | Audience | Required contents |
| --- | --- | --- |
| `README.md` | Humans | What the package is, how to install/remove, file map, links to host docs, hooks/MCP behaviour for **this** host, link to [`README.md`](README.md) |
| `AGENTS.md` | Agents | Contracts and do/don’t rules for **this** package, links to this file, configs/manifests, hooks/MCP entrypoints |

**Without both files the plugin is incomplete.** They keep humans and agents aligned, stop drive-by edits that break host layouts, and point at the shared root guides instead of re-deriving install paths from memory.

If either file is missing, add it before other changes. If it exists, update it — do not fork a second copy.

## Hard rules

1. **No `plugins/claude/`** unless Anthropic ships a real linkable plugin directory. Claude is `src/agents/claude/` only. Document absence in root README; do not stub a non-installable package.
2. **Do not break host discovery paths.** Manifests, `hooks/hooks.json`, `.mcp.json`, `${ZCODE_PLUGIN_ROOT}`, Cursor `mcp.json`, etc. are host contracts.
3. **Prefer calling the rtok binary** (`hook`, `mcp`, `guard check`, `filter`, …) over reimplementing policy in TS/shell.
4. **D21:** one plugin unit owning hooks+MCP when both exist; one `rtok mcp` per store. Warn in README when host also imports Claude settings (see `grok/`).
5. **Fail open** on missing `rtok` unless the host’s block semantics require otherwise; mention `ketch install listepo/rtok`.
6. **Keep installer and package in sync.** Manifest event lists must match `src/agents/<host>/` (follow existing parity tests).
7. **Thin launchers.** Do not merge cursor/zcode scripts into one shared file unless both hosts expand paths identically — prefer documenting duplication in [`TODO-docs.md`](TODO-docs.md) over a risky shared script.
8. **Tests:** update `tests/*_plugin.rs` / package Node tests when behaviour changes.

## Starting a new host package

1. Copy the closest peer layout (see root README table).
2. Add `README.md` + `AGENTS.md` in the same change.
3. Add `src/agents/<host>/` install/remove/doctor (+ README surface table).
4. Register the host in `src/agents/mod.rs` if needed.
5. Add parity / e2e coverage.
6. Link from root `plugins/README.md` package table.

## MCP pattern

- JSON: `mcpServers.rtok` → `rtok mcp` or a launcher script.
- TS hosts: either MCP config written by installer, or `registerTool` / `rtok mcp --call` (pi when enabled).

## Hooks pattern

- JSON hosts: `hooks.json` → `rtok hook <Event>` [`--host <name>`].
- TS hosts: map host events to `guard check` / `hook` / `filter` / compact hooks as in `opencode/rtok.ts` or `pi/extensions/rtok.ts`.

## Refactor policy

Extract shared code only when byte-identical behaviour is proven across hosts. Host-specific env expansion stays local. Large decompositions of working TS plugins need tests green before merge.
