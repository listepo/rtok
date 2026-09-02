---
title: rtok
layout: hextra-home
---

{{< hextra/hero-badge link="https://github.com/listepo/rtok" >}}
  <div class="hx:w-2 hx:h-2 hx:rounded-full hx:bg-primary-400"></div>
  <span>P0 scaffold done — measurement first</span>
  {{< icon name="arrow-circle-right" attributes="height=14" >}}
{{< /hextra/hero-badge >}}

<div class="hx:mt-6 hx:mb-6">
{{< hextra/hero-headline >}}
  Token reduction for&nbsp;<br class="hx:sm:block hx:hidden" />AI coding agents
{{< /hextra/hero-headline >}}
</div>

<div class="hx:mb-12">
{{< hextra/hero-subtitle >}}
  One Rust binary. Every method is a plugin, every saving is a measured row,&nbsp;<br class="hx:sm:block hx:hidden" />and everything shortened can be expanded back.
{{< /hextra/hero-subtitle >}}
</div>

<div class="hx:mb-6">
{{< hextra/hero-button text="Get started" link="docs/getting-started" >}}
</div>

<div class="hx:mt-6"></div>

{{< hextra/feature-grid >}}
  {{< hextra/feature-card
    title="One binary, three surfaces"
    subtitle="Claude Code hooks, an MCP server, and an `ANTHROPIC_BASE_URL` proxy hop — instead of a stack of overlapping tools."
    link="docs/reference/architecture"
  >}}
  {{< hextra/feature-card
    title="Measured, not claimed"
    subtitle="A saving that is not a `Measurement` row does not exist. `rtok stats` reports what actually changed."
    link="docs/plugins/measure"
  >}}
  {{< hextra/feature-card
    title="Lossless by default"
    subtitle="Anything shortened is archived on disk and retrievable with `rtok expand <id>`."
    link="docs/plugins/archive"
  >}}
  {{< hextra/feature-card
    title="Fails open"
    subtitle="A hook exits 0 in under 10 ms even on error, with unmodified input. A half-installed rtok never blocks the host."
  >}}
  {{< hextra/feature-card
    title="One config file"
    subtitle="Every CLI flag is a config key. `rtok config show --sources` says where each value came from."
    link="docs/reference/configuration"
  >}}
  {{< hextra/feature-card
    title="Write your own plugin"
    subtitle="Public `Plugin` trait, in-tree modules behind Cargo features. No daemon, no subprocesses, no WASM."
    link="docs/reference/plugin-authoring"
  >}}
{{< /hextra/feature-grid >}}
