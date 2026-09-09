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
**Status: v0.0.1.** All ten plugins are implemented across the three surfaces, with
measurement, config provenance, an OTLP export and a web dashboard. What is *not* done is
the number that matters: the committed A/B bench has only been run offline, so it reports
zeros for both configurations. rtok's claim today is "measurable", not "cheaper".
{{< /callout >}}

## Start here

{{< cards >}}
  {{< card link="getting-started" title="Getting started" subtitle="Install the binary and run your first command." icon="download" >}}
  {{< card link="commands" title="Commands" subtitle="Every subcommand and what it does today." icon="terminal" >}}
  {{< card link="plugins" title="Plugins" subtitle="The ten in-tree plugins and the surfaces they serve." icon="puzzle" >}}
  {{< card link="reference/configuration" title="Configuration" subtitle="One file, layered precedence, full key reference." icon="adjustments" >}}
  {{< card link="reference/comparison" title="Comparison" subtitle="Against rtk, lean-ctx, headroom, serena and the rest — with the evidence." icon="scale" >}}
{{< /cards >}}

## Why it exists

A typical Claude Code setup stacks several tools that each compress Bash output, cache
reads, and inject context at session start — each claiming 60–95 % savings that nobody
measures end to end. Measured, that stack saves 3–40 % and costs roughly 3 K injected
tokens per turn.

rtok replaces the stack with one binary that runs as one hook per event, serves eleven MCP
tools for ~143 description tokens per turn instead of ~8 600 across nine servers, sits as
one proxy hop that records real `usage`, and keeps one measurement table so `rtok stats` can
say what actually changed. The full tool-by-tool reading, including where rtok is behind, is
in [Comparison](reference/comparison).

Every plugin is written from scratch in this repo. rtok never runs, links, or reads the
tools it replaces.
