# Test Plan

1. Unit: protocol round-trip, Unicode, unsafe filenames.
2. Rust integration: listener, handshake, chat, bounded file stream, hash verification.
3. Virtual two-node: separate identities/databases and localhost ports.
4. LAN address: bind to private interface and connect from a second process/machine.
5. UI/package: file picker, drag/drop, macOS app, Windows installer.
6. Offline: packet capture with upstream Internet disabled.

Current automated evidence is limited to frontend production build, Rust check, and two Rust unit tests. Higher levels are `NOT VERIFIED`.
