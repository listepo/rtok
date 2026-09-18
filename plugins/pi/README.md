# rtok pi package

Linked by `rtok agents install pi --yes` to `~/.pi/agent/extensions/rtok` (D21: one bash call
path, no MCP). `rtok agents install pi --remove` unlinks it. By hand: `pi install /path/to/plugins/pi`.
pi loads a directory in `extensions/` from its `package.json` `pi.extensions` first and falls back to
`index.ts`, so the link needs no `index.ts` (pi 0.85.1 `resolveExtensionEntries`; the docs list only `*/index.ts`).

Files:

- `package.json` — pi manifest: `pi.extensions` → `extensions/rtok.ts`, `pi.skills` → `skills/`.
- `extensions/rtok.ts` — `tool_call` bash → `rtok run -- <command>`; `tool_result` bash →
  `rtok filter` with an `expand <id>` trailer; `context` → `rtok archive rewrite --stdin`
  (the archive live zone without a proxy, T70.2); `session_before_compact` →
  `rtok hook PreCompact --host pi` (T70.6, checkpoint save; the extension never returns a
  replacement summary). After `session_compact`, the next `context` call injects
  `PostCompact` restore bytes. Missing `rtok` fails open and names ketch
  (`ketch install listepo/rtok`). Hook hosts (Claude/Cursor/Codex/Copilot) are T58.2.
- `skills/rtok/SKILL.md` — tells the model how to recover full output (`rtok expand <id>`).
- `tests/load.test.ts` — loads the linked directory with pi's own `discoverAndLoadExtensions` and expects
  one extension with `tool_call` and `tool_result`; skipped when pi is not installed.
- `tests/rtok.test.ts` — Node unit test of the extension against a fake `rtok` on PATH (`tests/node/fake-rtok.ts`, every OS); run by
  `tests/pi_plugin.rs`. Outside `extensions/`, so pi never loads it.

## Docs

Host documentation this package is written against. Re-check every link when the package changes.

- Extensions (`~/.pi/agent/extensions/*.ts` or `*/index.ts`; `tool_call`, `tool_result`, `context`, `session_before_compact`, `session_compact`): https://pi.dev/docs/latest/extensions
- Compaction (`session_before_compact` may cancel or replace the summary; no append field): https://pi.dev/docs/latest/compaction
- Packages (`package.json` `pi` key: `extensions`, `skills`, `prompts`, `themes`; `pi install <path>`): https://pi.dev/docs/latest/packages
- Skills (`SKILL.md` frontmatter `name`, `description`): https://pi.dev/docs/latest/skills and https://agentskills.io/specification
- Source of the docs: https://github.com/earendil-works/pi/tree/main/packages/coding-agent/docs
