#!/usr/bin/env bash
# Build the Slint WASM bundle into crates/rtok-webui/pkg — the directory `rtok web`
# serves (T80) and the release archive carries (T81: `include` in Cargo.toml's
# [package.metadata.dist]). One script so `just web` and .github/build-setup.yml
# cannot drift apart.
#
# Flags:
#   --require   turn every skip into a failure (CI: an archive without the bundle is
#               the bug T81 closed). Without it a dev lacking wasm-pack still gets the
#               API, and `rtok web` says the bundle is missing.
#   --compress  also write the .br/.gz `rtok web` negotiates (T60.7). `just web` passes
#               it; the release archive does not carry them, since it serves loopback.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

require=0
compress=0
for arg in "$@"; do
  case "$arg" in
    --require) require=1 ;;
    --compress) compress=1 ;;
    *) echo "webui-bundle: unknown flag $arg" >&2; exit 2 ;;
  esac
done

wasm="crates/rtok-webui/pkg/rtok_webui_bg.wasm"
# Same number as WASM_SIZE_GATE in tests/web_wasm.rs (T60.7, measured in research.md).
gate=4500000

skip() {
  if [ "$require" -eq 1 ]; then
    echo "webui-bundle: $1" >&2
    exit 1
  fi
  echo "webui-bundle: $1; serving API only" >&2
  exit 0
}

command -v wasm-pack >/dev/null 2>&1 || skip "wasm-pack not on PATH"

wasm-pack build crates/rtok-webui --release --target web --out-dir pkg

# wasm-pack runs wasm-opt itself when binaryen is reachable (measured 4,392,425 B);
# an explicit -Oz shaves a little more (4,130,017 B) when binaryen is installed here.
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz "$wasm" -o "$wasm"
fi

if [ "$compress" -eq 1 ]; then
  # `set -e` would take a bare `cmd -v x && x ...` as the script's verdict.
  if command -v brotli >/dev/null 2>&1; then brotli -f -k "$wasm"; fi
  if command -v gzip >/dev/null 2>&1; then gzip -kf "$wasm"; fi
fi

bytes=$(wc -c < "$wasm" | tr -d ' ')
echo "webui-bundle: $wasm is $bytes bytes (gate $gate)"
if [ "$bytes" -gt "$gate" ]; then
  # Reached when wasm-opt ran nowhere: the unoptimised bundle is ~10.5 MB, and
  # shipping that in every archive is not a thing to discover after a release.
  echo "webui-bundle: over the T60.7 gate — is wasm-opt/binaryen reachable?" >&2
  exit 1
fi
