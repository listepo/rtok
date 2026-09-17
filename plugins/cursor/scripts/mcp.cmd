#!/bin/sh
exec "$(dirname "$0")/mcp.sh" "$@"
@echo off
REM Single MCP entry for the Cursor plugin (D21 singleton).
REM Unix execs scripts/mcp.sh through the two lines above; Windows cmd
REM reports those two lines, then runs this body.
where rtok >nul 2>&1
if errorlevel 1 (
  echo rtok is not installed. 1>&2
  echo. 1>&2
  echo Install with ketch: 1>&2
  echo   ketch install listepo/rtok 1>&2
  echo. 1>&2
  echo If ketch is not installed: 1>&2
  echo   curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh ^| bash 1>&2
  echo   ketch install listepo/rtok 1>&2
  exit /b 1
)
rtok mcp
