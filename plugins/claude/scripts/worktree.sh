#!/bin/sh
# Copyright (c) 2026 Ivan Tugay
# SPDX-License-Identifier: GPL-3.0-or-later
# Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

# Launcher for Claude Code's WorktreeCreate / WorktreeRemove hooks (T159, D31). Unlike hook.sh it
# cannot fail open by printing nothing: the host replaces its own create and remove with these
# hooks, so a create that prints no path fails the session. Each failure path therefore ends in
# what the host itself would have done, never in a lost worktree.
event=$1
input=$(cat)
# The fetch behind `rtok worktree add` must never wait for a prompt the host cannot show.
GIT_TERMINAL_PROMPT=0
export GIT_TERMINAL_PROMPT

bin=
for b in "$(command -v rtok 2>/dev/null)" "$HOME/.ketch/bin/rtok" \
  /usr/local/bin/rtok /opt/homebrew/bin/rtok; do
  if [ -n "$b" ] && [ -x "$b" ]; then
    bin=$b
    break
  fi
done

# The payload's string fields are slugs and absolute paths, so a sed is enough and no jq is needed.
field() {
  printf '%s' "$input" | sed -n 's/.*"'"$1"'"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
}

case "$event" in
WorktreeCreate)
  if [ -n "$bin" ]; then
    # An rtok too old to know the event prints `{}`: anything but a path falls through.
    if out=$(printf '%s' "$input" | "$bin" hook WorktreeCreate); then
      case "$out" in
      /* | [A-Za-z]:*)
        printf '%s\n' "$out"
        exit 0
        ;;
      esac
    fi
    echo "rtok: rtok worktree add failed; creating a plain worktree" >&2
  fi
  name=$(field name | tr -c 'A-Za-z0-9._-' '-')
  case "$name" in '' | .* | -*) name="w-$name" ;; esac
  root=$(git -C "$(field cwd)" rev-parse --show-toplevel) || exit 1
  # The host's own default: .claude/worktrees/<name> on worktree-<name>, from the current HEAD.
  dir=$root/.claude/worktrees/$name
  if [ ! -e "$dir/.git" ]; then
    git -C "$root" worktree add --quiet -b "worktree-$name" "$dir" >&2 ||
      git -C "$root" worktree add --quiet "$dir" "worktree-$name" >&2 || exit 1
  fi
  printf '%s\n' "$dir"
  ;;
WorktreeRemove)
  if [ -n "$bin" ]; then
    printf '%s' "$input" | "$bin" hook WorktreeRemove
    exit $?
  fi
  path=$(field worktree_path)
  [ -d "$path" ] || exit 0
  # Never --force: git refuses a dirty worktree, and the host keeps it and reports the reason.
  git -C "$(field cwd)" worktree remove "$path"
  ;;
esac
