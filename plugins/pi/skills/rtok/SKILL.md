# rtok pi skill

Use `rtok` to keep bash output small without losing anything.
This skill fires when bash output is large or repeated.

## What the extension already does

- Every `bash` call runs as `rtok run -- <command>`: raw output is
  archived, a filtered version is what the model sees.
- Every `bash` result passes through `rtok filter`: oversized output is
  compressed with an `expand <id>` trailer.

## Recovering full output

Full text of anything shortened is one call away:

```bash
rtok expand <id>
```

## If rtok is missing

Fail open and install with ketch:

```bash
ketch install listepo/rtok
```

Bootstrap ketch first if needed:

```bash
curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash
ketch install listepo/rtok
```

## Rules (D21)

- One call path only: never add `read`/`search` tools or a second bash
  rewrite — the extension owns the bash path, nothing else may duplicate it.
- Lossless by default: anything shortened stays retrievable via `expand`.
