#!/usr/bin/env bash
# Copyright (c) 2026 Ivan Tugay
# SPDX-License-Identifier: GPL-3.0-or-later
# Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

# T18.2 (P18). The one place a release version is decided, so `just release` and
# .github/workflows/bump.yml cannot drift apart.
#
# The version in Cargo.toml is the version to release. It is raised only when that version is
# already tagged — which is what makes the first run publish 0.0.1 instead of skipping to 0.0.2.
# This script never pushes and never tags: the Bump workflow (pyrlyn/infra bump.yml) runs it with
# --local, opens a PR with the version commit, rebase-merges it once the required checks pass,
# then tags the landed commit and dispatches the dist Release (docs/release.md).
#
# Usage: tools/release.sh [patch|minor|major] [--dry-run|--local]
#   (none)     start the Bump workflow on GitHub (gh workflow run bump.yml -f level=<level>)
#   --dry-run  print the version that would be released and change nothing
#   --local    make the version commit and stop (what bump.yml runs)
set -euo pipefail

cd "$(dirname "$0")/.."

level="${1:-patch}"
mode="${2:-}"
case "$level" in
  patch | minor | major) ;;
  *)
    echo "level must be patch, minor or major (got '$level')" >&2
    exit 2
    ;;
esac

# Same override convention as the justfile: tools come from mise unless the caller says otherwise.
CARGO="${CARGO:-mise exec -- cargo}"
CLIFF="${CLIFF:-mise exec -- git-cliff}"

case "$mode" in
  "")
    # The only way to a release: the Bump workflow (PR, required checks, merge, then the tag).
    gh workflow run bump.yml -f level="$level"
    echo "Bump and release ($level) started: gh run list --workflow bump.yml"
    exit 0
    ;;
  --dry-run | --local) ;;
  *)
    echo "unknown option: $mode" >&2
    exit 2
    ;;
esac

current=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
version="$current"

if git rev-parse -q --verify "refs/tags/v$current" >/dev/null; then
  IFS=. read -r major minor patch <<<"$current"
  case "$level" in
    major) version="$((major + 1)).0.0" ;;
    minor) version="$major.$((minor + 1)).0" ;;
    patch) version="$major.$minor.$((patch + 1))" ;;
  esac
fi

echo "current $current -> release v$version"
# The workflow reads this to know which tag to make. Not a `&&` one-liner: when the variable
# is unset the test fails, and under `set -e` a failing top-level list ends the script.
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "version=$version" >>"$GITHUB_OUTPUT"
fi

if [ "$mode" = "--dry-run" ]; then
  echo "dry run: nothing written"
  exit 0
fi

if [ "$version" != "$current" ]; then
  # -i.bak keeps this working on BSD sed (macOS) as well as GNU.
  sed -i.bak "s|^version = \".*\"|version = \"$version\"|" Cargo.toml && rm -f Cargo.toml.bak
  # Cargo.lock carries the package's own version too; -w touches workspace members only.
  $CARGO update --workspace --quiet
  # T279: every plugin manifest and .rtok-plugin-version file moves with Cargo.toml, in the
  # same commit, so a host that caches a plugin by its manifest version sees a new build.
  tools/plugin-versions.sh --set "$version"
  # Run before the commit, so the release commit itself is never in the notes it generates.
  $CLIFF --tag "v$version" -o CHANGELOG.md
  # T279: the file list lives once, in tools/plugin-versions.sh --files, not copied here too.
  # shellcheck disable=SC2046 # --files prints repo-root-relative paths with no spaces.
  git add Cargo.toml Cargo.lock CHANGELOG.md $(tools/plugin-versions.sh --files)
  git commit -m "release: v$version"
fi

echo "local: version commit made (if any), not pushed, not tagged"
