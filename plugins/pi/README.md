# rtok pi package

Linked by `rtok agents install pi --yes` to `~/.pi/agent/extensions/rtok` (D21: one bash call
path, no MCP). `rtok agents uninstall pi` unlinks it (`install pi --remove` is the same). By hand: `pi install /path/to/plugins/pi`.
pi loads a directory in `extensions/` from its `package.json` `pi.extensions` first and falls back to
`index.ts`, so the link needs no `index.ts` (pi 0.85.1 `resolveExtensionEntries`; the docs list only `*/index.ts`).

Files:

- `package.json` — pi manifest: `pi.extensions` → `extensions/rtok.ts`, `pi.skills` → `skills/`.
- `extensions/rtok.ts` — `tool_call` → `rtok guard check` (T70.5: `{block:true, reason}` when
  a duplicate read/command is denied; missing `rtok`, non-zero, unparsable or a deny without
  a reason fails open); bash then rewrites to `rtok run -- <command>`; `tool_result` seeds
  the guard cache via `rtok hook PostToolUse`, then bash → `rtok filter` with an `expand <id>`
  trailer; `tool_result` read/grep/find/ls → `rtok filter --stdin --cmd "<tool> <path-or-pattern>"`
  (T70.1); `context` → `rtok archive rewrite --stdin` (T70.2); `session_before_compact` →
  `rtok hook PreCompact --host pi` (T70.6, checkpoint save; the extension never returns a
  replacement summary). After `session_compact`, the next `context` call injects
  `PostCompact` restore bytes. When `[setup.pi] tools = true`, `session_start`
  registers the measured MCP set through `pi.registerTool` as `rtok mcp --call` (T70.3).
  Missing `rtok` fails open and names ketch (`ketch install listepo/rtok`). Hook hosts
  (Claude/Cursor/Codex/Copilot) are T58.2.
- `skills/rtok/SKILL.md` — tells the model how to recover full output (`rtok expand <id>`).
- `tests/load.test.ts` — loads the linked directory with pi's own `discoverAndLoadExtensions` and expects
  one extension with `tool_call` and `tool_result`; skipped when pi is not installed.
- `tests/rtok.test.ts` — Node unit test of the extension against a fake `rtok` on PATH (`tests/node/fake-rtok.ts`, every OS); run by
  `tests/pi_plugin.rs`. Outside `extensions/`, so pi never loads it.

## Docs

Host documentation this package is written against. Re-check every link when the package changes.

- Extensions (`~/.pi/agent/extensions/*.ts` or `*/index.ts`; `tool_call`, `tool_result`, `context`, `session_before_compact`, `session_compact`, `registerTool`): https://pi.dev/docs/latest/extensions
- Compaction (`session_before_compact` may cancel or replace the summary; no append field): https://pi.dev/docs/latest/compaction
- Packages (`package.json` `pi` key: `extensions`, `skills`, `prompts`, `themes`; `pi install <path>`): https://pi.dev/docs/latest/packages
- Skills (`SKILL.md` frontmatter `name`, `description`): https://pi.dev/docs/latest/skills and https://agentskills.io/specification
- Source of the docs: https://github.com/earendil-works/pi/tree/main/packages/coding-agent/docs

## Package docs

- Agents working on this package: [`AGENTS.md`](AGENTS.md)
- All host plugins: [`../README.md`](../README.md)
- Agent rules for `plugins/`: [`../AGENTS.md`](../AGENTS.md)
- Doc gaps: [`../TODO-docs.md`](../TODO-docs.md)

