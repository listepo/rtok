#!/bin/sh
# rtok hook launcher for the ZCode plugin (D21: fail open with the ketch hint).
# ZCode spawns hooks without a shell PATH, so the binary is resolved by hand.
for bin in "$(command -v rtok 2>/dev/null)" "$HOME/.ketch/bin/rtok" \
  /usr/local/bin/rtok /opt/homebrew/bin/rtok; do
  if [ -n "$bin" ] && [ -x "$bin" ]; then
    exec "$bin" hook "$1"
  fi
done
# T174: rtok is missing everywhere we look. Stay silent on every other event — a stderr
# blob on every tool call was 380 "command not found"-shaped hook errors/week in the field
# — and name the install command exactly once, on SessionStart, in Claude's own hook shape.
if [ "$1" = "SessionStart" ]; then
  printf '%s' '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"rtok is not installed; run ketch install listepo/rtok to enable it."}}'
fi
exit 0
