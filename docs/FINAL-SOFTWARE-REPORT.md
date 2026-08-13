# LocalShare Final Software Report

Audit date: 2026-08-13.

## Final status

PARTIAL. The existing project has a working macOS frontend/build path, corrected production-binary packaging, real TLS two-node chat and single-file/hash coverage, source-wired automatic mDNS connection, incoming approval, settings/history/diagnostics, and a prepared Windows x64 GitHub Actions installer pipeline. The primary Windows installer was not generated because this checkout has no GitHub remote/workflow run. No `.exe` was fabricated.

## Required Windows release status

```text
WINDOWS VM: NOT REQUIRED — GitHub Actions path configured
WINDOWS RUNNER: NOT RUN — no GitHub remote is configured for this checkout
WINDOWS BUILD ENVIRONMENT: Apple Silicon macOS; no Windows VM, no rustup, no Windows target libraries, no MSVC linker, no MinGW/Zig/cargo-xwin
WINDOWS BUILD: NOT VERIFIED — workflow configured but not executed
WINDOWS EXE: NOT VERIFIED — hosted workflow pending
WINDOWS INSTALLER: NOT VERIFIED — hosted workflow pending
FINAL INSTALLER: NOT CREATED LOCALLY; GitHub artifact is pending workflow execution
FINAL INSTALLER PATH: /Users/harshsinghal/Desktop/Local Network/release/windows/LocalShare-Setup.exe (does not exist)
WINDOWS GUI: NOT VERIFIED
WINDOWS AUTO IP: NOT VERIFIED
WINDOWS MDNS ADVERTISEMENT: NOT VERIFIED
WINDOWS MDNS BROWSING: NOT VERIFIED
MAC ↔ WINDOWS AUTO DISCOVERY: NOT VERIFIED
```

The exact host evidence is in `artifacts/test-results/windows-build-host-audit.txt`, `docs/WINDOWS-BUILD-AUDIT.md`, and `docs/WINDOWS-VM-BUILD-REPORT.md`. The cross-target command failed because `x86_64-pc-windows-msvc` has no installed `core/std` target libraries on this Mac. The GitHub Actions workflow avoids that local limitation, but it has not run because no remote repository is configured.

## What is prepared

- `src-tauri/Cargo.toml` explicitly maps production `localshare` to `src-tauri/src/main.rs`; `two_node` is opt-in behind the `two-node` feature.
- `src-tauri/tauri.conf.json` sets `mainBinaryName: localshare` and Tauri Windows `offlineInstaller` WebView2 mode.
- `BUILD-WINDOWS.cmd` calls `scripts/build-windows.ps1` using `%~dp0`.
- `.github/workflows/windows-release.yml` is the preferred build path: it uses a GitHub-hosted Windows runner and uploads `LocalShare-Setup.exe` as the `LocalShare-Setup` Actions artifact.
- `scripts/build-windows.ps1` runs locked npm install/build, Rust checks/tests, an explicit `x86_64-pc-windows-msvc` Tauri build, PE validation, `two_node.exe` rejection, NSIS normalization, optional MSI copy, and SHA-256 generation.
- `RUN-LOCALSHARE.cmd` launches a portable `LocalShare.exe` from its own folder.
- `release/windows/README-WINDOWS.txt` is an honest handoff; it contains no fake executable or checksum.

## macOS regression evidence

- BUILD: PASS — `npm run build`, `cd src-tauri && cargo check --all-targets`, `cargo test --all-targets`.
- VIRTUAL TWO-NODE TEST: PASS for the implemented harness slice — `cargo run --features two-node --bin two_node` passed real TLS chat A→B, chat B→A, 1 MiB A→B, and SHA-256 equality. Evidence: `artifacts/test-results/two-node-pass.txt`.
- MACOS `.app`: PASS from prior actual packaged inspection — `Contents/MacOS/localshare` is the only application executable and matches the release binary; `two_node` is not the packaged executable.
- MACOS DMG: PASS from prior actual mounted inspection — `src-tauri/target/release/bundle/macos/LocalShare_0.1.0_aarch64.dmg` contains the corrected app.
- PACKAGED MAC RUNTIME: PASS from prior actual launch — non-blank React UI, SQLite, listener, local diagnostics, and navigation were inspected.

## Acceptance matrix

| Area | Status | Evidence or reason |
| --- | --- | --- |
| MAC GUI | PASS | Actual packaged GUI opened and navigation was inspected |
| MAC `.app` | PASS | Only `Contents/MacOS/localshare`; checksum matched release binary |
| MAC DMG | PASS | Mounted DMG contained the corrected app |
| WINDOWS RUNNER | NOT VERIFIED | Workflow configured; no GitHub remote/run exists |
| WINDOWS EXE | NOT VERIFIED | GitHub runner workflow pending |
| WINDOWS INSTALLER | NOT VERIFIED | GitHub runner workflow pending |
| WINDOWS INSTALL TEST | NOT VERIFIED | No Windows runtime |
| WINDOWS FIREWALL | NOT VERIFIED | Requires Windows runtime |
| Automatic IP — Mac | PASS | Packaged diagnostics preferred the active private IPv4 observed on this Mac |
| Automatic IP — Windows | NOT VERIFIED | No Windows runtime |
| mDNS — Mac | PARTIAL | Local `_localshare._tcp.local.` advertisement observed; second-device discovery not verified |
| mDNS — Windows | NOT VERIFIED | No Windows runtime |
| Automatic peer discovery/connection | PARTIAL | Production source path exists with deterministic UUID-side connection selection; no physical two-device run |
| Manual IP required | NO for normal workflow | Retained only as troubleshooting fallback |
| Trust/pinning | PARTIAL | TLS encryption exists, but certificate pinning, persisted cryptographic identity, and trust prompts are incomplete |
| Messages page | PASS source/UI | Packaged navigation and SQLite command path inspected; cross-device runtime not fully verified |
| Transfer history | PASS source/UI | SQLite-backed page and clear action wired; population not fully E2E verified |
| Settings | PASS source/UI | Persistent settings command/page wired; Windows restart persistence not verified |
| Diagnostics | PASS source/UI | Status fields and redacted Copy diagnostics wired; Windows values not verified |
| Chat A→B | PASS | Real TLS two-node harness |
| Chat B→A | PASS | Real TLS two-node harness |
| Single file A→B | PASS | Real TLS two-node harness, SHA-256 equality |
| Single file B→A | NOT VERIFIED | Harness does not cover reverse file direction |
| Multiple files | NOT VERIFIED | Source/UI path exists; no E2E batch run |
| Large file | NOT VERIFIED | No 100 MB run |
| SHA-256 | PASS | Harness 1 MiB transfer; receive verifies before rename |
| Chat during transfer | NOT VERIFIED | Separate control/data queues exist; acceptance run pending |
| Cancel | PARTIAL | Sender/receiver command/protocol/UI wiring exists; E2E run pending |
| Reconnect | PARTIAL | Stale mDNS removal/rediscovery source path exists; E2E run pending |
| Resume | NOT IMPLEMENTED | Not faked |
| Path traversal | PASS | Rust unit tests cover traversal, absolute, and Windows path forms |
| Offline static audit | PASS | No configured public runtime endpoint; evidence in `artifacts/test-results/offline-static-audit.txt` |
| Offline runtime audit | PARTIAL | Limited LocalShare-owned socket audit exists; WAN-off packet capture not performed |
| Physical two-computer | NOT VERIFIED | No second physical Windows computer/Windows VM |

## Exact GitHub Actions handoff

Push this existing project to its intended GitHub repository, then run the `LocalShare Windows release` workflow from the Actions tab. The workflow uploads:

```bat
LocalShare-Setup
```

The artifact contains the genuine NSIS installer generated by the Windows runner. The workflow summary and `SHA256SUMS.txt` provide the exact size and SHA-256. After downloading it, Windows runtime and Mac↔Windows physical LAN testing remain separate and are not claimed by Actions.

```text
WINDOWS RUNTIME: NOT VERIFIED unless executed on Windows
MAC↔WINDOWS PHYSICAL LAN: NOT VERIFIED
```

The exact artifact download location will be the `LocalShare-Setup` artifact on that workflow run’s GitHub Actions page. The exact URL cannot be reported yet because this checkout has no GitHub remote and no workflow run exists.

Only after that installer is executed and the installed GUI is tested in Windows can the Windows runtime statuses move from unverified to PASS or FAIL.
