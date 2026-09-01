# rtok — `make check` is the gate every task must pass (plan T0.7).
# Rust is pinned in mise.toml; override CARGO if mise is already activated.
CARGO ?= mise exec -- cargo
CLIFF ?= mise exec -- git-cliff

.PHONY: check fmt fmt-check lint test build-min example changelog

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

example: ## run the plugin-authoring example
	$(CARGO) run -q --example hello_plugin

changelog: ## regenerate CHANGELOG.md from git history (git-cliff, config in cliff.toml)
	$(CLIFF) -o CHANGELOG.md
