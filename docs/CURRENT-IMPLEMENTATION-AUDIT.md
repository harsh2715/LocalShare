# Current Implementation Audit

Audit date: 2026-08-13. This reflects source inspection and the verification commands recorded in `artifacts/test-results/`.

## Implemented and verified in source

- React/Vite UI: `src/main.tsx`, `src/styles.css`; `npm run build` PASS.
- Tauri shell: `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`; macOS app bundle built after fixing the local Rust/LLVM toolchain path.
- Local SQLite schema: `src-tauri/src/storage.rs`; app database opened at platform local-data `LocalShare/localshare.sqlite`.
- Persistent UUID: `src-tauri/src/identity.rs`; fingerprint is a SHA-256-derived UUID value, not a public-key identity.
- Framed protocol and validation: `src-tauri/src/protocol.rs`; unit tests now cover Unicode, traversal, Windows/absolute names, bad manifest, and version.
- Direct TLS listener/client: `src-tauri/src/main.rs`; TLS 1.3-capable rustls session with AWS-LC provider installed at startup.
- mDNS: `start_mdns` in `src-tauri/src/main.rs`; advertises and browses `_localshare._tcp.local.`.
- Bounded file I/O: sender uses 1 MiB buffers; receiver writes partial files and verifies SHA-256 before rename.
- Duplicate naming: `unique_path` in `src-tauri/src/main.rs`.
- Chat command/event path: `send_chat` and `Message::Chat` in `src-tauri/src/main.rs`.
- Cancellation wiring: `cancel_transfer` command and `TransferCancel` protocol path now exist; UI exposes a cancel action.
- Separate control/data queues: `Session` in `src-tauri/src/main.rs` prioritizes chat/control frames over data frames.

## Partial or not implemented

- Incoming requests are still auto-accepted in the Rust backend; no user accept/reject decision is wired.
- Trust is not authenticated: the custom TLS verifier accepts any certificate, and no keychain/DPAPI-backed key or pinning flow exists.
- Reconnect state machine is not complete; the session is marked offline, but retry and rediscovery need more testing/wiring.
- Resume is not implemented.
- Transfer history UI, chat history load/clear UI, diagnostics page, notifications, drag/drop, and save-location settings are incomplete.
- Port fallback is now probed in `main`, but mDNS/manual-connect behavior under an occupied port is not yet tested.

## Actual test evidence

- `npm run build`: PASS.
- `cargo check`: PASS after AWS-LC startup fix and control/data queue refactor.
- `cargo test --all-targets`: PASS, 4 protocol tests.
- `cargo run --bin two_node`: PASS for real TLS chat A→B/B→A, 1 MiB A→B, SHA-256 equality; evidence: `artifacts/test-results/two-node-pass.txt`.
- `npm run tauri build` with normal environment: FAIL because Homebrew Rust’s `rust-objcopy` symlink targets a missing LLVM path.
- `scripts/build-macos.sh`/explicit Homebrew LLVM+Rust target PATH: native `.app` and DMG were built, but initial runtime test exposed a Rustls crypto-provider panic; source fix is present and packaged runtime must be rebuilt/retested.
- Windows: not built; no Windows environment available.
- Physical LAN / mDNS / packet capture: not verified.
