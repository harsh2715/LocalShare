# Current Implementation Audit

Audit date: 2026-08-13. This reflects source inspection and the verification commands recorded in `artifacts/test-results/`.

Windows-specific build and host findings are recorded separately in `docs/WINDOWS-BUILD-AUDIT.md` and `docs/WINDOWS-VM-BUILD-REPORT.md`. The current Mac has no Windows VM or Windows toolchain, so Windows runtime/package claims remain blocked or unverified.

## Implemented and verified in source

- React/Vite UI: `src/main.tsx`, `src/styles.css`; `npm run build` PASS.
- Tauri shell: `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`; `mainBinaryName: localshare` is explicit, and `scripts/build-macos.sh` removes the test-only `two_node` from the generated app payload. The packaged app now contains only `Contents/MacOS/localshare`.
- Local SQLite schema: `src-tauri/src/storage.rs`; app database opened at platform local-data `LocalShare/localshare.sqlite`.
- Persistent UUID: `src-tauri/src/identity.rs`; fingerprint is a SHA-256-derived UUID value, not a public-key identity.
- Framed protocol and validation: `src-tauri/src/protocol.rs`; unit tests now cover Unicode, traversal, Windows/absolute names, bad manifest, and version.
- Direct TLS listener/client: `src-tauri/src/main.rs`; TLS 1.3-capable rustls session with AWS-LC provider installed at startup.
- mDNS: `start_mdns` in `src-tauri/src/main.rs`; advertises and browses `_localshare._tcp.local.`, retains the daemon handle, removes stale services, prefers active private IPv4, and automatically connects from the lower device UUID to avoid duplicate outbound sessions.
- Bounded file I/O: sender uses 1 MiB buffers; receiver writes partial files and verifies SHA-256 before rename.
- Duplicate naming: `unique_path` in `src-tauri/src/main.rs`.
- Chat command/event path: `send_chat` and `Message::Chat` in `src-tauri/src/main.rs`; `chat_history` loads SQLite records into the Messages page.
- Transfer history/settings/diagnostics: `transfer_history`, `settings`, `save_settings`, `open_location`, and the corresponding pages in `src/main.tsx`.
- Incoming approval: `transfer-request`, `accept_transfer`, and `reject_transfer` are wired through the existing framed protocol; configured download location is used for receives.
- Native drag/drop: Tauri webview drag/drop events in `src/main.tsx` call the same multi-file send command as the picker.
- Cancellation wiring: `cancel_transfer` command and `TransferCancel` protocol path now exist; UI exposes a cancel action.
- Separate control/data queues: `Session` in `src-tauri/src/main.rs` prioritizes chat/control frames over data frames.

## Partial or not implemented

- Trust is not authenticated: the custom TLS verifier accepts any certificate, and no keychain/DPAPI-backed key or pinning flow exists.
- Reconnect state machine is still incomplete; stale mDNS peers are removed and known peers can be rediscovered, but bounded reconnect behavior is not physically tested.
- Resume is not implemented.
- Trust/pinning, notifications, and resume remain incomplete. The Messages, Transfer History, Diagnostics, Settings, drag/drop, save-location, and incoming approval UI paths are now source-wired and packaged-navigation verified.
- Port fallback is now probed in `main`, but mDNS/manual-connect behavior under an occupied port is not yet tested.

## Actual test evidence

- `npm run build`: PASS after the relative Vite asset fix (`vite.config.ts`, `base: './'`).
- Packaged GUI: PASS — the actual `.app` opened with non-blank React UI; Messages, Transfer History, Settings, and Diagnostics were clicked and inspected.
- Package contents: PASS — `Contents/MacOS/localshare` is the only executable; `CFBundleExecutable=localshare`; its checksum matches the release binary.
- mDNS advertisement: PASS on this Mac — `dns-sd -B _localshare._tcp local.` observed the LocalShare UUID service; physical peer discovery remains unverified.
- `cargo check`: PASS after AWS-LC startup fix and control/data queue refactor.
- `cargo test --all-targets`: PASS, 4 protocol tests.
- `cargo run --features two-node --bin two_node`: PASS for real TLS chat A→B/B→A, 1 MiB A→B, SHA-256 equality; evidence: `artifacts/test-results/two-node-pass.txt`.
- `npm run tauri build` with normal environment: FAIL because Homebrew Rust’s `rust-objcopy` symlink targets a missing LLVM path.
- `scripts/build-macos.sh`: PASS — it supplies the Homebrew LLVM/Rust target PATH, builds the app, removes the test-only executable from the app payload, and creates a plain hdiutil DMG suitable for headless/offline sessions.
- Packaged runtime: PASS — native window, SQLite, listener, mDNS sockets, and React pages verified.
- Windows: hosted Windows workflow PASS on run `31724824519`; installer and installed-process smoke test passed. Physical Windows GUI/LAN behavior remains unverified.
- Physical LAN / mDNS / packet capture: not verified.

## Windows readiness conclusion

- Production binary selection is explicit: `src-tauri/Cargo.toml` maps `localshare` to `src-tauri/src/main.rs`; `two_node` requires the opt-in `two-node` feature; `src-tauri/tauri.conf.json` sets `mainBinaryName: localshare`.
- `scripts/build-windows.ps1` is prepared to build NSIS/MSI, validate genuine PE output, reject `two_node.exe`, normalize the primary installer to `release/windows/LocalShare-Setup.exe`, and emit checksums.
- The Windows config uses Tauri `offlineInstaller` WebView2 mode; hosted build, silent install, and process launch passed. User-machine Internet-off installation remains unverified.
- `dirs::data_local_dir()` and Windows `explorer` handling are present, and the LAN interface filter excludes common virtual adapters. Windows firewall, mDNS, DPAPI/credential storage, installed GUI, and Mac↔Windows transport remain unverified.
- Current status: `WINDOWS BUILD: PASS`; `WINDOWS EXE: PASS`; `WINDOWS INSTALLER: PASS` on GitHub Actions run `31724824519`. Windows GUI visual, physical LAN, and firewall behavior remain unverified.
