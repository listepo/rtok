#!/usr/bin/env bash
# Manual PyPI upload of what tools/pypi-build.sh collected in target/pypi/dist. No workflow
# runs this. twine comes from PATH, else through uvx.
#
#   tools/pypi-publish.sh --dry-run      # checks + `twine check`, uploads nothing
#   tools/pypi-publish.sh --testpypi     # upload to TestPyPI only (https://test.pypi.org)
#   tools/pypi-publish.sh                # upload to PyPI; needs a token (TWINE_PASSWORD / ~/.pypirc)
#
# Refuses when a file's version is not the `rtok` version in Cargo.toml, when the sdist is
# missing, or — for a PyPI upload — when a platform wheel is missing.
set -euo pipefail

mode=pypi
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) mode=dry-run ;;
    --testpypi) mode=testpypi ;;
    -h | --help)
      sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "pypi-publish: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$(git rev-parse --show-toplevel)"
dist=target/pypi/dist
dist_name=rtok_cli
# One wheel per dist target (dist-workspace.toml), matched by its platform tag.
platform_tags=("macosx_*_arm64" "manylinux_*_x86_64" "win_amd64")

version=$(cargo metadata --format-version 1 --no-deps --locked |
  node -e 'const m = JSON.parse(require("fs").readFileSync(0, "utf8"));
    console.log(m.packages.find((p) => p.name === "rtok").version)')

problems=()
shopt -s nullglob
files=("$dist"/*.whl "$dist"/*.tar.gz)
[ ${#files[@]} -gt 0 ] || problems+=("nothing in $dist; run tools/pypi-build.sh first")
for f in "${files[@]}"; do
  base=$(basename "$f")
  case "$base" in
    "${dist_name}-${version}-"*.whl | "${dist_name}-${version}.tar.gz") ;;
    *) problems+=("$base is not ${dist_name} ${version}") ;;
  esac
done
[ -f "$dist/${dist_name}-${version}.tar.gz" ] || problems+=("missing sdist ${dist_name}-${version}.tar.gz")
missing=()
for tag in "${platform_tags[@]}"; do
  matches=("$dist/${dist_name}-${version}"-*-${tag}.whl)
  [ ${#matches[@]} -gt 0 ] || missing+=("$tag")
done
shopt -u nullglob
if [ ${#missing[@]} -gt 0 ]; then
  if [ "$mode" = pypi ]; then
    problems+=("no wheel for: ${missing[*]}")
  else
    echo "note: no wheel for ${missing[*]} (a PyPI upload refuses until they are there)" >&2
  fi
fi
if [ ${#problems[@]} -gt 0 ]; then
  echo "pypi-publish: refusing to upload rtok-cli ${version}:" >&2
  printf '  - %s\n' "${problems[@]}" >&2
  exit 1
fi

if command -v twine >/dev/null 2>&1; then twine=(twine); else twine=(uvx twine); fi
"${twine[@]}" check --strict "${files[@]}"
case "$mode" in
  dry-run) echo "dry run: ${#files[@]} file(s) for ${dist_name} ${version} pass; nothing uploaded" ;;
  testpypi) "${twine[@]}" upload --repository-url https://test.pypi.org/legacy/ "${files[@]}" ;;
  pypi) "${twine[@]}" upload "${files[@]}" ;;
esac
