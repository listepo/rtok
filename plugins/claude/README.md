# rtok Claude Code plugin

Claude Code's plugin form of rtok: the same hooks `rtok agents install claude` writes into
`~/.claude/settings.json`, and one `rtok mcp`, as one unit (D21). Claude Code loads it in the CLI
and in the desktop app's Code tab. `rtok agents install claude` installs it from the GitHub
marketplace at the repo root (`.claude-plugin/marketplace.json`: `rtok`, plugin `rtok`, source
`./plugins/claude`) — a local path broke across a ketch upgrade (T139). By hand:

```bash
claude plugin marketplace add listepo/rtok
claude plugin install rtok@rtok
```

Remove with `claude plugin uninstall rtok@rtok` and `claude plugin marketplace remove rtok`.
`rtok agents install claude` runs both commands by default — no `--yes` needed — once `claude`
is on PATH, skipping `marketplace add` when Claude already knows the marketplace; while the
plugin is installed it strips rtok's own hooks from `~/.claude/settings.json` and
`mcpServers.rtok` from `~/.claude.json`, so every event fires once (D21). `rtok agents remove
claude` uninstalls it. Installing by hand and then running a plain `rtok agents install claude`
leaves that singleton rule in force too.

Files:

- `.claude-plugin/plugin.json` — manifest (`name` `rtok`); `hooks/hooks.json` and `.mcp.json` are
  found by convention. Claude copies the plugin into `~/.claude/plugins/cache/`, so the tree is
  self-contained.
- `.claude-plugin/marketplace.json` — this directory's own one-plugin marketplace (source `./`),
  kept for local/dev use (`claude plugin marketplace add plugins/claude`); the installer itself
  now adds the repo-root marketplace (`../../.claude-plugin/marketplace.json`, source
  `./plugins/claude`) by its GitHub shorthand `listepo/rtok`.
- `hooks/hooks.json` — the installer's nine entries (`claude::ENTRIES`: PreToolUse Bash, Read,
  Skill; PostToolUse `*`; UserPromptSubmit; SessionStart; PreCompact; PostCompact; SessionEnd) →
  `rtok hook <event>`, `timeout` 5 s. The command execs `rtok` from PATH in Claude Code's own
  shell and runs `scripts/hook.sh` only when PATH has none: the second shell cost ~6 ms per call
  (`research.md` §19). A unit test in `src/agents/claude/mod.rs` keeps them equal.
- `.mcp.json` — `mcpServers.rtok` → `scripts/mcp.sh`.
- `scripts/hook.sh`, `scripts/mcp.sh` — resolve `rtok` from PATH or the ketch store; a missing
  `rtok` fails the hook open (exit 0) and the MCP loudly (exit 1), both printing
  `ketch install listepo/rtok`.

Windows: Claude Code runs hook commands through Git Bash, so `hook.sh` works; the MCP launcher is
POSIX too, so on Windows prefer the plain `rtok agents install claude`.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (layout, `.claude-plugin/plugin.json`, `--plugin-dir`): https://code.claude.com/docs/en/plugins
- Plugins reference (`${CLAUDE_PLUGIN_ROOT}`, hooks and MCP in a plugin): https://code.claude.com/docs/en/plugins-reference
- Marketplaces (`marketplace.json`, relative `source`, `claude plugin marketplace add`): https://code.claude.com/docs/en/plugin-marketplaces
- Hooks (`hooks.json` shape, events, `timeout`): https://code.claude.com/docs/en/hooks
