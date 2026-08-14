# Automated Test Report

Date: 2026-08-14

- `npm run build`: PASS
- `cargo check --all-targets` in `src-tauri`: PASS
- `cargo test --all-targets`: PASS — protocol assertions across lib/main targets, 0 failed
- Frontend typecheck: PASS as part of build
- `cargo run --features two-node --bin two_node`: PASS — real TLS framing, chat A→B/B→A, chat during transfer, streaming A→B, SHA-256 equality
- File matrix generator: PRESENT, not executed at scale
- Virtual two-node transfer: PASS for the scenarios in the harness — real single-file/chat/TLS/SHA-256; multi-file/cancel/reconnect are not covered by the current harness
- LAN-address transfer: NOT VERIFIED
- 100 MiB transfer: PASS in the macOS loopback harness and Windows hosted runner; 1 GiB PASS in the macOS loopback harness
- Cancellation: source/UI wiring covers sender and receiver cancellation, but end-to-end transfer test is NOT VERIFIED
- Reconnect/resume: NOT VERIFIED / resume NOT IMPLEMENTED
- Packaged executable startup: PASS — stays alive, opens SQLite, binds TCP 32123, opens mDNS sockets; packaged GUI navigation was also inspected
- Windows hosted release: PASS — run `31783746842` passed Windows Rust check/tests, the Windows two-node TLS/chat/100 MiB streaming/chat-during-transfer/SHA-256 harness, Tauri NSIS build, PE/hash validation, silent install, installed-process launch, signed updater artifact generation, and artifact upload. GUI visual and physical LAN tests remain unverified.
