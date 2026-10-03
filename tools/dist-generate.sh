#!/usr/bin/env bash
# Copyright (c) 2026 Ivan Tugay
# SPDX-License-Identifier: GPL-3.0-or-later
# Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

# Regenerate .github/workflows/release.yml from dist-workspace.toml, then map
# dist's CODESIGN_* secret names to the MACOS_* secrets this repository uses
# (same names as pyrlyn/ketch). CODESIGN_IDENTITY is not a secret: the
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

REPORT = """      - name: Report artifact sizes
        shell: bash
        run: |
          set -euo pipefail
          root="target/distrib"
          if [ ! -d "$root" ]; then
            echo "no $root yet; skipping size report"
            exit 0
          fi
          {
            echo "### Release artifact sizes"
            echo
            echo "| File | Size |"
            echo "|---|---:|"
            find "$root" -maxdepth 1 -type f \\( \\
              -name '*.tar.xz' -o -name '*-update' -o -name 'rtok-installer.sh' -o -name 'rtok.rb' -o -name 'sha256.sum' -o -name 'source.tar.gz' \\
            \\) -print0 | sort -z | while IFS= read -r -d '' f; do
              bytes=$(wc -c <"$f" | tr -d ' ')
              human=$(awk -v b="$bytes" 'BEGIN {
                if (b < 1024) { printf "%d B", b; exit }
                if (b < 1048576) { printf "%.1f KiB", b/1024; exit }
                printf "%.2f MiB", b/1048576
              }')
              echo "| $(basename "$f") | ${human} (${bytes} bytes) |"
            done
          } | tee -a "$GITHUB_STEP_SUMMARY"
"""

if "Report artifact sizes" not in text:
    for anchor in (
        "          name: artifacts-build-local-${{ join(matrix.targets, '_') }}",
        "          name: artifacts-build-global",
    ):
        idx = text.find(anchor)
        if idx < 0:
            print(f"dist-generate patch: missing upload anchor {anchor}", file=sys.stderr)
            sys.exit(1)
        step_start = text.rfind('      - name: "Upload artifacts"', 0, idx)
        if step_start < 0:
            print("dist-generate patch: Upload step missing", file=sys.stderr)
            sys.exit(1)
        text = text[:step_start] + REPORT + "\n" + text[step_start:]

# create-release = false: bump.yml made the tag and a draft Release (notes from CHANGELOG.md);
# dist uploads to it and undrafts it. Append the archive sizes to bump's notes on the way.
create_old = """          # If we're editing a release in place, we need to upload things ahead of time
          gh release upload \"${{ needs.plan.outputs.tag }}\" artifacts/*

          gh release edit \"${{ needs.plan.outputs.tag }}\" --target \"$RELEASE_COMMIT\" $PRERELEASE_FLAG --draft=false
"""

create_new = """          # If we're editing a release in place, we need to upload things ahead of time
          gh release upload \"${{ needs.plan.outputs.tag }}\" artifacts/*

          # bump.yml wrote the notes; append archive sizes so the Release page shows MiB.
          gh release view \"${{ needs.plan.outputs.tag }}\" --json body --jq .body > \"$RUNNER_TEMP/notes.txt\"
          {
            echo
            echo \"## Download sizes\"
            echo
            echo \"| File | Size |\"
            echo \"|---|---:|\"
            find artifacts -maxdepth 1 -type f \\( \\
              -name '*.tar.xz' -o -name '*-update' -o -name 'rtok-installer.sh' -o -name 'rtok.rb' -o -name 'source.tar.gz' \\
            \\) -print0 | sort -z | while IFS= read -r -d '' f; do
              bytes=$(wc -c <\"$f\" | tr -d ' ')
              human=$(awk -v b=\"$bytes\" 'BEGIN {
                if (b < 1024) { printf \"%d B\", b; exit }
                if (b < 1048576) { printf \"%.1f KiB\", b/1024; exit }
                printf \"%.2f MiB\", b/1048576
              }')
              echo \"| $(basename \"$f\") | ${human} |\"
            done
          } >> \"$RUNNER_TEMP/notes.txt\"
          sed -n '/^## Download sizes$/,$p' \"$RUNNER_TEMP/notes.txt\" | tee -a \"$GITHUB_STEP_SUMMARY\"

          gh release edit \"${{ needs.plan.outputs.tag }}\" --target \"$RELEASE_COMMIT\" $PRERELEASE_FLAG --notes-file \"$RUNNER_TEMP/notes.txt\" --draft=false
"""

if "## Download sizes" not in text:
    if create_old not in text:
        print("dist-generate patch: Release upload block missing/changed (create-release = false?)", file=sys.stderr)
        sys.exit(1)
    text = text.replace(create_old, create_new, 1)

if (
    text == original
    and "secrets.MACOS_CERTIFICATE" not in text
    and "macos-sign" in pathlib.Path("dist-workspace.toml").read_text()
):
    print(
        "dist-generate patch: macos-sign is on but CODESIGN/MACOS mapping not applied",
        file=sys.stderr,
    )
    sys.exit(1)

# T279: fail the release before it builds anything when a plugin manifest or
# .rtok-plugin-version file does not match the tag being released (`v` stripped).
plan_checkout = """  plan:
    runs-on: "ubuntu-22.04"
    outputs:
      val: ${{ steps.plan.outputs.manifest }}
      tag: ${{ (inputs.tag != 'dry-run' && inputs.tag) || '' }}
      tag-flag: ${{ inputs.tag && inputs.tag != 'dry-run' && format('--tag={0}', inputs.tag) || '' }}
      publishing: ${{ inputs.tag && inputs.tag != 'dry-run' }}
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false
          submodules: recursive
      - name: Install dist"""

plan_checkout_with_check = """  plan:
    runs-on: "ubuntu-22.04"
    outputs:
      val: ${{ steps.plan.outputs.manifest }}
      tag: ${{ (inputs.tag != 'dry-run' && inputs.tag) || '' }}
      tag-flag: ${{ inputs.tag && inputs.tag != 'dry-run' && format('--tag={0}', inputs.tag) || '' }}
      publishing: ${{ inputs.tag && inputs.tag != 'dry-run' }}
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false
          submodules: recursive
      - name: Check plugin manifest versions
        if: ${{ inputs.tag && inputs.tag != 'dry-run' }}
        run: tools/plugin-versions.sh --check "${TAG#v}"
        env:
          TAG: ${{ inputs.tag }}
      - name: Install dist"""

if "Check plugin manifest versions" not in text:
    if plan_checkout not in text:
        print("dist-generate patch: plan job checkout block missing/changed", file=sys.stderr)
        sys.exit(1)
    text = text.replace(plan_checkout, plan_checkout_with_check, 1)

# A last job that turns a failed release into a `release-failure` issue (pyrlyn/infra).
NOTIFY = """
  # Added by tools/dist-generate.sh: a failed release (not a pull request or a dry run)
  # opens or comments on a `release-failure` issue that mentions and assigns @listepo. The
  # only release failure notification: GitHub cannot filter Actions notifications per
  # workflow. Pinned to pyrlyn/infra's ci/notify-release-failure; repin to its merge commit.
  notify-failure:
    needs: [plan, build-local-artifacts, build-global-artifacts, host, announce]
    if: >-
      always() && github.event_name == 'workflow_dispatch' && inputs.tag != 'dry-run'
      && contains(needs.*.result, 'failure')
    runs-on: "ubuntu-22.04"
    timeout-minutes: 5
    permissions:
      "actions": "read"
      "issues": "write"
    steps:
      - uses: pyrlyn/infra/.github/actions/notify-release-failure@d709124d53dd4923eff8f594b3155842508b0049
        with:
          ref: ${{ inputs.tag }}
          needs: ${{ toJSON(needs) }}
"""
if "notify-failure:" not in text:
    text = text.rstrip("\n") + "\n" + NOTIFY

path.write_text(text)
print(
    f"patched {path}: MACOS_* secrets, artifact size reports, release notes sizes, "
    "plugin version check, notify-failure job"
)
PY
