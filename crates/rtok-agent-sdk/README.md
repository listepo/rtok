# rtok-agent-sdk

The agent-host half of [rtok](https://github.com/listepo/rtok): what `rtok agent setup <host>` and
`rtok agent remove <host>` do to a host's configuration, factored out of the hosts themselves.

Five hosts ship in rtok — Claude Code, Cursor, Codex, OpenCode, pi — and each one edits a different
file in a different format. What they share is the contract around the edit, and that is this
crate:

- **Reversible** — `backup` copies a file to `<name>.bak-<unix-seconds>` before the first write,
  so one `.bak-*` per file is the whole undo.
- **Idempotent** — a second apply returns `NO_CHANGES` and writes nothing.
- **Dry-runnable** — `--dry-run` produces the same report and touches nothing at all.
- **Offered, never forced** — `PluginLink` prompts on a terminal, declines itself anywhere else
  (CI, a pipe, a host running setup for the user), and takes `--yes` as the answer.

The report line *is* the write gate: `Apply` refuses to write when the report is
`NO_CHANGES`, so "nothing changed" and "nothing was written" cannot drift apart.

```rust,ignore
use rtok_agent_sdk::{Apply, PluginLink, register_mcp};

let apply = Apply { dry_run: false, backup: true, yes: false };

// A host that reads `mcpServers` — Claude Code's `~/.claude.json`, Cursor's `~/.cursor/mcp.json`.
let report = register_mcp(&apply, path, "rtok", "rtok", &["mcp"])?;

// A host that loads a plugin directory.
let report = PluginLink {
    src_rel: "plugins/cursor",
    src: repo.join("plugins/cursor"),
    dest: cursor_dir.join("plugins/local/rtok"),
    label: Some("~/.cursor/plugins/local"),
    host: "Cursor",
}
.run(&apply, false)?;
# Ok::<(), anyhow::Error>(())
```

Three dependencies: `serde_json`, `anyhow`, and `dialoguer` for the one prompt.

Licensed under Apache-2.0.
