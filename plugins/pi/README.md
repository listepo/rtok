# rtok pi package

Linked by `rtok agents install pi --yes` to `~/.pi/agent/extensions/rtok` (D21: one bash call
path, no MCP). `rtok agents install pi --remove` unlinks it. By hand: `pi install /path/to/plugins/pi`.

Files:

- `package.json` — pi manifest: `pi.extensions` → `extensions/rtok.ts`, `pi.skills` → `skills/`.
- `extensions/rtok.ts` — `tool_call` bash → `rtok run -- <command>`; `tool_result` bash →
  `rtok filter` with an `expand <id>` trailer. Missing `rtok` fails open and names ketch
  (`ketch install listepo/rtok`).
- `skills/rtok/SKILL.md` — tells the model how to recover full output (`rtok expand <id>`).
- `tests/rtok.test.ts` — Node unit test of the extension against a fake `rtok` on PATH; run by
  `tests/pi_plugin.rs`. Outside `extensions/`, so pi never loads it.

## Docs

Host documentation this package is written against. Re-check every link when the package changes.

- Extensions (`~/.pi/agent/extensions/*.ts` or `*/index.ts`; `tool_call`, `tool_result`, `pi.sendMessage`): https://pi.dev/docs/latest/extensions
- Packages (`package.json` `pi` key: `extensions`, `skills`, `prompts`, `themes`; `pi install <path>`): https://pi.dev/docs/latest/packages
- Skills (`SKILL.md` frontmatter `name`, `description`): https://pi.dev/docs/latest/skills and https://agentskills.io/specification
- Source of the docs: https://github.com/earendil-works/pi/tree/main/packages/coding-agent/docs
