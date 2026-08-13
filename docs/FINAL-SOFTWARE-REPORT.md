# LocalShare Final Software Report

## Final Status

PARTIAL — the repository now has a compiling and packaged macOS Tauri/Rust/React implementation, a real TLS two-node harness with passing chat/single-file/hash coverage, and honest remaining gaps around trust, multi-file UI, cancellation/reconnect/resume coverage, and physical LAN verification.

## What Was Built

Local UI, direct TCP/TLS session framing, mDNS advertisement/browse, persistent UUID, SQLite schema, chat command/event path, file manifest/chunk path, SHA-256 verification, safe partial files, duplicate-name helper, manual IP connection, and offline static audit.

## PairDrop Concepts Studied / Major Differences

The peer list, transfer manifest, progress, and chat concepts were studied from PairDrop’s public project. LocalShare does not copy source or branding and replaces browser signaling/WebRTC/STUN/TURN/WebSocket fallback with native direct LAN transport.

## Verification

- BUILD: PASS — `npm run build`, `cargo check`, `cargo test --all-targets`
- MACOS PACKAGE: PASS — `src-tauri/target/release/bundle/macos/LocalShare.app`
- PACKAGED MAC RUNTIME: PASS for backend startup — process stays alive, SQLite opens, TCP 32123 listens, mDNS sockets open; GUI visual/permission test not independently verified
- WINDOWS BUILD: BLOCKED_BY_ENVIRONMENT — script added, no Windows environment
- MDNS DISCOVERY: NOT PHYSICALLY VERIFIED — mDNS source path and sockets present; `dns-sd` browse produced no service evidence here
- MANUAL IP CONNECTION: SOURCE IMPLEMENTED, NOT VERIFIED across two machines
- OFFLINE STATIC AUDIT: PASS
- OFFLINE RUNTIME AUDIT: PASS for limited LocalShare-owned socket audit; evidence `artifacts/test-results/offline-runtime-audit.txt`
- CHAT A → B: PASS in real TLS two-node harness
- CHAT B → A: PASS in real TLS two-node harness
- SINGLE FILE A → B: PASS in real TLS two-node harness
- SINGLE FILE B → A: NOT VERIFIED
- MULTIPLE FILES: NOT VERIFIED
- LARGE FILE: NOT VERIFIED
- CHAT DURING FILE TRANSFER: NOT VERIFIED in production harness
- SHA-256 INTEGRITY: PASS for harness 1 MiB transfer
- DUPLICATE FILE POLICY: UNIT/SOURCE IMPLEMENTED, not end-to-end verified
- PATH TRAVERSAL PROTECTION: PASS unit tests for Unix/Windows/absolute traversal forms
- CANCEL: SOURCE/UI WIRING IMPLEMENTED, NOT VERIFIED end-to-end
- RECONNECT: NOT IMPLEMENTED/VERIFIED as a complete state machine
- RESUME: NOT IMPLEMENTED
- VIRTUAL TWO-NODE TEST: PARTIAL/PASS for TLS chat + single file + hash; remaining scenarios not covered
- REAL LAN-IP TEST: NOT VERIFIED
- PHYSICAL TWO-COMPUTER TEST: NOT VERIFIED

## Security and Reliability

Transport encryption and file integrity checks are present. Authenticated trust, secure key storage, multi-file/cancel/reconnect/resume verification remain incomplete; see [SECURITY.md](/Users/harshsinghal/Desktop/Local%20Network/docs/SECURITY.md) and [KNOWN-LIMITATIONS.md](/Users/harshsinghal/Desktop/Local%20Network/docs/KNOWN-LIMITATIONS.md).

## Exact Commands

```bash
cd "/Users/harshsinghal/Desktop/Local Network"
npm install
npm run build
cd src-tauri && cargo test && cargo check
cargo run --bin two_node
cd ..
scripts/build-macos.sh
```

## User Test

Follow [PHYSICAL-TWO-COMPUTER-TEST.md](/Users/harshsinghal/Desktop/Local%20Network/docs/PHYSICAL-TWO-COMPUTER-TEST.md). Do not treat this build as production secure until the trust/pinning and packaging gaps are closed.
