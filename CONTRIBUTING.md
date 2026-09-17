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
