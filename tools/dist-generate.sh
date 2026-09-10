#!/usr/bin/env bash
# Regenerate .github/workflows/release.yml from dist-workspace.toml, then map
# dist's CODESIGN_* secret names to the MACOS_* secrets this repository uses
# (same names as listepo/ketch). CODESIGN_IDENTITY is not a secret: the
# github-build-setup step discovers it on macOS runners.
#
# Invoked by `just dist-generate`. Do not hand-edit release.yml; change
# dist-workspace.toml (or .github/build-setup.yml) and re-run this.
#
# allow-dirty = ["ci"] is set so `dist plan` / `dist build` accept the patched
# workflow. That same flag makes bare `dist generate` skip writing release.yml,
# so this script briefly clears it, generates, then restores the file.

set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

dist_bin="${DIST:-}"
if [ -z "$dist_bin" ]; then
  dist_bin="mise x cargo:cargo-dist@0.32.0 -- dist"
fi

cfg="dist-workspace.toml"
cfg_backup="$(mktemp)"
cp "$cfg" "$cfg_backup"
cleanup() { mv "$cfg_backup" "$cfg"; }
trap cleanup EXIT

# Drop allow-dirty for the generate pass so release.yml is rewritten.
python3 - "$cfg" <<'PY'
from pathlib import Path
import re
import sys
path = Path(sys.argv[1])
text = path.read_text()
# Remove allow-dirty lines (and a preceding comment line about the patch, if present).
text2 = re.sub(
    r"(?m)^(?:#.*post-patched.*\n)?allow-dirty\s*=\s*\[[^\]]*\]\s*\n",
    "",
    text,
    count=1,
)
path.write_text(text2)
PY

# shellcheck disable=SC2086
$dist_bin generate

# Restore config (with allow-dirty) before patching, so the working tree matches intent.
mv "$cfg_backup" "$cfg"
trap - EXIT

workflow=".github/workflows/release.yml"
if [ ! -f "$workflow" ]; then
  echo "expected $workflow after dist generate" >&2
  exit 1
fi

python3 - "$workflow" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()
original = text

replacements = [
    (
        "CODESIGN_CERTIFICATE: ${{ secrets.CODESIGN_CERTIFICATE }}",
        "CODESIGN_CERTIFICATE: ${{ secrets.MACOS_CERTIFICATE }}",
    ),
    (
        "CODESIGN_CERTIFICATE_PASSWORD: ${{ secrets.CODESIGN_CERTIFICATE_PASSWORD }}",
        "CODESIGN_CERTIFICATE_PASSWORD: ${{ secrets.MACOS_CERTIFICATE_PWD }}",
    ),
]

for old, new in replacements:
    if old not in text:
        if "CODESIGN_CERTIFICATE:" in text and "secrets.MACOS_CERTIFICATE" not in text:
            print(f"dist-generate patch: missing expected line:\n  {old}", file=sys.stderr)
            sys.exit(1)
    else:
        text = text.replace(old, new)

identity_line = "      CODESIGN_IDENTITY: ${{ secrets.CODESIGN_IDENTITY }}\n"
if identity_line in text:
    text = text.replace(
        identity_line,
        "      # CODESIGN_IDENTITY: set on macOS by .github/build-setup.yml (not a secret)\n",
    )

if text == original and "secrets.MACOS_CERTIFICATE" not in text and "macos-sign" in pathlib.Path("dist-workspace.toml").read_text():
    print("dist-generate patch: macos-sign is on but CODESIGN/MACOS mapping not applied", file=sys.stderr)
    sys.exit(1)

path.write_text(text)
print(f"patched {path}: MACOS_* secrets mapped; CODESIGN_IDENTITY left to build-setup")
PY
