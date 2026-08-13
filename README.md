# LocalShare

LocalShare is a native desktop LAN sharing app for direct file transfer and chat between nearby computers. It is designed to run without Internet, cloud signaling, public STUN/TURN, telemetry, or CDN assets.

## Current status

The existing V1 implementation builds as a native macOS app, advertises/browses the LAN with mDNS, automatically attempts the deterministic secure peer connection, and exposes clickable Messages, Transfer History, Settings, Diagnostics, incoming approval, picker, and native drag/drop paths. Full acceptance is not claimed yet: trust prompting/pinning, resumable transfers, transfer cancellation, large/multi-file runtime verification, Windows build verification, and physical two-computer verification remain incomplete or unverified.

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

Put both computers on the same Wi-Fi, Ethernet, or isolated hotspot LAN. Start LocalShare on both. They should appear through mDNS and automatically connect; manual IP is retained only in the Troubleshooting fallback if discovery is blocked. Runtime code contains no configured public endpoint or signaling server.

## Windows build and release

`.github/workflows/windows-release.yml` is the preferred Windows build path. It runs on GitHub’s `windows-latest` hosted runner and performs the locked frontend build, Rust checks/tests, Tauri NSIS packaging, PE validation, and artifact upload. The uploaded primary artifact is `LocalShare-Setup.exe`. `BUILD-WINDOWS.cmd` and `scripts/build-windows.ps1` remain local Windows developer/build helpers and are not required by the normal end user.

`RUN-LOCALSHARE.cmd` launches a prebuilt `LocalShare.exe` beside it using `%~dp0`. It is a portable fallback only; it is not the primary release artifact. The normal end user needs only `LocalShare-Setup.exe` and does not need Node.js, npm, Rust, Cargo, Git, or the source repository.

The Windows installer is configured for Tauri’s offline WebView2 installer mode so first installation can work without Internet. This mode has not been built or tested here because this Apple Silicon Mac has no Windows VM or Windows toolchain. See [the Windows build audit](/Users/harshsinghal/Desktop/Local%20Network/docs/WINDOWS-BUILD-AUDIT.md) and [the VM report](/Users/harshsinghal/Desktop/Local%20Network/docs/WINDOWS-VM-BUILD-REPORT.md).

## Security note

TLS encrypts the session, but the current development build uses a custom verifier that does not yet pin a persisted Ed25519 identity key or show a user confirmation dialog. See [docs/SECURITY.md](/Users/harshsinghal/Desktop/Local%20Network/docs/SECURITY.md) before treating this as production secure.

## Documentation

See the [final software report](/Users/harshsinghal/Desktop/Local%20Network/docs/FINAL-SOFTWARE-REPORT.md), [offline guarantee audit](/Users/harshsinghal/Desktop/Local%20Network/docs/OFFLINE-GUARANTEE.md), and [physical test procedure](/Users/harshsinghal/Desktop/Local%20Network/docs/PHYSICAL-TWO-COMPUTER-TEST.md).
