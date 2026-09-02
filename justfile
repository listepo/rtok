# rtok — `just check` is the gate every task must pass (plan T0.7, D16).
# Tools are pinned in mise.toml; override CARGO/CLIFF/HUGO if mise is already activated.

cargo := env("CARGO", "mise exec -- cargo")
cache := env("CARGO_CACHE", "mise exec -- cargo-cache")
cliff := env("CLIFF", "mise exec -- git-cliff")
# cargo-dist is not in mise.toml (compiling it on every `mise install` is slow); mise fetches it on demand.
dist := env("DIST", "mise x cargo:cargo-dist@0.32.0 -- dist")
hugo := env("HUGO", "mise exec -- hugo --source site")

default: check

# fmt --check, clippy -D warnings, tests, min-feature build
check: fmt-check lint test build-min

fmt:
    {{cargo}} fmt

fmt-check:
    {{cargo}} fmt --check

lint:
    {{cargo}} clippy --all-targets --all-features -- -D warnings

test:
    {{cargo}} test

# T0.4: one plugin feature must build alone
build-min:
    {{cargo}} build -q --no-default-features --features measure

# plugin-authoring examples (hook plugin, MCP-tool plugin)
example:
    {{cargo}} run -q --example hello_plugin
    {{cargo}} run -q --example mcp_tool

# T10.4 check: cargo-dist can plan a release from dist-workspace.toml
dist-plan:
    {{dist}} plan

# regenerate .github/workflows/release.yml from dist-workspace.toml
dist-generate:
    {{dist}} generate

# regenerate CHANGELOG.md from git history (git-cliff, config in cliff.toml)
changelog:
    {{cliff}} -o CHANGELOG.md

# build the docs site into site/public (fails on a broken link or missing mount)
site:
    {{hugo}} --minify --panicOnWarning

# docs site at http://localhost:1313 with live reload
site-serve:
    {{hugo}} server --buildDrafts

# $CARGO_HOME sizes (no deletes) and ./target
cache:
    {{cache}}
    du -sh target 2>/dev/null || echo "target: (missing)"

# drop extracted crate/git checkouts; keep archives
cache-autoclean:
    {{cache}} --autoclean
