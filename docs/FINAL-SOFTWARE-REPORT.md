# LocalShare Final Software Report

Audit date: 2026-08-13.

## Final status

PARTIAL. The Windows installer milestone is complete on GitHub Actions. The broader product remains partial because physical LAN, GUI visual inspection, trust/pinning, multi-file, large-file, cancel, reconnect, and offline physical tests remain unverified or incomplete.

## Windows release result

```text
WINDOWS RUNNER: PASS
WINDOWS BUILD: PASS
LOCALSHARE.EXE: PASS
LOCALSHARE.EXE PE VALIDATION: PASS — x86_64 PE on runner
NSIS BUILD: PASS
LOCALSHARE-SETUP.EXE: PASS
INSTALLER PE VALIDATION: PASS — genuine PE32 Nullsoft installer
INSTALLER SHA-256: 26614a23763f11e78d3ab9bda9e0fd50c0d4b69a974944295de8e21895d69e02
GITHUB ARTIFACT: PASS
ARTIFACT NAME: LocalShare-Setup
WINDOWS INSTALL AUTOMATION: PASS — silent NSIS install completed
WINDOWS PROCESS LAUNCH: PASS — installed process stayed alive during smoke test
WINDOWS GUI VISUAL TEST: NOT VERIFIED — hosted runner has no visual inspection
WINDOWS RUNTIME: PARTIAL — process launch and transport harness passed; physical runtime not verified
```

Successful workflow run: [31724824519](https://github.com/harsh2715/LocalShare/actions/runs/31724824519)

Final artifact download: [LocalShare-Setup](https://github.com/harsh2715/LocalShare/actions/runs/31724824519/artifacts/9191863029)

The exact installer is also copied locally at `/Users/harshsinghal/Desktop/Local Network/release/windows/LocalShare-Setup.exe`.

## First Windows failure and fix

The first Windows run failed in `Check Rust targets` because Tauri’s build script could not find `src-tauri/icons/icon.ico`:

```text
`icons/icon.ico` not found; required for generating a Windows Resource file during tauri-build
```

Fix: added the required checked-in ICO resource, generated reproducibly from the existing `src-tauri/icons/icon.png` by `scripts/create-windows-icon.py`. No runtime code or macOS architecture was replaced.

## Windows acceptance matrix

| Area | Status | Evidence |
| --- | --- | --- |
| Checkout | PASS | Hosted run checked out commit `bb3fed350ebaa6fcb6f64e0a5b518a4fc4024475` |
| Node setup | PASS | Node 22 from `.nvmrc` |
| Rust/MSVC setup | PASS | Stable Rust and `x86_64-pc-windows-msvc` |
| npm lockfile install | PASS | `npm ci` |
| Frontend build | PASS | `npm run build` |
| Windows Rust check | PASS | `cargo check --all-targets` |
| Windows Rust tests | PASS | `cargo test --all-targets`, 4 passed per target |
| Windows two-node harness | PASS | Real TLS chat A→B/B→A, single file A→B, SHA-256 |
| Tauri production build | PASS | Tauri production binary emitted |
| `LocalShare.exe` | PASS | Release directory contains `LocalShare.exe`; `two_node.exe` rejected |
| `LocalShare.exe` PE | PASS | x86_64 PE validation |
| NSIS build | PASS | Setup executable emitted |
| Installer PE | PASS | PE signature validated; downloaded artifact is PE32 Nullsoft installer |
| SHA-256 | PASS | `26614a23763f11e78d3ab9bda9e0fd50c0d4b69a974944295de8e21895d69e02` |
| Silent install | PASS | NSIS `/S` smoke test |
| Installed process launch | PASS | Process stayed alive for smoke interval |
| Artifact upload | PASS | `LocalShare-Setup` artifact ID `9191863029` |
| Auto IP Windows | NOT VERIFIED | No physical user LAN |
| mDNS Windows | NOT VERIFIED | No physical user LAN |
| Windows firewall | NOT VERIFIED | Hosted runner cannot validate user firewall prompt/policy |
| Windows GUI visual test | NOT VERIFIED | No interactive visual inspection |

## Existing product matrix

| Area | Status | Evidence or reason |
| --- | --- | --- |
| macOS build | PASS | `npm run build`, Rust checks/tests |
| macOS `.app` / DMG | PASS | Correct `Contents/MacOS/localshare`; prior mounted DMG inspection |
| Automatic peer discovery/connection | PARTIAL | Source path and deterministic selection exist; physical peer not tested |
| Manual IP required | NO for normal workflow | Troubleshooting fallback only |
| Trust/pinning | PARTIAL | TLS exists; persisted cryptographic identity, pinning, and trust prompts incomplete |
| Messages | PASS source/UI; cross-device E2E NOT VERIFIED | Packaged navigation and SQLite command path inspected |
| Transfer history | PASS source/UI; E2E population NOT VERIFIED | SQLite-backed page wired |
| Settings | PASS source/UI; Windows restart NOT VERIFIED | Persistent settings wired |
| Diagnostics | PASS source/UI; Windows values NOT VERIFIED | Redacted Copy diagnostics wired |
| Single file | PASS in Windows/macOS two-node harness A→B | SHA-256 verified; reverse direction not covered |
| Multiple files | NOT VERIFIED | No E2E batch run |
| 100 MB file | NOT VERIFIED | No 100 MB run |
| Chat during transfer | NOT VERIFIED | Control/data queues exist; acceptance run pending |
| Cancel | PARTIAL | Source/UI wiring exists; E2E run pending |
| Reconnect | PARTIAL | Source rediscovery path exists; E2E run pending |
| Resume | NOT IMPLEMENTED | Not faked |
| Path traversal | PASS | Rust unit tests cover Unix/Windows/absolute forms |
| Offline static audit | PASS | No configured public runtime endpoint |
| Offline physical LAN | NOT VERIFIED | Requires real Mac↔Windows LAN with Internet/WAN unavailable |
| Physical Mac↔Windows | NOT VERIFIED | Requires user’s physical second computer |

## Runtime/offline boundary

The installer uses Tauri’s offline WebView2 installer mode. The hosted runner built and silently installed it, but a user-machine Internet-off installation has not been tested. LocalShare’s intended runtime transport remains direct TCP/TLS plus `_localshare._tcp.local` mDNS; the hosted runner did not perform physical LAN or Internet-off claims.

## Commands/evidence

```text
npm run build
cd src-tauri && cargo check --all-targets
cd src-tauri && cargo test --all-targets
cd src-tauri && cargo run --features two-node --bin two_node
```

Hosted evidence is in `artifacts/test-results/windows-rust-failure.txt` and `artifacts/test-results/windows-runner.txt`. The workflow is `.github/workflows/windows-release.yml`.
