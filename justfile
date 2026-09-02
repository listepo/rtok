# rtok — `just check` is the gate every task must pass (plan T0.7, D16).
# Tools are pinned in mise.toml.

cargo := "mise exec -- cargo"
cache := "mise exec -- cargo-cache"
cliff := "mise exec -- git-cliff"

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

changelog:
    {{cliff}} -o CHANGELOG.md

# $CARGO_HOME sizes (no deletes) and ./target
cache:
    {{cache}}
    du -sh target 2>/dev/null || echo "target: (missing)"

# drop extracted crate/git checkouts; keep archives
cache-autoclean:
    {{cache}} --autoclean
