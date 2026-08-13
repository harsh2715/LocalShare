# Test Plan

1. Unit: protocol round-trip, Unicode, unsafe filenames.
2. Rust integration: listener, handshake, chat, bounded file stream, hash verification.
3. Virtual two-node: separate identities/databases and localhost ports.
4. LAN address: bind to private interface and connect from a second process/machine.
5. UI/package: file picker, drag/drop, macOS app, Windows installer.
6. Offline: packet capture with upstream Internet disabled.

Current evidence includes the frontend production build, Rust check/tests, real TLS two-node chat/single-file/hash harness (`cargo run --features two-node --bin two_node`), packaged macOS app/DMG inspection, packaged GUI navigation, and a Mac mDNS advertisement observation. Physical Mac↔Windows discovery, multi-file/large-file/cancel/reconnect/resume, and packet capture remain `NOT VERIFIED`.
