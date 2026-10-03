# rtok Claude Code plugin

Claude Code's plugin form of rtok: the same hooks `rtok agents install claude` writes into
`~/.claude/settings.json`, plus the `rtok-scout` sub-agent. It carries no MCP server of its own
(T275): `rtok mcp` reaches Claude Code and Claude Desktop only through `mcpServers.rtok`,
written into `~/.claude.json` and `claude_desktop_config.json` by `rtok agents install claude`
whether or not this plugin is installed. Claude Code loads the plugin in the CLI
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
plugin is installed it strips rtok's own hooks from `~/.claude/settings.json`, so every hook
event fires once (D21) — `mcpServers.rtok` is unaffected, since MCP is no longer part of that
singleton (T275). `rtok agents remove claude` uninstalls it and takes the MCP entries back out
too.

Files:

- `.claude-plugin/plugin.json` — manifest (`name` `rtok`); `hooks/hooks.json` is found by
  convention. Claude copies the plugin into `~/.claude/plugins/cache/`, so the tree is
  self-contained.
- `.claude-plugin/marketplace.json` — this directory's own one-plugin marketplace (source `./`),
  kept for local/dev use (`claude plugin marketplace add plugins/claude`); the installer itself
  now adds the repo-root marketplace (`../../.claude-plugin/marketplace.json`, source
  `./plugins/claude`) by its GitHub shorthand `listepo/rtok`.
- `hooks/hooks.json` — the installer's ten entries (`claude::CLAUDE_ENTRIES`: PreToolUse Bash, Read,
  Skill; PostToolUse `*`; UserPromptSubmit; SessionStart; PreCompact; PostCompact; SessionEnd;
  SubagentStart — the spawn brief, T130) →
  `rtok hook <event>`, `timeout` 5 s. The command execs `rtok` from PATH in Claude Code's own
  shell and runs `scripts/hook.sh` only when PATH has none: the second shell cost ~6 ms per call
  (`research.md` §19). A unit test in `src/agents/claude/mod.rs` keeps them equal.
- `hooks/hooks.json` also carries `WorktreeCreate` and `WorktreeRemove` (T159, D31), plugin only and
  never in `settings.json`: they replace the host's own create/remove, so each runs
  `scripts/worktree.sh`, `timeout` 120 s, which routes through `rtok worktree` and falls back to
  the host's default (`.claude/worktrees/<name>`) on any rtok failure.
- `scripts/hook.sh` — resolves `rtok` from PATH or the ketch store; a missing `rtok` fails the
  hook open (exit 0), printing `ketch install pyrlyn/rtok`.
- `scripts/worktree.sh` — the worktree hooks' launcher. `WorktreeCreate` must print a path, so a
  missing, failing or too-old `rtok` ends in a plain `git worktree add` instead of silence;
  `WorktreeRemove` without `rtok` is a plain `git worktree remove`, never forced.
- `agents/rtok-scout.md` — a `model: haiku` sub-agent (T132) scoped to the rtok MCP's `read`,
  `search`, `outline`, `explore`, `expand` tools (named `mcp__rtok__<tool>`, the plain form for
  the `mcpServers.rtok` config entry — T275, this plugin ships no `.mcp.json` of its own), so
  code-lookup questions default to the cheap path instead of a full-price general-purpose agent.
  Discovered automatically from `agents/` — no manifest entry needed.

Windows: Claude Code runs hook commands through Git Bash, so `hook.sh` works.

## Docs

Host documentation this plugin is written against. Re-check every link when the plugin changes.

- Plugins (layout, `.claude-plugin/plugin.json`, `--plugin-dir`): https://code.claude.com/docs/en/plugins
- Plugins reference (`${CLAUDE_PLUGIN_ROOT}`, hooks and MCP in a plugin): https://code.claude.com/docs/en/plugins-reference
- Marketplaces (`marketplace.json`, relative `source`, `claude plugin marketplace add`): https://code.claude.com/docs/en/plugin-marketplaces
- Hooks (`hooks.json` shape, events, `timeout`): https://code.claude.com/docs/en/hooks
- Hooks, `WorktreeCreate` / `WorktreeRemove` (input, path output, exit codes): https://code.claude.com/docs/en/hooks#worktreecreate
- Worktrees (default location, branch and cleanup the launcher falls back to): https://code.claude.com/docs/en/worktrees
- Sub-agents (frontmatter: `name`, `description`, `tools`, `model`): https://code.claude.com/docs/en/sub-agents
