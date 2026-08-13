@echo off
setlocal
cd /d "%~dp0"

where npm >nul 2>nul || (echo Node.js/npm is required for the development launcher.&pause&exit /b 1)
where cargo >nul 2>nul || (echo Rust/Cargo is required for the development launcher.&pause&exit /b 1)

call npm install
if errorlevel 1 exit /b 1
call npm run build
if errorlevel 1 exit /b 1
cd src-tauri
cargo tauri build --bundles nsis,msi
if errorlevel 1 exit /b 1
cd ..
echo Build complete. Packaged artifacts are under src-tauri\target\release\bundle.
pause
