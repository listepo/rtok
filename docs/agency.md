# Agency: cross-platform product

This is the main **agency** text for coding agents working on rtok.

## Platforms

**rtok is a cross-platform product.** It supports — and **must keep supporting** —
**Windows**, **macOS**, and **Linux** as first-class targets.

Treat every change as if it will run on all three:

- Paths, env vars, shells, line endings, and install layouts differ by OS; do
  not assume Unix-only (`~/.…`, `sh`, symlinks, `chmod`) without a Windows path.
- Hooks, MCP, and `rtok proxy` are the same surfaces on every OS; host install
  paths and GUI reopen behaviour still need Windows + macOS + Linux coverage
  (see [agents.md](agents.md)).
- Release already ships `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and
  `x86_64-unknown-linux-gnu` (see [release.md](release.md)). Prefer fixes that
  keep that matrix green; do not drop or soft-deprecate a platform without an
  explicit user decision.
- CI, docs, scripts, and examples that only work on macOS/Linux are incomplete
  until Windows is addressed (or the gap is documented as a known limit with a
  tracked follow-up).

## What this is not

This file is **not** the host × module matrix ([agents.md](agents.md)) and not
a substitute for root `AGENTS.md` (workflow, Checks, fail-open rules). It
states the platform contract those agents must honour.

## Related

- Root [`AGENTS.md`](../AGENTS.md) — short instructions for agents
- [agents.md](agents.md) — `rtok agents install` host matrix
- [release.md](release.md) — per-OS release targets and signing
- [getting-started.md](getting-started.md) — install entry points
- [colima.md](colima.md) — Linux/macOS Docker via Colima (not a Windows substitute)
