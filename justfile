# rtok — `just check` is the gate every task must pass (plan T0.7, D16).
# Tools are pinned in mise.toml; override CARGO/CLIFF/HUGO if mise is already activated.

cargo := env("CARGO", "mise exec -- cargo")
cache := env("CARGO_CACHE", "mise exec -- cargo-cache")
cliff := env("CLIFF", "mise exec -- git-cliff")
# cargo-dist is not in mise.toml (compiling it on every `mise install` is slow); mise fetches it on demand.
dist := env("DIST", "mise x cargo:cargo-dist@0.32.0 -- dist")
hugo := env("HUGO", "mise exec -- hugo --source site")
jscpd := env("JSCPD", "mise exec -- jscpd")
oxlint := env("OXLINT", "mise exec -- oxlint")
oxfmt := env("OXFMT", "mise exec -- oxfmt")
pytest := env("PYTEST", "mise exec -- pytest")

# Logical CPUs, portable across the OSes rtok's CI runs on (Linux/macOS/BSD, getconf fallback).
cpus := `case "$(uname -s)" in Linux) nproc;; Darwin|*BSD) sysctl -n hw.ncpu;; *) getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4;; esac`

default: check

# fmt --check, clippy -D warnings, tests, min-feature build, copy-paste detector, JS/TS lint+format, Python tests
check: fmt-check lint test build-min dup js python

fmt:
    {{cargo}} fmt

fmt-check:
    {{cargo}} fmt --check

# rtok-wasm-demo-guest is no_std cdylib for wasm32; host `--all-targets` cannot
# compile its lib (unwind without std). Lint it as `(lib test)` instead.
lint:
    {{cargo}} clippy --workspace --all-targets --all-features --exclude rtok-wasm-demo-guest -- -D warnings
    {{cargo}} clippy -p rtok-wasm-demo-guest --tests -- -D warnings

# T26.0: copy-paste detector. Config, paths and threshold live in `.jscpd.json`; jscpd exits
# non-zero past the threshold, which is what makes "don't duplicate logic" a gate and not a wish.
dup:
    {{jscpd}}

# T110: the TypeScript host plugins and tests/node. JS/TS files only — oxfmt would also
# rewrite the plugins' JSON manifests, which tests compare byte for byte.
js_files := `git ls-files '*.ts' '*.tsx' '*.js' '*.mjs' '*.cjs' | tr '\n' ' '`

js:
    {{oxlint}} --deny-warnings {{js_files}}
    {{oxfmt}} --check {{js_files}}

js-fmt:
    {{oxfmt}} {{js_files}}

# T183: tools/publish_marketplace's own test suite (no network, no real `gh`).
python:
    {{pytest}} tools/tests

# --workspace so `rtok-plugin-sdk` (the published contract, D25) is in the same gate.
# `-j` is the number of concurrent test threads; heavy tests in .config/nextest.toml
# reserve `num-test-threads`, which is this value.
test:
    {{cargo}} nextest run --workspace --test-threads {{cpus}}

# Inner loop: build and run only the test targets the current change can reach. `nextest -E`
# filters after the build, so the saving comes from cargo target selection (`--test <name>`);
# tools/test-changed.sh maps the diff onto it. Selection is by name, so this is an
# accelerator, not a coverage proof — `just check` stays the gate before a commit.
test-changed rev="HEAD":
    NEXTEST_TEST_THREADS="{{cpus}}" CARGO="{{cargo}}" tools/test-changed.sh {{rev}}

# T0.4: one plugin feature must build alone
build-min:
    {{cargo}} build -q --no-default-features --features measure

# plugin-authoring examples (hook plugin, MCP-tool plugin, and the same plugin written
# against the published contract alone)
example:
    {{cargo}} run -q --example hello_plugin
    {{cargo}} run -q --example mcp_tool
    {{cargo}} run -q -p rtok-plugin-sdk --example shrink

# T23.6: crates.io publish for rtok-plugin-sdk is paused; dry-run is a no-op until re-enabled.
# When publishing again: restore `publish = true` in the crate + release-plz, and this target.
publish-dry:
    @echo "rtok-plugin-sdk crates.io publish paused; skipping dry-run"

# T9.5: execute every README bash fence marked `# check`.
readme-check:
    python3 -c 'import re; from pathlib import Path; print("".join(block[len("# check\\n"):] for block in re.findall(r"```bash\\n(.*?)\\n```", Path("README.md").read_text(), re.S) if block.startswith("# check\\n")), end="")' | bash -euo pipefail

# T10.4 check: cargo-dist can plan a release from dist-workspace.toml
dist-plan:
    {{dist}} plan

# regenerate .github/workflows/release.yml from dist-workspace.toml, then map
# CODESIGN_* secret names to MACOS_* (see tools/dist-generate.sh).
dist-generate:
    DIST="{{dist}}" tools/dist-generate.sh

# T18.2: release the version in Cargo.toml, or the next one if that is already tagged.
# Same script the Bump workflow runs, so local and CI cannot disagree.
release level="patch" *flags:
    tools/release.sh {{level}} {{flags}}

# regenerate CHANGELOG.md from git history (git-cliff, config in cliff.toml)
changelog:
    {{cliff}} -o CHANGELOG.md

# build the docs site into site/public (fails on a broken link or missing mount)
site:
    {{hugo}} --minify --panicOnWarning

# docs site at http://localhost:1313 with live reload
site-serve:
    {{hugo}} server --buildDrafts

# Slint WASM UI, then API+UI on host:port (T60.7 profile + wasm-opt; T81 shares the script with CI)
web host="127.0.0.1" port="3333": web-bundle
    {{cargo}} run -q -- web --host {{host}} --port {{port}}

# Just the WASM bundle `rtok web` serves and the release archive carries (T81).
# Fails open without wasm-pack; CI runs the same script with --require.
web-bundle:
    tools/webui-bundle.sh --compress

# `crates/rtok-webui` is excluded from the workspace, so `just check` never compiles
# it — a wasm-only break reaches main unseen (T81 hit one). CI runs this.
webui-check:
    rustup target add wasm32-unknown-unknown
    {{cargo}} check --manifest-path crates/rtok-webui/Cargo.toml --target wasm32-unknown-unknown

# $CARGO_HOME sizes (no deletes) and ./target
cache:
    {{cache}}
    du -sh target 2>/dev/null || echo "target: (missing)"

# drop extracted crate/git checkouts; keep archives
cache-autoclean:
    {{cache}} --autoclean

# T53.4: Jaeger + Grafana on shifted ports; skips when Docker is unavailable.
otel-check:
    tools/otel-check.sh


# T119: the CodeQL scan of .github/workflows/codeql.yml, run locally on the tracked files
# (working-tree content, none of the ignored clutter). Not in `check`: it takes minutes.
# SARIF lands in target/codeql/<lang>.sarif; any result fails the recipe.
codeql *langs="actions javascript-typescript python rust":
    #!/usr/bin/env bash
    set -euo pipefail
    out=target/codeql
    rm -rf "$out/src" && mkdir -p "$out/src"
    git ls-files -z | tar --null -T - -cf - | tar -xf - -C "$out/src"
    fail=0
    for lang in {{langs}}; do
      pack=${lang%%-*}
      mise exec -- codeql database create "$out/db-$lang" --overwrite --quiet \
        --language="$lang" --build-mode=none --source-root="$out/src"
      mise exec -- codeql database analyze "$out/db-$lang" --download --quiet \
        "codeql/$pack-queries:codeql-suites/$pack-security-and-quality.qls" \
        --format=sarif-latest --sarif-category="/language:$lang" --output="$out/$lang.sarif"
      n=$(mise exec -- node -e 'const s=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));console.log(s.runs.reduce((a,r)=>a+r.results.length,0))' "$PWD/$out/$lang.sarif")
      echo "codeql $lang: $n result(s) → $out/$lang.sarif"
      [ "$n" = 0 ] || fail=1
    done
    exit $fail
