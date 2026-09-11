# LocalShare

LocalShare is a desktop app for sending files, folders, and messages directly between nearby computers. Transfers stay on your local Wi-Fi or Ethernet network—there is no account, cloud upload, telemetry, or Internet dependency for sharing.

## Quick start

1. Install and open LocalShare on both computers.
2. Connect both computers to the same Wi-Fi, Ethernet network, or personal hotspot.
3. Wait a few seconds for the other computer to appear under **Nearby devices**.
4. Select it and compare the identity fingerprint shown on both computers.
5. Approve the device once, then send a message, choose files, choose a folder, or drag files into the conversation.

Received files are saved to `Downloads/LocalShare` by default. You can change this under **Settings → Received files**.

> Keep LocalShare open on both computers until the transfer finishes. Sleep, a network change, or closing either app interrupts the current V1 transfer.

## Install

### Windows

Use `LocalShare-Setup.exe` from the latest release. The installer includes the WebView2 bootstrapper, so end users do not need Node.js, Rust, Git, or the source code.

If Windows Firewall asks for access, allow LocalShare on **Private networks** so nearby-device discovery can work.

### macOS

Open the release DMG and move LocalShare to Applications. On first launch, macOS may ask for local-network access; allow it so LocalShare can discover nearby computers.

## If a device does not appear

- Confirm both computers are on the same network and are not using a guest network with device isolation.
- Keep LocalShare open on both computers for at least 10 seconds.
- Allow LocalShare through the operating-system firewall on private/local networks.
- Temporarily disconnect a VPN if it blocks local-network traffic.
- Open **Diagnostics** on both computers and confirm that the LAN listener and device discovery are active.
- As a fallback, choose **Connect by IP** and enter the LAN address and port shown in Diagnostics on the other computer.

## Privacy and safety

- Sharing uses a direct TLS 1.3 connection over the LAN.
- New devices require explicit fingerprint approval.
- Incoming transfers can require approval before anything is written.
- Files are streamed to a temporary `.localshare-part` file, size- and SHA-256-verified, then moved to their final name.
- Unsafe paths, traversal, symbolic links, oversized frames, and invalid manifests are rejected.
- Chat, trust, settings, and transfer history are stored locally in SQLite.
- Internet access is used only for optional signed update checks.

See [Security](docs/SECURITY.md), [Offline guarantee](docs/OFFLINE-GUARANTEE.md), and [Protocol](docs/PROTOCOL.md) for the exact boundaries.

## Current V1 limits

Physical Mac↔Windows LAN verification is still required before declaring broad production readiness. Resume-after-restart is not implemented, and cancellation/reconnect plus multi-file/folder transfers still need full physical-device acceptance testing. See [Known limitations](docs/KNOWN-LIMITATIONS.md) and the [Physical two-computer test](docs/PHYSICAL-TWO-COMPUTER-TEST.md).

## Development

Prerequisites: Node.js/npm, the Rust toolchain, and the platform requirements from the Tauri documentation.

```bash
npm ci
npm run build
cargo check --all-targets --manifest-path src-tauri/Cargo.toml
cargo test --all-targets --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

The preferred Windows release path is [`.github/workflows/windows-release.yml`](.github/workflows/windows-release.yml). Packaging and release details are in [docs/RELEASE.md](docs/RELEASE.md) and [docs/RELEASE-CHECKLIST.md](docs/RELEASE-CHECKLIST.md).

## Architecture

- React/Vite interface inside a Tauri 2 desktop shell.
- Rust/Tokio listener on TCP port `32123`, with automatic fallback when occupied.
- mDNS/DNS-SD discovery using `_localshare._tcp.local.`.
- Direct TLS 1.3 sessions with separate bounded control and transfer channels.
- Local SQLite state under the platform application-data directory.

For more detail, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and [docs/BACKEND-STORAGE.md](docs/BACKEND-STORAGE.md).
