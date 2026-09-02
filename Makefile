# rtok — `make check` is the gate every task must pass (plan T0.7).
# Rust is pinned in mise.toml; override CARGO if mise is already activated.
CARGO ?= mise exec -- cargo
CLIFF ?= mise exec -- git-cliff
# cargo-dist is not in mise.toml (compiling it on every `mise install` is slow); mise fetches it on demand.
DIST ?= mise x cargo:cargo-dist@0.32.0 -- dist

.PHONY: check fmt fmt-check lint test build-min example changelog dist-plan dist-generate

check: fmt-check lint test build-min ## fmt --check, clippy -D warnings, tests, min-feature build

fmt: ## rewrite sources with rustfmt
	$(CARGO) fmt

fmt-check:
	$(CARGO) fmt --check

lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

test:
	$(CARGO) test

build-min: ## T0.4 check: one plugin feature must build alone
	$(CARGO) build -q --no-default-features --features measure

example: ## run the plugin-authoring examples (hook plugin, MCP-tool plugin)
	$(CARGO) run -q --example hello_plugin
	$(CARGO) run -q --example mcp_tool

dist-plan: ## T10.4 check: cargo-dist can plan a release from dist-workspace.toml
	$(DIST) plan

dist-generate: ## regenerate .github/workflows/release.yml from dist-workspace.toml
	$(DIST) generate

changelog: ## regenerate CHANGELOG.md from git history (git-cliff, config in cliff.toml)
	$(CLIFF) -o CHANGELOG.md
