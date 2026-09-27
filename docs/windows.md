# Developing rtok on Windows

What behaves differently from the CI contract when `just check` runs on a
Windows machine, and what to do about each. Found 2026-09-26 running the full
gate locally on Windows for the first time (T272–T274).

## The Windows CI job runs tests only

`ci.yml`'s `windows` matrix runs `cargo nextest run --workspace` (in two
shards); `fmt`, `clippy`, `dup`, `js` and `python` live on the Linux jobs.
A local `just check` on Windows is therefore stricter than what CI proves
about Windows, and it surfaces Windows-only lint debt that the Linux lint
jobs cannot see — code behind `#[cfg(windows)]` or `#[cfg(unix)]` is only
analyzed on the OS that compiles it in. T273 (`permissions_set_readonly_false`
in the `cfg(windows)` `clear_readonly`) and T274 (unix-shaped test code:
early-return bodies, unix-gated helpers) were exactly that class. If clippy
fails on Windows over code CI never lints this way, fix it — do not
`--allow` the recipe.

## Line endings: match the CI checkout

Git for Windows defaults to `core.autocrlf=true`, which checks the tree out
with CRLF. Golden tests compare against fixture bytes (`tests/cmd_golden/*.in`)
and fail on a CRLF tree — `apt.in` produced `ok 0\r\n` where `ok 0\n` was
expected. CI runners check out LF. Align the tree:

```bash
git config core.autocrlf false
git rm --cached -r . && git reset --hard HEAD
```

The repo content is LF; this only rewrites the working tree to what CI sees.

## dart on PATH makes one test run — and fail

`graph_lsp_gate::lsp_backend_outlines_dart_main` skips when `dart` is not on
PATH, so CI never runs it. On a machine with the Dart SDK installed it runs,
and on Windows dart 3.9 the analysis server answers
`File is not being analyzed` for the freshly written temp project — the
outline call fails before the assertions. Until that is fixed (roadmap,
Windows developer experience), exclude it locally:

```bash
mise exec -- cargo nextest run --workspace -E 'not test(lsp_backend_outlines_dart_main)'
```

## Antivirus warms up slowly to fresh binaries

After a tree-wide rebuild (which is what the CRLF realignment above triggers)
Defender deep-scans every freshly written `rtok.exe`/test binary on first
spawn. The `agents_install` tests spawn the binary many times and can hit
nextest's 181 s `terminate-after` while cold — the same tests pass on the
warm rerun. If two integration tests time out right after a big rebuild,
rerun before investigating; excluding `target\` from real-time scanning
removes the effect entirely (a machine-level decision, not a repo one).

## Toolchain

Everything comes through mise; `pipx:pytest` resolves on Windows only with
`uv` present (pinned in `mise.toml` — the pipx backend has no Windows build
in the mise registry). Run cargo as `mise exec -- cargo <cmd>`, never a
system toolchain (CONTRIBUTING.md).
