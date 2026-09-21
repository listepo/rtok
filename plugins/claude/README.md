# rtok Claude Code plugin

Claude Code's plugin form of rtok: the same hooks `rtok agents install claude` writes into
`~/.claude/settings.json`, and one `rtok mcp`, as one unit (D21). Claude Code loads it in the CLI
and in the desktop app's Code tab. This directory is also its own marketplace
(`.claude-plugin/marketplace.json`: `rtok`, plugin `rtok`, source `./`). By hand:

```bash
claude plugin marketplace add <rtok>/plugins/claude
claude plugin install rtok@rtok
```

Remove with `claude plugin uninstall rtok@rtok` and `claude plugin marketplace remove rtok`.
Installing the plugin and also running a plain `rtok agents install claude` fires every hook
twice; `rtok agents install claude --yes` (T115) will do the plugin install and keep one call
path.

Files:

- `.claude-plugin/plugin.json` — manifest (`name` `rtok`); `hooks/hooks.json` and `.mcp.json` are
  found by convention. Claude copies the plugin into `~/.claude/plugins/cache/`, so the tree is
  self-contained.
- `.claude-plugin/marketplace.json` — the one-plugin marketplace `claude plugin marketplace add`
  reads.
- `hooks/hooks.json` — the installer's nine entries (`claude::ENTRIES`: PreToolUse Bash, Read,
  Skill; PostToolUse `*`; UserPromptSubmit; SessionStart; PreCompact; PostCompact; SessionEnd) →
  `rtok hook <event>` through `scripts/hook.sh`, `timeout` 5 s. A unit test in
  `src/agents/claude/mod.rs` keeps them equal.
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
