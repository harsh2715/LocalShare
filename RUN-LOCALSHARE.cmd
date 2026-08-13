@echo off
setlocal
set "APP_EXE=%~dp0LocalShare.exe"

if not exist "%APP_EXE%" (
  echo LocalShare.exe was not found.
  echo Please use the complete LocalShare Windows folder.
  pause
  exit /b 1
)

start "LocalShare" "%APP_EXE%"
exit /b 0
