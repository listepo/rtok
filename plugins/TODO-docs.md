# Documentation gaps (`plugins/`)

Tasks to close gaps between code and docs. Track here until folded into `plan.md`.

| # | Task |
| --- | --- |
| D1 | Root package table: add install command one-liner per host (`rtok agents install …` vs manual). |
| D2 | Document Windows vs POSIX launcher story for `zcode/` and `cursor/` scripts in one place. |
| D3 | Cross-link each package README to its `src/agents/<host>/README.md` surface table (relative path). |
| D4 | Spell out Grok ↔ Claude double-fire risk with a short “choose one” checklist (already partly in grok README). |
| D5 | OpenCode/Kilo: document that `--host opencode` labels Kilo rows; when/if a kilo host tag appears. |
| D6 | pi: document `[setup.pi] tools = true` MCP-via-registerTool path with a minimal config snippet. |
| D7 | Antigravity: explicit “why no hooks” decision table vs hosts that support rewrite. |
| D8 | Shared vs per-host `mcp.sh`/`mcp.cmd`: either extract a tested shared helper or document intentional divergence (hashes differ today). |
| D9 | Agent Plugins / Cursor dual manifest (`plugin.json` + `.cursor-plugin/plugin.json`) — when each applies. |
| D10 | End-to-end “new host checklist” as a checked list in root AGENTS (link from plan when scheduled). |
| D11 | Examples: minimal fake host package under `examples/` or docs — optional, only if maintainers want it. |
| D12 | Keep `docs/agents.md` matrix rows in sync when a package is added/removed. |
