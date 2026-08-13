# LocalShare

LocalShare is a native desktop LAN sharing app for direct file transfer and chat between nearby computers. It is designed to run without Internet, cloud signaling, public STUN/TURN, telemetry, or CDN assets.

## Current status

This repository contains an early V1 implementation. The Rust backend and React UI build locally; protocol unit tests pass. Full acceptance is not claimed yet: trust prompting/pinning, resumable transfers, transfer cancellation, chat/file history UI, OS notifications, packaged macOS runtime, Windows packaging, and physical two-computer verification remain incomplete or unverified.

## Architecture

- Tauri 2 desktop shell with a local React/Vite frontend.
- Rust/Tokio listener on TCP 32123 with framed JSON messages.
- mDNS/DNS-SD service `_localshare._tcp.local.` for nearby peer discovery.
- Direct TLS 1.3 session over the LAN, with separate bounded message channels for control and transfers.
- Local SQLite state under the platform application-data directory.
- Files are written to `Downloads/LocalShare` through a `.localshare-part` file and renamed after SHA-256 verification.

## Development

```bash
npm install
npm run build
cd src-tauri && cargo test && cargo check
```

For a development window, install the Tauri CLI and run `npm run tauri dev`. Packaging requires the native macOS/Windows build prerequisites for the target platform.

## Offline use

Put both computers on the same Wi-Fi, Ethernet, or isolated hotspot LAN. Start LocalShare on both. They should appear through mDNS; if discovery is blocked, use the other device's displayed private address and port 32123 via “Connect manually”. Runtime code contains no configured public endpoint or signaling server.

## Security note

TLS encrypts the session, but the current development build uses a custom verifier that does not yet pin a persisted Ed25519 identity key or show a user confirmation dialog. See [docs/SECURITY.md](/Users/harshsinghal/Desktop/Local%20Network/docs/SECURITY.md) before treating this as production secure.

## Documentation

See the [final software report](/Users/harshsinghal/Desktop/Local%20Network/docs/FINAL-SOFTWARE-REPORT.md), [offline guarantee audit](/Users/harshsinghal/Desktop/Local%20Network/docs/OFFLINE-GUARANTEE.md), and [physical test procedure](/Users/harshsinghal/Desktop/Local%20Network/docs/PHYSICAL-TWO-COMPUTER-TEST.md).
