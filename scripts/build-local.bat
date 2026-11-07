@echo off
rem Local installer build for Windows. Opens a menu; anything typed after the command
rem (win64, --check, --skip-tests) is passed on. It needs Git for Windows, which brings bash.
rem
rem pushd also works from a network share (Windows gives it a temporary drive letter).
pushd "%~dp0.." || exit /b 1

set "BASH="
for %%P in ("%ProgramFiles%\Git\bin\bash.exe" "%ProgramFiles(x86)%\Git\bin\bash.exe" "%LOCALAPPDATA%\Programs\Git\bin\bash.exe") do (
  if exist %%P if not defined BASH set "BASH=%%~P"
)
if not defined BASH (
  echo Git for Windows was not found. Install it from https://git-scm.com/downloads
  popd
  pause
  exit /b 1
)

"%BASH%" scripts/build-local.sh %*
set "CODE=%ERRORLEVEL%"
popd
pause
exit /b %CODE%
