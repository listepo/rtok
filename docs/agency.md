# Agency: cross-platform product

This is the main **agency** text for coding agents working on rtok.

## Platforms

**rtok is a cross-platform product.** The supported platforms are exactly
**Windows**, **macOS**, and **Linux**. It **must** keep supporting all three
as first-class targets.

Do **not** add, document, CI, or release for any other platform. Do not mention
other operating systems as supported, planned, or optional.

Treat every change as if it will run on Windows, macOS, and Linux:

- Paths, env vars, shells, line endings, and install layouts differ across
  these three; do not assume Unix-only habits (`~/.…`, `sh`, symlinks,
  `chmod`) without a Windows path.
- Hooks, MCP, and `rtok proxy` are the same surfaces on Windows, macOS, and
  Linux; host install paths and GUI reopen behaviour still need coverage on
  all three (see [agents.md](agents.md)).
- Release ships `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and
  `x86_64-unknown-linux-gnu` (see [release.md](release.md)). Prefer fixes that
  keep that matrix green; do not drop or soft-deprecate Windows, macOS, or
  Linux without an explicit user decision.
- CI, docs, scripts, and examples that only work on a subset of these three
  are incomplete until the missing one(s) among Windows / macOS / Linux are
  addressed (or the gap is documented as a known limit with a tracked
  follow-up).

## What this is not

This file is **not** the host × module matrix ([agents.md](agents.md)) and not
a substitute for root `AGENTS.md` (workflow, Checks, fail-open rules). It
states the platform contract those agents must honour.

## Related

- Root [`AGENTS.md`](../AGENTS.md) — short instructions for agents
- [agents.md](agents.md) — `rtok agents install` host matrix
- [release.md](release.md) — Windows / macOS / Linux release targets and signing
- [getting-started.md](getting-started.md) — install entry points
- [colima.md](colima.md) — Docker via Colima on macOS and Linux (not for Windows)
