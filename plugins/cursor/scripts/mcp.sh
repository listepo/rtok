#!/bin/sh
# Single MCP entry for the Cursor plugin (D21 singleton).
if ! command -v rtok >/dev/null 2>&1; then
  cat >&2 <<'EOF'
rtok is not installed.

Install with ketch:
  ketch install listepo/rtok

If ketch is not installed:
  curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash
  ketch install listepo/rtok
EOF
  exit 1
fi
exec rtok mcp
