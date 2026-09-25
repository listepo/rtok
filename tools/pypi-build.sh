#!/usr/bin/env bash
# Builds the PyPI distributions into target/pypi/dist: a wheel for the host (or --target) and,
# with --sdist, the source distribution. maturin and twine run through uvx, so neither is a
# dependency of the repository; `twine check` validates the metadata at the end.
#
#   tools/pypi-build.sh                               # host wheel
#   tools/pypi-build.sh --sdist                       # host wheel + sdist
#   tools/pypi-build.sh --target x86_64-unknown-linux-gnu --zig   # cross, via maturin --zig
#
# One run builds one platform. A publish needs a wheel per platform in target/pypi/dist:
# build each on its OS (or cross with --zig) and collect them there; see docs/release.md.
set -euo pipefail

maturin_spec="maturin>=1.9,<2"
sdist=0
build_args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --sdist) sdist=1 ;;
    --target)
      [ $# -ge 2 ] || { echo "pypi-build: --target needs a triple" >&2; exit 2; }
      build_args+=(--target "$2")
      shift
      ;;
    --zig) build_args+=(--zig) ;;
    -h | --help)
      sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "pypi-build: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$(git rev-parse --show-toplevel)"
out=target/pypi
# <prefix>/share/rtok/{plugins,skills} after install; see PREFIX_SHARE_DIR in src/agents/mod.rs.
data="$out/rtok_cli.data/data/share/rtok"

rm -rf "$out/rtok_cli.data"
mkdir -p "$data" "$out/dist"
cp -R plugins skills "$data/"

uvx --from "$maturin_spec" maturin build --release --out "$out/dist" "${build_args[@]+"${build_args[@]}"}"
if [ "$sdist" -eq 1 ]; then
  uvx --from "$maturin_spec" maturin sdist --out "$out/dist"
fi
uvx twine check --strict "$out"/dist/*
ls -l "$out/dist"
