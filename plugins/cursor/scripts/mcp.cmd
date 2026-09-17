@echo off
REM Single MCP entry helper for Windows (D21 singleton / ketch message).
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
