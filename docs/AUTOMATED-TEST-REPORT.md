# Automated Test Report

Date: 2026-08-13

- `npm run build`: PASS
- `cargo check` in `src-tauri`: PASS (one dead-code warning)
- `cargo test --all-targets`: PASS — 8 protocol assertions across lib/main targets, 0 failed
- Frontend typecheck: PASS as part of build
- `cargo run --bin two_node`: PASS — real TLS framing, chat A→B/B→A, 1 MiB A→B, SHA-256 equality
- File matrix generator: PRESENT, not executed at scale
- Virtual two-node transfer: PARTIAL — real single-file/chat harness implemented and passing; multi-file/cancel/reconnect not covered
- LAN-address transfer: NOT VERIFIED
- 100 MB transfer: NOT VERIFIED
- Cancellation/reconnect/resume: NOT IMPLEMENTED or NOT VERIFIED
- Packaged executable startup: PASS — stays alive, opens SQLite, binds TCP 32123, opens mDNS sockets
