param(
    [switch] $SkipInstall,
    [switch] $SkipFrontend,
    [switch] $SkipRustChecks,
    [switch] $BuildMsi
)

$ErrorActionPreference = 'Stop'

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$TauriRoot = Join-Path $ProjectRoot 'src-tauri'
$WindowsTarget = 'x86_64-pc-windows-msvc'
$TargetRelease = Join-Path $TauriRoot "target\$WindowsTarget\release"
$BundleRoot = Join-Path $TargetRelease 'bundle'
$ReleaseRoot = Join-Path $ProjectRoot 'release\windows'

Set-Location $ProjectRoot

if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    $localSigningKey = Join-Path $ProjectRoot '.localshare-private\updater.key'
    if (Test-Path $localSigningKey) {
        $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw -Path $localSigningKey
    }
}

function Require-Command([string] $Name, [string] $InstallHint) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name is required. $InstallHint"
    }
}

function Assert-Pe([string] $Path, [bool] $RequireX64) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        throw "$Path is not a Windows PE executable (missing MZ header)."
    }
    $peOffset = [System.BitConverter]::ToInt32($bytes, 0x3c)
    if ($peOffset -lt 0 -or $peOffset + 6 -gt $bytes.Length -or [System.Text.Encoding]::ASCII.GetString($bytes, $peOffset, 4) -ne "PE`0`0") {
        throw "$Path is not a Windows PE executable (missing PE header)."
    }
    $machine = [System.BitConverter]::ToUInt16($bytes, $peOffset + 4)
    if ($RequireX64 -and $machine -ne 0x8664) {
        throw "$Path is not x86_64 PE. Machine field was 0x$('{0:x4}' -f $machine)."
    }
}

Require-Command 'node' 'Install the current Node.js LTS release on Windows.'
Require-Command 'npm' 'Install npm with the current Node.js LTS release.'
Require-Command 'rustc' 'Install Rust stable with rustup and the MSVC toolchain.'
Require-Command 'cargo' 'Install Rust stable with rustup and the MSVC toolchain.'
Require-Command 'rustup' 'Install Rust with rustup, then add the x86_64-pc-windows-msvc target.'

Write-Host "Project: $ProjectRoot"
Write-Host "Node: $(node --version)"
Write-Host "npm: $(npm --version)"
Write-Host "Rust: $(rustc --version)"
Write-Host "Cargo: $(cargo --version)"
Write-Host "Target: $WindowsTarget"

if (-not ((rustup target list --installed) -contains $WindowsTarget)) {
    throw "Rust target $WindowsTarget is not installed. Run: rustup target add $WindowsTarget"
}

if (-not (Test-Path (Join-Path $ProjectRoot 'package-lock.json'))) {
    throw 'package-lock.json is missing; refusing a non-reproducible npm install.'
}

if (-not $SkipInstall) {
    Write-Host 'Installing locked frontend dependencies...'
    npm ci
}

if (-not $SkipFrontend) {
    Write-Host 'Building frontend...'
    npm run build
}

if (-not $SkipRustChecks) {
    Push-Location $TauriRoot
    try {
        Write-Host 'Running Rust checks...'
        cargo check --locked --all-targets
        cargo test --locked --all-targets
    }
    finally {
        Pop-Location
    }
}

Write-Host 'Building the production Tauri Windows bundles...'
# The two_node binary is feature-gated in Cargo.toml. This command builds the
# default localshare binary only; the validation below rejects test binaries in
# the bundle if a stale or misconfigured build ever includes one.
$bundles = if ($BuildMsi) { 'nsis,msi' } else { 'nsis' }
$tauriArgs = @('--target', $WindowsTarget, '--bundles', $bundles)
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    # Main-branch builds must not receive release signing credentials. Tauri
    # still produces a normal installer, but does not attempt updater artifacts.
    $tauriArgs += @('--config', '{"bundle":{"createUpdaterArtifacts":false}}')
}
& npm run tauri build -- @tauriArgs
if ($LASTEXITCODE -ne 0) { throw "Tauri build failed with exit code $LASTEXITCODE" }

$productionExe = Join-Path $TargetRelease 'localshare.exe'
if (-not (Test-Path $productionExe)) {
    $productionExe = Get-ChildItem -Path $TargetRelease -Filter 'localshare.exe' -File -Recurse |
        Where-Object { $_.FullName -notmatch '\\bundle\\' } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $productionExe -or -not (Test-Path $productionExe)) {
    throw 'The production localshare.exe was not emitted.'
}
Assert-Pe $productionExe $true

$bundledTestBinary = Get-ChildItem -Path $BundleRoot -Filter 'two_node.exe' -File -Recurse -ErrorAction SilentlyContinue
if ($bundledTestBinary) {
    throw "The test-only two_node.exe was included in the production bundle: $($bundledTestBinary.FullName)"
}

$setupExe = Get-ChildItem -Path (Join-Path $BundleRoot 'nsis') -Filter '*-setup.exe' -File -Recurse -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -notmatch 'webview2|WebView2' } |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $setupExe) {
    throw 'The NSIS setup executable was not emitted. A genuine LocalShare-Setup.exe cannot be produced.'
}
Assert-Pe $setupExe.FullName $false

New-Item -ItemType Directory -Force -Path $ReleaseRoot | Out-Null
foreach ($name in @('LocalShare-Setup.exe', 'LocalShare.exe', 'LocalShare.msi', 'RUN-LOCALSHARE.cmd', 'README-WINDOWS.txt', 'SHA256SUMS.txt')) {
    $old = Join-Path $ReleaseRoot $name
    if (Test-Path $old) { Remove-Item -Force $old }
}

$finalSetup = Join-Path $ReleaseRoot 'LocalShare-Setup.exe'
$finalPortable = Join-Path $ReleaseRoot 'LocalShare.exe'
Copy-Item $setupExe.FullName $finalSetup -Force
Copy-Item $productionExe $finalPortable -Force
Copy-Item (Join-Path $ProjectRoot 'RUN-LOCALSHARE.cmd') (Join-Path $ReleaseRoot 'RUN-LOCALSHARE.cmd') -Force

$msi = Get-ChildItem -Path (Join-Path $BundleRoot 'msi') -Filter '*.msi' -File -Recurse -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if ($msi) {
    Copy-Item $msi.FullName (Join-Path $ReleaseRoot 'LocalShare.msi') -Force
}

$readme = @"
LocalShare Windows release
==========================

Primary installer: LocalShare-Setup.exe
Portable executable: LocalShare.exe

The installer is built by Tauri/NSIS for Windows 10/11 x64. It embeds the
WebView2 offline installer so first installation does not need Internet.
LocalShare's runtime communication remains LAN-only and uses the
_localshare._tcp.local. mDNS service.

On first launch, allow LocalShare through Windows Defender Firewall on
Private networks. Do not enable Public network access unless your network
policy requires it.

The portable fallback is RUN-LOCALSHARE.cmd beside LocalShare.exe. End users
do not need Node.js, npm, Rust, Cargo, Git, or the source repository.
"@
Set-Content -Path (Join-Path $ReleaseRoot 'README-WINDOWS.txt') -Value $readme -Encoding UTF8

$releaseFiles = Get-ChildItem -Path $ReleaseRoot -File |
    Where-Object { $_.Name -match '\.(exe|msi)$' } |
    Sort-Object Name
$sumLines = foreach ($file in $releaseFiles) {
    $hash = Get-FileHash -Algorithm SHA256 -Path $file.FullName
    "{0}  {1}" -f $hash.Hash.ToLowerInvariant(), $file.Name
}
Set-Content -Path (Join-Path $ReleaseRoot 'SHA256SUMS.txt') -Value ($sumLines -join [Environment]::NewLine) -Encoding ASCII

Write-Host ''
Write-Host 'WINDOWS BUILD COMPLETE' -ForegroundColor Green
Write-Host "EXE: $finalPortable"
Write-Host "INSTALLER: $finalSetup"
if ($msi) { Write-Host "MSI: $(Join-Path $ReleaseRoot 'LocalShare.msi')" }
Write-Host "CHECKSUMS: $(Join-Path $ReleaseRoot 'SHA256SUMS.txt')"
