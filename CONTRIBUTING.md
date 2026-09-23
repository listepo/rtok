# Contributing

Repository prose is English. Read [`AGENTS.md`](AGENTS.md) before changing
code — layout, plugin surfaces, and the fail-open / measurement invariants.

## Checks

```bash
just check
just example
just readme-check
```

Do not invent metrics or A/B wins in docs. Numbers in the README and
[`docs/comparison.md`](docs/comparison.md) must match committed evidence
(`research.md`, bench results). User guides: [`docs/getting-started.md`](docs/getting-started.md)
and the site under `site/`.

## Expanded rules (from `AGENTS.md`)

- Hook contract: read `research.md` §3 before any hook task. Hook input is
  JSON on stdin, output JSON on stdout, exit 0. Exit 2 blocks (PreToolUse
  only). A hook that crashes must still exit 0.
- No new dependency without a one-line justification in the commit message.
- Code style: `cargo fmt`, `cargo clippy -D warnings`,
  `cargo nextest run` green before every Check.
- Every new CLI flag gets a key in `config/default.toml` and a row in
  `docs/config.md` in the same commit (D12).
- No plugin shells out to, links, imports from, or reads the data of a
  third-party tool (D6).
- Graph backends: never reintroduce `lbug` / `graph-lbug` /
  `symbols_lbug.rs` / `grafeo` / `graph-grafeo` / cmake-for-liblbug. Symbol
  index is SQLite only.
- A saving that is not a `Measurement` row does not exist — in prose too:
  every number in `README.md`, `docs/` or the site cites a measured row,
  `research.md`, or a dated command, never a vendor-style claim.
- The human is the only author. No agent adds a `Co-Authored-By` trailer, a
  "Generated with …" line, or itself as author to a commit, merge, or PR.
- Host plugins (D21): `rtok agents install <host>` offers `plugins/<host>/`
  (Cursor: `rtok agents install cursor`). If `rtok` is missing, fail open and
  say to install with ketch (`ketch install listepo/rtok`). Every
  `plugins/<host>/README.md` and `src/agents/<host>/README.md` has a
  `## Docs` list linking the host's current config and plugin documentation;
  re-verify links on each change (`tests/host_docs.rs`). After adding or
  changing a host or plugin surface, rerun `tests/agents_doc.rs` with
  `RTOK_BLESS=1` and commit the regenerated `docs/agents.md` table.
- New plugin: see `docs/plugin-authoring.md`.
- Parent rules: if a directory above this repository contains an `AGENTS.md`
  or `CLAUDE.md`, follow it too. On conflict with `AGENTS.md`, ask the creator.

## Models (expanded)

Low-cost (e.g. Haiku) only for docs, scans, lookups, and running commands —
never for code. Mid-tier for any code change, however small. High-performance
for research and investigation only after the user confirms — do not switch up
alone. On Cursor: grok 4.6 (no fast) for planning, refactoring, bugs;
composer 2.5 (no fast) for commands, tests, file moves, scans, web. No max
effort or fast without permission. ≤5 agents per project unless told
otherwise. Ask if unclear; write the execution plan into the `plan.md` card
before claiming. Before writing code, find the best ready library or framework
and use it; custom code only when every ready option is worse (abandoned,
heavier, poor fit) — say why in the commit message. A maintained dependency
needs no approval: add it with its one-line reason and a `toolchain.md` row;
never add an abandoned one. Prefer latest tool/package versions, but bump
already-installed ones only with the creator's permission. Rust: reuse crates
already used by sibling projects (workspace-root `rust.md`); if this repo
lacks one it should use, add it and update `toolchain.md` and `rust.md` in
the same change. Extract duplicated helpers into `packages/` via local
`{ path = "…" }`. No version bumps without permission.

## Toolchain, containers, docs

- Rust is pinned in `mise.toml`. Run everything as
  `mise exec -- cargo <cmd>` (or `mise activate` your shell). Never install
  or switch a global toolchain.
- Containers: use Colima + Docker CLI (mise pins), not Docker Desktop — see
  `docs/colima.md`.
- `README.md` and `docs/` are the public surface; the Hugo site mounts repo
  markdown read-only, so a repo file *is* the page (`just site` builds; a new
  `docs/*.md` page needs a row in
  `site/content/docs/reference/_content.gotmpl`). Brand assets live in
  `site/static/` and are shared with the README.
- Before you start: delegate one-off shell (build, test, git, cargo),
  API/HTTP, and file listings; do not run those from the main context.

## Testing

Run tests with `just test` — the default locally and in CI (`just check`):
`-j` = logical CPUs (`--test-threads {{cpus}}`). Heavy tests (cold repo
index, 100-session memory bench, 3 000-file graph bench) run alone via
`threads-required = "num-test-threads"` in `.config/nextest.toml`; add a
matching `[[profile.default.overrides]]` there for new resource-hungry tests,
never `--test-threads=1` in the test.

Unit tests for logic; integration tests (`assert_cmd`, `predicates`,
`assert_fs`, `trycmd`) for the binary, args, and output — see `plan.md` →
Reference / Working agreement.

Prefer `crate::testutil::Vfs` (in-memory path → bytes) over host `TempDir`
for unit tests that only need path/content/size. See plan D29 / T56.

JS/TS tests (host plugins) use vitest (`vitest.config.mjs`, globals, no
`vitest` import); never `node:test`/`node:assert`. Prefer
`toMatchInlineSnapshot` for structured output. See T111.
