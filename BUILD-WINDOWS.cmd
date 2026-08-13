@echo off
setlocal
cd /d "%~dp0"

if not exist "%~dp0scripts\build-windows.ps1" (
  echo scripts\build-windows.ps1 was not found.
  echo Run this file from the complete LocalShare source folder.
  pause
  exit /b 1
)

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-windows.ps1"
if errorlevel 1 (
  echo.
  echo WINDOWS BUILD FAILED
  pause
  exit /b 1
)

echo.
echo WINDOWS BUILD COMPLETE
echo EXE: %~dp0release\windows\LocalShare.exe
echo INSTALLER: %~dp0release\windows\LocalShare-Setup.exe
pause
exit /b 0
