#!/usr/bin/env bash
# T18.2 (P18). The one place a release version is decided, so `just release` and
# .github/workflows/bump.yml cannot drift apart.
#
# The version in Cargo.toml is the version to release. It is raised only when that version is
# already tagged — which is what makes the first run publish 0.0.1 instead of skipping to 0.0.2.
# dist creates the tag and the GitHub Release itself (dispatch-releases in dist-workspace.toml),
# so this script's job ends at "the version commit is on the remote, the workflow is running".
#
# Usage: tools/release.sh [patch|minor|major] [--dry-run|--local]
#   --dry-run  print the version that would be released and change nothing
#   --local    make the version commit but neither push nor start the workflow
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
# The workflow reads this to know which tag to dispatch. Not a `&&` one-liner: when the variable
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
  # Run before the commit, so the release commit itself is never in the notes it generates.
  $CLIFF --tag "v$version" -o CHANGELOG.md
  git add Cargo.toml Cargo.lock CHANGELOG.md
  git commit -m "release: v$version"
fi

if [ "$mode" = "--local" ]; then
  echo "local: version commit made, not pushed"
  exit 0
fi

git push origin HEAD
gh workflow run release.yml --field tag="v$version"
echo "release v$version dispatched"
