# Toolchain

Project programs and direct packages from the manifests.

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| mise | brew / curl, then `mise install` | Pinned tool versions | https://github.com/jdx/mise |
| cargo-cache | mise | `just cache` / `just cache-autoclean` (T17.2); the shared cargo home fills up | https://github.com/matthiaskrgr/cargo-cache |
| binaryen | brew (optional) | wasm-opt for the T60.7 webui bundle; wasm-pack downloads its own when it is not on PATH, with the same flags from `crates/rtok-webui/Cargo.toml` | https://github.com/WebAssembly/binaryen |
| twiggy | optional (cargo install) | Per-function and per-crate size of the webui wasm when `tests/web_wasm.rs` reports growth (research.md, T60.7) | https://github.com/AlexEne/twiggy |
| cargo-nextest | global (cargo install) | Parallel test runner | https://github.com/nextest-rs/nextest |
| cargo-llvm-cov | mise | `just test-cov` (T301): coverage over the nextest suite; lcov for SonarCloud | https://github.com/taiki-e/cargo-llvm-cov |
| cargo-fuzz | global (`cargo install cargo-fuzz`) + `rustup toolchain install nightly` | `just fuzz` / `fuzz/README.md`: libFuzzer targets; nightly only for the fuzz build, the pinned toolchain is untouched | https://github.com/rust-fuzz/cargo-fuzz |
| codeql | mise | `just codeql` (T119): local run of the code scanning in `.github/workflows/codeql.yml` | https://github.com/github/codeql-cli-binaries |
| git-cliff | mise | Changelog | https://github.com/orhun/git-cliff |
| go | mise | hugo resolves the hextra theme as a Go module (site/go.mod) | https://github.com/golang/go |
| hugo | mise | Documentation site | https://github.com/gohugoio/hugo |
| just | mise | Command recipes | https://github.com/casey/just |
| ketch | see its README | Installs swarfr | https://github.com/pyrlyn/ketch |
| swarfr | ketch | `just test` / `just test-changed` end with a lossless cleanup of `target/` (T236) | https://github.com/listepo/swarfr |
| tailspin | mise (`ubi:bensadeh/tailspin`) | `just logs` (T225): `tspin` highlights `~/.rtok/logs/rtok.log` and the `RUST_LOG` stderr stream; a viewer, not a logger | https://github.com/bensadeh/tailspin |
| node | mise | jscpd, oxlint, oxfmt and vitest run on it; nothing in the binary does. Its `npm` packs and publishes the npm package (`just npm-build` / `just npm-publish`) | https://github.com/nodejs/node |
| gh | brew | `just npm-build --release vX.Y.Z` downloads the dist Release archives | https://github.com/cli/cli |
| uv | brew / curl | `uvx` runs maturin and twine for `just pypi-build` / `just pypi-publish`; neither is pinned in the repo | https://github.com/astral-sh/uv |
| maturin | uvx (`maturin>=1.9,<2`) | Builds the `rtok-cli` wheels (`bindings = "bin"`, pyproject.toml) and the sdist | https://github.com/PyO3/maturin |
| twine | uvx or `uv tool install twine` | `twine check` and the manual PyPI/TestPyPI upload in `tools/pypi-publish.sh` | https://github.com/pypa/twine |
| python | mise | T183: `tools/publish_marketplace` and its tests | https://github.com/python/cpython |
| pytest | mise (`pipx:pytest`) | T183: `just python` runs `tools/tests` | https://github.com/pytest-dev/pytest |
| jscpd | mise | `just dup` (T26.0): copy-paste detector, config in .jscpd.json | https://github.com/kucherenko/jscpd |
| oxlint | mise (`npm:oxlint`) | `just js` (T110): lint for the TS host plugins and tests/node, `--deny-warnings` | https://github.com/oxc-project/oxc |
| oxfmt | mise (`npm:oxfmt`) | `just js` / `just js-fmt` (T110): formatter for the same JS/TS files | https://github.com/oxc-project/oxc |
| vitest | mise (`npm:vitest`) | T111: runs the TS host plugin tests (`vitest.config.mjs`, globals, inline snapshots); driven by `tests/filter.rs` and `tests/pi_plugin.rs` (`--bail=1`), on Linux only (skipped on macOS and Windows) | https://github.com/vitest-dev/vitest |
| vite | mise (`npm:vite`) | Peer of vitest 5 (`@vitest/mocker`); required so Windows CI can resolve `vite` when running host plugin tests | https://github.com/vitejs/vite |
| rust | mise | The one Rust version of the repo: local, ci.yml, verify.yml, sonarcloud.yml and release builds (build-setup.yml) all install it from mise.toml; `rustfmt,clippy` because CI otherwise installs the minimal profile | https://github.com/rust-lang/rust |
| rustc | mise (pin rust) | Rust compiler | https://github.com/rust-lang/rust |
| cargo | mise (pin rust) | Rust build and dependencies | https://github.com/rust-lang/cargo |
| colima | mise | Container runtime that runs Docker (and others) inside a Lima VM — lightweight alternative to Docker Desktop for agents | https://github.com/abiosoft/colima |
| lima | mise | Linux VM Colima drives; install via mise, usually started only through `colima start` | https://github.com/lima-vm/lima |
| docker-cli | mise | `docker` client; points at Colima's Docker context when Colima is running | https://github.com/docker/cli |
| docker-compose | mise | Standalone `docker-compose` against the same Colima daemon (~Docker-compatible; not Podman) | https://github.com/docker/compose |

## ketch

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| swarfr | global | https://github.com/listepo/swarfr | Lossless `target/` cleanup after tests |

## npm (web/)

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| react | local | https://github.com/facebook/react | T310.1: the admin SPA UI |
| react-dom | local | https://github.com/facebook/react | T310.1: React DOM renderer |
| vite | local | https://github.com/vitejs/vite | T310.1: SPA dev server + build (`just spa-dev` / `just spa-build`) |
| @vitejs/plugin-react | local | https://github.com/vitejs/vite-plugin-react | T310.1: React fast refresh + JSX transform for Vite |
| json-schema-to-typescript | local | https://github.com/bcherny/json-schema-to-typescript | T310.2: `web/src/api/snapshot.gen.ts` from the `/ws` JSON Schema (`npm run gen:api`) |
| @tanstack/react-query | local | https://github.com/TanStack/query | T310.3: query cache the `/ws` snapshot is pushed into, plus the `set` / `expand` mutations |
| vitest | local | https://github.com/vitest-dev/vitest | T310.3: web/src unit tests (`just spa-test`), run on web's own `vite.config.ts`, not the root `vitest.config.mjs` |
| typescript | local | https://github.com/microsoft/TypeScript | T310.1: strict typecheck (`just spa-typecheck`) |
| @types/react | local | https://github.com/DefinitelyTyped/DefinitelyTyped | T310.1: React types |
| @types/react-dom | local | https://github.com/DefinitelyTyped/DefinitelyTyped | T310.1: React DOM types |
| tailwindcss | local | https://github.com/tailwindlabs/tailwindcss | T310.1: utility CSS, themed from the design tokens |
| @tailwindcss/vite | local | https://github.com/tailwindlabs/tailwindcss | T310.1: Tailwind v4 Vite plugin |
| @tanstack/react-router | local | https://github.com/TanStack/router | T310.4: code-based route tree built from the page list |
| @tanstack/react-table | local | https://github.com/TanStack/table | T310.5: headless model for `DataTable` (v9: `useTable`, `tableFeatures`) |
| @tanstack/react-virtual | local | https://github.com/TanStack/virtual | T310.5: windowed rows for `DataTable` |
| storybook | local | https://github.com/storybookjs/storybook | T310.5: UI kit workshop and static build (`just spa-storybook`) |
| @storybook/react-vite | local | https://github.com/storybookjs/storybook | T310.5: Storybook framework for React on Vite |
| @storybook/addon-vitest | local | https://github.com/storybookjs/storybook | T310.5: runs every story as a Vitest browser test (`just spa-stories`) |
| @storybook/addon-a11y | local | https://github.com/storybookjs/storybook | T310.5: axe checks on every story; a violation fails the run |
| @vitest/browser | local | https://github.com/vitest-dev/vitest | T310.5: browser mode for the story tests |
| @vitest/browser-playwright | local | https://github.com/vitest-dev/vitest | T310.5: Playwright provider for browser mode |
| playwright | local | https://github.com/microsoft/playwright | T310.5: drives Chromium for the story tests |
| @playwright/test | local | https://github.com/microsoft/playwright | T310.10: runner for the e2e suite against the real `rtok web` binary (`just spa-e2e`) |
| @types/node | local | https://github.com/DefinitelyTyped/DefinitelyTyped | T310.10: Node types for the e2e fixture (`node:child_process`, `node:fs`) |
| @testing-library/react | local | https://github.com/testing-library/react-testing-library | T310.4: renders the shell and routes in Vitest |
| @testing-library/dom | local | https://github.com/testing-library/dom-testing-library | T310.4: peer of @testing-library/react (queries, events) |
| happy-dom | local | https://github.com/capricorn86/happy-dom | T310.4: DOM for component tests (`// @vitest-environment happy-dom`, faster than jsdom) |
| three | local | https://github.com/mrdoob/three.js | T329.13: WebGL scene of the projects overview (lazy chunk `Scene3D`, 575 kB, 143 kB gzip; the first bundle does not grow) |
| d3-force-3d | local | https://github.com/vasturiano/d3-force-3d | T329.13: force layout run in a web worker (`layout.worker`, 29 kB); `3d-force-graph` would simulate on the main thread, and this is the engine it uses. Last push 2025-04-09, so watch its upkeep |
| @types/three | local | https://github.com/DefinitelyTyped/DefinitelyTyped | T329.13: Three.js types |

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| anyhow | local | https://crates.io/crates/anyhow | CLI errors |
| arbitrary | local | https://crates.io/crates/arbitrary | `fuzz/`: structured fuzz inputs (argv, hook/rules bodies) |
| assert_cmd | local | https://crates.io/crates/assert_cmd | CLI e2e tests |
| axum | local | https://crates.io/crates/axum | Rust dependency |
| clap | local | https://crates.io/crates/clap | CLI |
| clap_complete | local | https://crates.io/crates/clap_complete | `rtok completions` shell scripts (T53.2) |
| clap_mangen | local | https://crates.io/crates/clap_mangen | `rtok man` roff page (T53.2) |
| console_error_panic_hook | local | https://crates.io/crates/console_error_panic_hook | WASM panic hook for the Slint web UI (T222) |
| crossterm | local | https://crates.io/crates/crossterm | Terminal |
| diesel | local | https://crates.io/crates/diesel | SQLite ORM |
| diesel_migrations | local | https://crates.io/crates/diesel_migrations | Embedded SQLite migrations (T163.4) |
| divan | local | https://crates.io/crates/divan | Divan benches in benches/ |
| dotenvy | local | https://crates.io/crates/dotenvy | Rust dependency |
| dunce | local | https://crates.io/crates/dunce | Canonicalize without Windows UNC prefixes |
| env_logger | local | https://crates.io/crates/env_logger | T225: `RUST_LOG` debug log on stderr, off by default |
| figment | local | https://crates.io/crates/figment | Config |
| futures-util | local | https://crates.io/crates/futures-util | Rust dependency |
| globset | local | https://crates.io/crates/globset | T329.7: workspace member globs of project references; already in the tree through `ignore` |
| httpmock | local | https://crates.io/crates/httpmock | Rust dependency |
| humantime | local | https://crates.io/crates/humantime | T282: `[agents] idle` duration parsing; already in the lock as a transitive dep |
| i-slint-backend-testing | local | https://crates.io/crates/i-slint-backend-testing | Headless backend for Slint UI e2e tests |
| ignore | local | https://crates.io/crates/ignore | Rust dependency |
| indicatif | local | https://crates.io/crates/indicatif | Rust dependency |
| insta | local | https://crates.io/crates/insta | Snapshot tests for stable text output |
| jiff | local | https://crates.io/crates/jiff | T358: IANA time zones with DST for `rtok agents usage` day and month buckets (`--tz`); already in the lock as a transitive dep of env_logger |
| js-sys | local | https://crates.io/crates/js-sys | JS bindings for the Slint web UI (T222) |
| jsonc-parser | local | https://crates.io/crates/jsonc-parser | JSONC parse for Zed settings (T222) |
| libfuzzer-sys | local | https://crates.io/crates/libfuzzer-sys | `fuzz/`: libFuzzer runtime for the cargo-fuzz targets |
| libsqlite3-sys | local | https://crates.io/crates/libsqlite3-sys | Rust dependency |
| log | local | https://crates.io/crates/log | T225: logging facade env_logger drains; D26 lines are mirrored into it |
| mime_guess | local | https://crates.io/crates/mime_guess | Content-Type for the SPA files `rtok web` serves |
| notify | local | https://crates.io/crates/notify | Rust dependency |
| owo-colors | local | https://crates.io/crates/owo-colors | Rust dependency |
| pathdiff | local | https://crates.io/crates/pathdiff | Relative path between two paths |
| printpdf | local | https://crates.io/crates/printpdf | Rust dependency |
| ratatui | local | https://crates.io/crates/ratatui | TUI |
| regex | local | https://crates.io/crates/regex | Rust dependency |
| reqwest | local | https://crates.io/crates/reqwest | HTTP |
| rmcp | local | https://crates.io/crates/rmcp | Rust dependency |
| rstest | local | https://crates.io/crates/rstest | Rust dependency |
| rust-embed | local | https://crates.io/crates/rust-embed | Embeds the built SPA (`web/dist`) in the binary in every profile; memory-serve reads disk in debug builds and has no run-time override, include_dir has no media types or digests |
| rustix | local | https://crates.io/crates/rustix | Rust dependency |
| rustls | local | https://crates.io/crates/rustls | Preconfigured webpki TLS client config (T53.3) |
| rustls-pemfile | local | https://crates.io/crates/rustls-pemfile | `SSL_CERT_FILE` bundle parsing (T53.3) |
| semver | local | https://crates.io/crates/semver | T279: plugin version compare + `.rtok-plugin-version` (de)serialization |
| serde | local | https://crates.io/crates/serde | Serialization |
| schemars | local | https://github.com/GREsau/schemars | T310.2: JSON Schema of the `/ws` protocol, committed as `web/src/api/ws.schema.json` |
| serde_json | local | https://crates.io/crates/serde_json | JSON |
| serde-saphyr | local | https://crates.io/crates/serde-saphyr | T329.7: `pnpm-workspace.yaml` of project references; maintained (serde_yaml is deprecated) |
| sha2 | local | https://crates.io/crates/sha2 | Rust dependency |
| similar | local | https://crates.io/crates/similar | Rust dependency |
| slint | local | https://crates.io/crates/slint | Rust dependency |
| slint-build | local | https://crates.io/crates/slint-build | Rust dependency |
| tokio | local | https://crates.io/crates/tokio | Async runtime |
| toml_edit | local | https://crates.io/crates/toml_edit | Rust dependency |
| tree-sitter | local | https://crates.io/crates/tree-sitter | Rust dependency |
| tree-sitter-c | local | https://crates.io/crates/tree-sitter-c | Rust dependency |
| tree-sitter-c-sharp | local | https://crates.io/crates/tree-sitter-c-sharp | C# grammar tags (T52.2) |
| tree-sitter-dart | local | https://crates.io/crates/tree-sitter-dart | Rust dependency |
| tree-sitter-go | local | https://crates.io/crates/tree-sitter-go | Rust dependency |
| tree-sitter-java | local | https://crates.io/crates/tree-sitter-java | Java grammar tags (T52.2) |
| tree-sitter-javascript | local | https://crates.io/crates/tree-sitter-javascript | Rust dependency |
| tree-sitter-kotlin-ng | local | https://crates.io/crates/tree-sitter-kotlin-ng | Kotlin grammar tags (T52.2) |
| tree-sitter-php | local | https://crates.io/crates/tree-sitter-php | PHP grammar tags (T52.2) |
| tree-sitter-python | local | https://crates.io/crates/tree-sitter-python | Rust dependency |
| tree-sitter-ruby | local | https://crates.io/crates/tree-sitter-ruby | Ruby grammar tags (T52.2) |
| tree-sitter-rust | local | https://crates.io/crates/tree-sitter-rust | Rust dependency |
| tree-sitter-swift | local | https://crates.io/crates/tree-sitter-swift | Swift grammar tags (T52.2) |
| tree-sitter-tags | local | https://crates.io/crates/tree-sitter-tags | Rust dependency |
| tree-sitter-typescript | local | https://crates.io/crates/tree-sitter-typescript | Rust dependency |
| trycmd | local | https://crates.io/crates/trycmd | Full CLI command-output fixtures in tests/trycmd/ |
| pulldown-cmark | local (dev) | https://crates.io/crates/pulldown-cmark | CommonMark parse of README and docs in tests/docs_structure.rs: unclosed fences, skipped heading levels (T359) |
| tokio-tungstenite | local | https://crates.io/crates/tokio-tungstenite | WebSocket client for the `rtok web` e2e (tests/web_e2e.rs) |
| url | local | https://crates.io/crates/url | `file://` MCP roots → path (T263) |
| uuid | local | https://crates.io/crates/uuid | T282: random UUIDv4 rtok agent id (D34); already in the lock as a transitive dep |
| wasm-bindgen | local | https://crates.io/crates/wasm-bindgen | JS glue for the Slint web UI (T222) |
| wasm-bindgen-futures | local | https://crates.io/crates/wasm-bindgen-futures | JS futures for the Slint web UI (T222) |
| wasmi | local | https://crates.io/crates/wasmi | Rust dependency |
| web-sys | local | https://crates.io/crates/web-sys | Web APIs for the Slint web UI (T222) |
| webpki-roots | local | https://crates.io/crates/webpki-roots | Mozilla roots without the platform verifier (T53.3) |
| windows-sys | local | https://crates.io/crates/windows-sys | Windows process + file-lock shims in rtok-sys (T222) |
