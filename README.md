# LocalShare

LocalShare is a native desktop LAN sharing app for direct file transfer and chat between nearby computers. It is designed to run without Internet, cloud signaling, public STUN/TURN, telemetry, or CDN assets.

## Current status

The existing V1 implementation builds as a native macOS app, advertises/browses the LAN with mDNS, uses explicit first-contact fingerprint trust, and exposes clickable Messages, Transfer History, Settings, Diagnostics, incoming approval, file/folder pickers, and native drag/drop paths. The loopback harness now passes 100 MiB and 1 GiB streaming/hash checks plus chat in both directions during the 100 MiB stream. Full physical acceptance is not claimed yet: multi-file/folder batch E2E, transfer cancellation, reconnect, resumable transfers, real updater installation, and physical two-computer verification remain incomplete or unverified.

## Architecture

- Tauri 2 desktop shell with a local React/Vite frontend.
- Rust/Tokio listener on TCP 32123 with framed JSON messages.
- mDNS/DNS-SD service `_localshare._tcp.local.` for nearby peer discovery.
- Direct TLS 1.3 session over the LAN, with separate bounded message channels for control and transfers.
- Local SQLite state under the platform application-data directory.
- Files are written to the platform Downloads directory under `LocalShare` (macOS: `~/Downloads/LocalShare`; Windows: `%USERPROFILE%\\Downloads\\LocalShare`) through a `.localshare-part` file and renamed after SHA-256 verification. The destination is configurable in Settings.
- File and folder manifests use normalized relative paths; symlinks and traversal are rejected. Existing names are collision-safe (`name (1).ext`).

## Development

```bash
npm install
npm run build
cd src-tauri && cargo test && cargo check
```

For a development window, install the Tauri CLI and run `npm run tauri dev`. Packaging requires the native macOS/Windows build prerequisites for the target platform.

## Storage and updates

Backend state is stored under the platform application-data directory in
`LocalShare/localshare.sqlite` plus the persistent `device-id`. Received files
are separate user data in `Downloads/LocalShare` by default. See
`docs/BACKEND-STORAGE.md` for the exact platform paths and persistence rules.

Software updates are optional and Internet-only. The updater checks the
authorized GitHub Releases metadata, verifies Tauri-signed artifacts, and
relaunches after installation. If the Internet is unavailable, the check is
skipped/fails gracefully and direct LAN discovery, chat, file, and folder
transfers remain local and usable.

## Offline use

Put both computers on the same Wi-Fi, Ethernet, or isolated hotspot LAN. Start LocalShare on both. They should appear through mDNS and automatically connect; manual IP is retained only in the Troubleshooting fallback if discovery is blocked. Runtime code contains no configured public endpoint or signaling server.

## Windows build and release

`.github/workflows/windows-release.yml` is the preferred Windows build path. It runs on GitHub’s `windows-latest` hosted runner and performs the locked frontend build, Rust checks/tests, Tauri NSIS packaging, PE validation, and artifact upload. The uploaded primary artifact is `LocalShare-Setup.exe`. `BUILD-WINDOWS.cmd` and `scripts/build-windows.ps1` remain local Windows developer/build helpers and are not required by the normal end user.

`RUN-LOCALSHARE.cmd` launches a prebuilt `LocalShare.exe` beside it using `%~dp0`. It is a portable fallback only; it is not the primary release artifact. The normal end user needs only `LocalShare-Setup.exe` and does not need Node.js, npm, Rust, Cargo, Git, or the source repository.

The Windows installer is configured for Tauri’s offline WebView2 installer mode. GitHub Actions run `31748837359` built it, validated both PE files, silently installed it, launched the installed process, generated signed updater artifacts, and uploaded `LocalShare-Setup.exe`. Physical Windows GUI/LAN/firewall and Internet-off installation tests remain separate. See [the Windows build audit](/Users/harshsinghal/Desktop/Local%20Network/docs/WINDOWS-BUILD-AUDIT.md) and [the final report](/Users/harshsinghal/Desktop/Local%20Network/docs/FINAL-SOFTWARE-REPORT.md).

## Security note

TLS encrypts the session. First-seen peers require explicit trust, trusted
fingerprints are persisted in SQLite, and a changed fingerprint is rejected.
The certificate key itself remains process-generated in this V1; see
[docs/SECURITY.md](/Users/harshsinghal/Desktop/Local%20Network/docs/SECURITY.md)
for the exact boundary.

## Documentation

See the [final software report](/Users/harshsinghal/Desktop/Local%20Network/docs/FINAL-SOFTWARE-REPORT.md), [offline guarantee audit](/Users/harshsinghal/Desktop/Local%20Network/docs/OFFLINE-GUARANTEE.md), and [physical test procedure](/Users/harshsinghal/Desktop/Local%20Network/docs/PHYSICAL-TWO-COMPUTER-TEST.md).
