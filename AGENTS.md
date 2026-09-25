# rtok — instructions for agents

**What.** One Rust binary cutting tokens for coding agents; every method is a plugin. Surfaces: `rtok hook <event>`, `rtok mcp`, `rtok proxy`.

**Platforms.** Only Windows, macOS, and Linux (first-class); see `docs/agency.md`.

**Read first.** `plan.md`, `architecture.md`, `research.md`. Also `roadmap.md`, `ideas.md` (never implement), `done.md`. Per plugin: `src/plugins/<id>/AGENTS.md`. No unplanned work.

**Workflow.** Claim a `todo` row in `plan.md` first. One task = one branch = one PR off `origin/main`; ≤300 LOC, ≤10 files. Finish: `just check`, commit `<id>: <title>`, move task to `done.md`.

**Rules.** Fail open: hook exits 0 in ≤10 ms unmodified. Lossless: `expand <id>`. No `Measurement` row = no saving claim. Injections budgeted, byte-stable. PostToolUse adds context only. New dep: one-line reason. No raw SQL (Diesel). No duplicated logic. Skills only in `skills/`, never bundled (T234). New plugins/hosts obey D21. Own TOML: schema from types, one config module (T238). Host configs: no schema; check only our entry, rest byte-for-byte.

**Models.** Low-cost: docs, scans, commands — never code. Mid-tier: code. High-performance: research, user OK only. Re-run tests, read the diff; sub-agent "green" is not evidence.

`AGENTS.md`/`CLAUDE.md` above this repo apply too; on conflict ask the creator. Details: `CONTRIBUTING.md`. `CLAUDE.md` symlinks here. Keep under 350 tokens.
