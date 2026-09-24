# rtok — instructions for agents

**What.** One Rust binary cutting tokens for coding agents; every method is a plugin. Surfaces: `rtok hook <event>`, `rtok mcp`, `rtok proxy` (`ANTHROPIC_BASE_URL`).

**Platforms.** Windows, macOS, and Linux are first-class; see `docs/agency.md`.

**Read first.** `plan.md` (tasks, Checks, D1–D30), `architecture.md`, `research.md`. Also `roadmap.md`, `ideas.md` (propositions; never implement), `done.md`. Per plugin: `src/plugins/<id>/AGENTS.md`. Do not add unplanned work.

**Workflow.** Claim a `todo` row in `plan.md` first. One task = one branch = one PR off `origin/main`; ≤200 LOC, ≤10 files. Finish: `just check`, commit `<id>: <title>`, move task to `done.md`.

**Rules.** Fail open: hook exits 0 in ≤10 ms unmodified. Lossless: shortened output via `expand <id>`. No `Measurement` row = no saving, in code or prose. Injections budgeted, byte-stable. PostToolUse adds context only. No new dep without one-line reason. No raw SQL (Diesel + `schema.rs`). No duplicated logic. Skills live only in `skills/`; installers copy them, plugins never bundle one (T234). New plugins/hosts obey D21.

**Models.** Any provider. Low-cost: docs, scans, commands — never code. Mid-tier: all code. High-performance: research only with user OK. Re-run tests and read the diff; sub-agent "green" is not evidence.

Details in `CONTRIBUTING.md`. `CLAUDE.md` symlinks here — edit this file only. Keep this file under 350 tokens.
