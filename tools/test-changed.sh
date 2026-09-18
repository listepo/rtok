#!/usr/bin/env bash
# `just test-changed [rev]` — build and run only the test targets the current change can reach.
#
# The suite has 41 integration targets, and `cargo nextest run` links every one of them before
# it runs a single test. Filtering with `-E` does not help: the filter is applied after the
# build. Only cargo's own target selection (`--lib`, `--test <name>`) skips the link, so this
# script's whole job is to turn a diff into that argument list.
#
# Selection, cheapest signal first:
#   tests/<name>.rs changed     -> --test <name>
#   a shared input changed      -> the whole suite (Cargo.toml, crates/, config/, justfile,
#                                  tests/common, fixtures, snapshots: too wide to map by name)
#   anything else changed       -> --lib, plus every tests/<name>.rs whose file name or body
#                                  mentions one of the changed path's segments
#
# The unit-test binary is then trimmed by name too (`-E 'kind(lib) & test(~seg)'`). Linking it is
# cheap, but running all of it is not: the TUI tests alone take ~75s of the ~76s wall clock.
#
# Markdown goes through the same name match rather than being ignored: `docs/agents.md` reaches
# the host-docs test, `plan.md` reaches the plugin-plan test, and a doc nothing asserts on
# selects nothing but the unit tests.
#
# This is an inner-loop accelerator, not a coverage proof: targets are picked by name, so a test
# that exercises a module without ever naming it is not selected. `just check` stays the gate.
#
# Usage: tools/test-changed.sh [rev]   (default rev: HEAD, i.e. the working tree)
set -euo pipefail

rev="${1:-HEAD}"
cargo="${CARGO:-cargo}"

# Path segments that name no module worth matching on: too generic to narrow anything.
stoplist='^(mod|lib|main|src|rs|md|tests|test|docs|doc|site|github|workflows|plugins|util|utils|common|core|types|impl|new)$'

# RTOK_CHANGED (newline-separated paths) replaces the git query, so the mapping can be
# exercised without staging a diff. Everything below it is the same code path.
changed="${RTOK_CHANGED-$(
  {
    git diff --name-only "$rev" --
    git ls-files --others --exclude-standard
  } | sort -u
)}"

if [[ -z "$changed" ]]; then
  echo "test-changed: no changes against $rev — running unit tests only"
  exec $cargo nextest run -p rtok --lib
fi

run_all=0
targets=()
tokens=()

while IFS= read -r f; do
  [[ -n "$f" ]] || continue
  case "$f" in
    tests/*/*) run_all=1 ;;                       # common/, fixtures/, snapshots/, trycmd/
    tests/*.rs) targets+=("$(basename "$f" .rs)") ;;
    Cargo.toml | Cargo.lock | justfile | mise.toml | crates/* | config/* | .config/* | .cargo/*)
      run_all=1
      ;;
    *)
      # src/plugins/graph/lsp.rs -> plugins graph lsp -> graph lsp
      while IFS= read -r seg; do
        [[ "$seg" =~ $stoplist ]] && continue
        ((${#seg} >= 3)) && tokens+=("$seg")
      done < <(echo "$f" | tr '/.' '\n\n')
      ;;
  esac
done <<<"$changed"

if ((run_all)); then
  echo "test-changed: a shared input changed — running the whole suite"
  exec $cargo nextest run --workspace
fi

# Every test file that names one of the touched modules, by file name or in its body.
if ((${#tokens[@]})); then
  pattern="$(printf '%s\n' "${tokens[@]}" | sort -u | paste -sd'|' -)"
  while IFS= read -r f; do
    targets+=("$(basename "$f" .rs)")
  done < <(grep -lwiE -- "$pattern" tests/*.rs || true)
  while IFS= read -r f; do
    targets+=("$(basename "$f" .rs)")
  done < <(ls tests/*.rs | grep -iE -- "$pattern" || true)
fi

args=(-p rtok)
((${#tokens[@]})) && args+=(--lib)
picked=0
while IFS= read -r t; do
  [[ -n "$t" ]] || continue
  args+=(--test "$t")
  picked=$((picked + 1))
done < <(printf '%s\n' "${targets[@]:-}" | sort -u)

# Past roughly three quarters of the suite the selection has stopped paying for itself:
# the remaining targets link in parallel anyway, and running all of them is the honest answer.
total=$(ls tests/*.rs | wc -l | tr -d ' ')
if ((picked * 4 >= total * 3)); then
  echo "test-changed: $picked of $total targets selected — running the whole suite instead"
  exec $cargo nextest run --workspace
fi

# Same name match again, this time inside the unit-test binary: it links in seconds but runs for
# over a minute, almost all of it in TUI tests that no source change outside src/tui can reach.
if ((${#tokens[@]})); then
  unit="$(printf 'test(~%s)|' $(printf '%s\n' "${tokens[@]}" | sort -u) | sed 's/|$//')"
  args+=(-E "kind(lib) & ($unit) | !kind(lib)")
  echo "test-changed: $picked of $total integration targets + the matching unit tests"
else
  echo "test-changed: $picked of $total integration targets, no unit tests"
fi

exec $cargo nextest run "${args[@]}"
