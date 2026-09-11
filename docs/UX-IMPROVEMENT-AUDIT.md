# LocalShare usability improvement audit

Date: 2026-09-06

## Scope reviewed

- React/Tauri navigation, discovery, approval, chat, file/folder sending, transfer progress, history, settings, diagnostics, updates, and error states.
- Rust commands, transfer lifecycle, mDNS/TLS session wiring, SQLite-backed settings/history/trust, path validation, and developer startup.
- End-user and developer setup documentation.

## Problems corrected

1. `tauri dev` could not start reliably because Tauri expected port 1420 while Vite used 5173, and Cargo had two binaries with no `default-run`.
2. Incoming approval UI was emitted even when **Ask before receiving** was disabled, leaving a dialog whose request was not pending.
3. Transfer completion was emitted per file with partial payloads, which could create duplicate or malformed progress cards for a multi-file transfer.
4. Concurrent sends to the same peer could interleave over the shared transfer channel. The backend now allows one active outbound transfer per peer and returns a clear error for a second attempt.
5. Incoming approval and device-trust events could overwrite one another. The UI now queues them.
6. The Messages page lost access to saved conversations after a peer disappeared. Trusted device names are now loaded from SQLite for offline conversations.
7. Device-name, message-length, IP, and port errors could surface as low-level backend text. Validation is now enforced at both UI and command boundaries with actionable messages.
8. The Notifications preference had no runtime implementation. It is now backed by the official Tauri notification plugin and permission-gated.
9. The 900 px minimum window could be narrower than the old two-column content minimum. The workspace now switches to one column before that point.
10. Several destructive, loading, offline, and keyboard states were ambiguous. The redesign adds clear confirmation, busy/disabled states, focus styles, live status messages, dialog semantics, and reduced-motion support.

## Usability additions

- Three-step first-use guidance and a clearer Connect-by-IP fallback.
- Connection health summary with per-service status.
- Full fingerprint review/copy UI before trust.
- Drag-and-drop destination overlay.
- Transfer status labels, safe cancellation, and automatic cleanup of completed cards.
- Friendly empty states and remembered offline conversations.
- Searchable conversations and peer names in transfer history.
- Read-only download path with a native folder picker.
- Real desktop notification support for completed incoming transfers.
- A quick-start, install, firewall/VPN troubleshooting, privacy, and V1-limits rewrite of the README.

## Verification completed

- `npm run build`
- `cargo check --all-targets --manifest-path src-tauri/Cargo.toml`
- `cargo test --all-targets --manifest-path src-tauri/Cargo.toml`
- `LOCALSHARE_TEST_SIZE_MB=1 cargo run --features two-node --bin two_node --manifest-path src-tauri/Cargo.toml`
- `npm run tauri dev` native launch smoke test
- Browser accessibility-tree and visual inspection of Nearby, Settings, Diagnostics, and Connect-by-IP validation
- `cargo fmt` and `git diff --check`

## Still requires physical acceptance

- Mac↔Windows discovery and firewall behavior on real machines.
- Multi-file/folder, cancel, reconnect, sleep/wake, and network-change behavior across two physical computers.
- Internet-disconnected installer behavior and a real signed version-A → version-B updater run.
- Persistent certificate/public-key pinning. The current trust fingerprint is persistent but UUID-derived; see `docs/SECURITY.md`.
- Resume after app restart is not implemented.
