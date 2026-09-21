#!/bin/sh
# rtok hook launcher for the Claude Code plugin (D21: fail open with the ketch hint).
# The desktop Code tab may start without a shell PATH, so the binary is resolved by hand.
for bin in "$(command -v rtok 2>/dev/null)" "$HOME/.ketch/bin/rtok" \
  /usr/local/bin/rtok /opt/homebrew/bin/rtok; do
  if [ -n "$bin" ] && [ -x "$bin" ]; then
    exec "$bin" hook "$1"
  fi
done
cat >&2 <<'EOF'
rtok is not installed; this hook passes the input through unchanged.

Install with ketch:
  ketch install listepo/rtok

If ketch is not installed:
  curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash
  ketch install listepo/rtok
EOF
exit 0
