$ErrorActionPreference = 'Stop'
Write-Host 'LocalShare Windows build'
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Rust/Cargo is required.' }
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) { throw 'Node.js/npm is required.' }
npm install
npm run build
Push-Location src-tauri
cargo tauri build --bundles nsis,msi
Pop-Location
Write-Host 'Windows artifacts are under src-tauri/target/release/bundle/'
