---
title: rtok
layout: hextra-home
---

{{< hextra/hero-badge link="https://github.com/listepo/rtok/releases" >}}
  <div class="hx:w-2 hx:h-2 hx:rounded-full hx:bg-primary-400"></div>
  <span>Δtok · measured, not claimed</span>
  {{< icon name="arrow-circle-right" attributes="height=14" >}}
{{< /hextra/hero-badge >}}

<div class="hx:mt-6 hx:mb-6">
{{< hextra/hero-headline >}}
  Token reduction for&nbsp;<br class="hx:sm:block hx:hidden" />AI coding agents
{{< /hextra/hero-headline >}}
</div>

<div class="hx:mb-8">
{{< hextra/hero-subtitle >}}
  One Rust binary. Hooks, MCP, and a proxy hop — every saving is a ledger row,&nbsp;<br class="hx:sm:block hx:hidden" />and everything shortened expands back by id.
{{< /hextra/hero-subtitle >}}
</div>

<div class="hx:mb-10 hx:flex hx:flex-wrap hx:gap-3">
{{< hextra/hero-button text="Get started" link="docs/getting-started" >}}
</div>

<div class="rtok-hero-shot hx:mb-16">
  <picture>
    <source media="(prefers-color-scheme: light)" srcset="images/hero-overview-light.png">
    <img src="images/hero-overview-dark.png" alt="rtok webui Overview — measured token ledger" width="1280" height="720">
  </picture>
</div>

{{< hextra/feature-grid >}}
  {{< hextra/feature-card
    title="Measured, not claimed"
    subtitle="If it is not a `Measurement` row, it did not happen. `rtok stats` shows the Δ — tokens before, after, and why."
    icon="rtok-measure"
    link="docs/plugins/measure"
  >}}
  {{< hextra/feature-card
    title="One binary, three surfaces"
    subtitle="Claude Code hooks, an MCP server, and an `ANTHROPIC_BASE_URL` proxy — one process, one ledger, no tool sprawl."
    icon="rtok-calls"
    link="docs/reference/architecture"
  >}}
  {{< hextra/feature-card
    title="Lossless by default"
    subtitle="Shortened context is archived on disk. Bring it back with `rtok expand <id>` when the agent needs the full payload."
    icon="rtok-sessions"
    link="docs/plugins/archive"
  >}}
  {{< hextra/feature-card
    title="Fails open"
    subtitle="Hooks exit 0 in under 10 ms even on error, with input unchanged. A half-installed rtok never blocks the host."
    icon="rtok-doctor"
  >}}
  {{< hextra/feature-card
    title="Plugin toolkit"
    subtitle="Public `Plugin` trait and in-tree modules behind Cargo features. No daemon, no subprocesses, no WASM."
    icon="rtok-plugins"
    link="docs/reference/plugin-authoring"
  >}}
  {{< hextra/feature-card
    title="Signal-scope UI"
    subtitle="Dark/light webui: bitset mark, scope icons, Overview → Logs. See savings as a ledger, not a marketing claim."
    icon="rtok-savings"
    link="docs/getting-started"
  >}}
{{< /hextra/feature-grid >}}
