@echo off
rem ============================================================================
rem  Real-machine acceptance -- just double-click this file.
rem  (ASCII only on purpose: cmd.exe reads .cmd bytes with the OEM codepage, so
rem   non-ASCII text here turns into mojibake and gets executed as commands.)
rem
rem  What it does:
rem    1) launches two test instances with the freshly built exe (two DSH windows)
rem    2) checks: launch ok / both homes online at once / duplicate launch rejected /
rem       no leftover process after stop / global pointer file restored byte-for-byte
rem    3) answers A8: does upstream honour a private %APPDATA% ?
rem    4) stops the test instances, removes the test dirs, writes one log
rem
rem  While it runs (about 1-2 minutes): do NOT click "restart" in your own DSH
rem  windows. Your own instances are not stopped and their data is not touched.
rem
rem  When it finishes, paste the last 12 lines (or the log path) back to the agent.
rem ============================================================================
setlocal
cd /d "%~dp0.."

set "EXE=%~dp0..\src-tauri\target\release\DSH-NEXT-Multi.exe"
set "LOG=%TEMP%\dsh-nul-accept-report.txt"

echo ============================================================
echo  DSH-NUL launcher -- real-machine acceptance
echo  binary: %EXE%
echo  log:    %LOG%
echo ============================================================
echo.

if not exist "%EXE%" (
  echo [ERROR] not found: %EXE%
  echo         build it first:  pnpm tauri build --no-bundle
  echo.
  if "%~1"=="" pause
  exit /b 1
)

pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0accept-real.ps1" -Exe "%EXE%" > "%LOG%" 2>&1
set "CODE=%ERRORLEVEL%"

type "%LOG%"

echo.
echo ------------------------------------------------------------
if "%CODE%"=="0" (
  echo  RESULT: PASSED  ^(exit=0^)
) else (
  echo  RESULT: FAILED  ^(exit=%CODE%^)
)
echo  full log: %LOG%
echo  paste the last 12 lines back to the agent.
echo ------------------------------------------------------------

if "%~1"=="" pause
exit /b %CODE%
