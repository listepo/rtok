#!/usr/bin/env bash
# Copyright (c) 2026 Ivan Tugay
# SPDX-License-Identifier: GPL-3.0-or-later
# Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

# T319: man pages and completion scripts for the release archives, from the built binary.
# Usage: tools/share-files.sh <rtok binary> [out dir, default share]
# Writes <out>/man/man1/*.1 (rtok man --dir) and <out>/completions/{rtok.bash,_rtok,rtok.fish,
# rtok.ps1,rtok.elv,rtok.lua}. Not committed (79 pages); dist `include`s the directory.
set -euo pipefail
bin="${1:?usage: tools/share-files.sh <rtok binary> [out dir]}"
out="${2:-share}"
rm -rf "$out"
mkdir -p "$out/man/man1" "$out/completions"
"$bin" man --dir "$out/man/man1" >/dev/null
for pair in bash:rtok.bash zsh:_rtok fish:rtok.fish powershell:rtok.ps1 elvish:rtok.elv clink:rtok.lua; do
  "$bin" completions "${pair%%:*}" > "$out/completions/${pair#*:}"
done
echo "$(find "$out" -type f | wc -l | tr -d ' ') files in $out"
