@echo off
rem Double-click: collect a redacted G-OpenSteamTool diagnostic bundle to the Desktop.
rem Extra args are forwarded, e.g.  collect-diagnostics.cmd -SteamDir "D:\Steam" -NoZip
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0collect-diagnostics.ps1" %*
set code=%errorlevel%
if not "%code%"=="0" (
  echo.
  echo Collection failed with exit code %code%. Pass -SteamDir if Steam is not found.
  pause
)
exit /b %code%
