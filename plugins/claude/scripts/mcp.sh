#!/bin/sh
# Single MCP entry for the Claude Code plugin (D21 singleton).
for bin in "$(command -v rtok 2>/dev/null)" "$HOME/.ketch/bin/rtok" \
  /usr/local/bin/rtok /opt/homebrew/bin/rtok; do
  if [ -n "$bin" ] && [ -x "$bin" ]; then
    exec "$bin" mcp
  fi
done
cat >&2 <<'EOF'
rtok is not installed.

Install with ketch:
  ketch install listepo/rtok

If ketch is not installed:
  curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash
  ketch install listepo/rtok
EOF
exit 1
