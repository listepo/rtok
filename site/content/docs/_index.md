---
title: Documentation
weight: 1
---

rtok is one Rust binary that reduces the tokens an AI coding agent burns. Every
token-reduction method is a plugin behind a single trait, and three surfaces reach those
plugins:

| Surface | Entry point |
|---------|-------------|
| Claude Code hooks | `rtok hook <event>` — stdin JSON in, stdout JSON out |
| MCP server | `rtok mcp` — `read`, `search`, `tree`, `expand`, `mem_*`, graph tools |
| API proxy | `rtok proxy` — `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL` hop |

{{< callout type="warning" >}}
**Status: P0 scaffold done.** Config, store, plugin registry, estimator, hook types and CI
are in place. No token has been saved yet — by design, measurement comes first. Nothing in
the repo has produced a measured saving; the first number lands in P1.
{{< /callout >}}

## Start here

{{< cards >}}
  {{< card link="getting-started" title="Getting started" subtitle="Install the binary and run your first command." icon="download" >}}
  {{< card link="commands" title="Commands" subtitle="Every subcommand and what it does today." icon="terminal" >}}
  {{< card link="plugins" title="Plugins" subtitle="The ten in-tree plugins and the surfaces they serve." icon="puzzle" >}}
  {{< card link="reference/configuration" title="Configuration" subtitle="One file, layered precedence, full key reference." icon="adjustments" >}}
{{< /cards >}}

## Why it exists

A typical Claude Code setup stacks several tools that each compress Bash output, cache
reads, and inject context at session start — each claiming 60–95 % savings that nobody
measures end to end. Measured, that stack saves 3–40 % and costs roughly 3 K injected
tokens per turn.

rtok replaces the stack with one binary that runs as one hook per event, serves five MCP
tools instead of seventy-eight, sits as one proxy hop that records real `usage`, and keeps
one measurement table so `rtok stats` can say what actually changed.

Every plugin is written from scratch in this repo. rtok never runs, links, or reads the
tools it replaces.
