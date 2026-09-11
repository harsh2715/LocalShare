# LocalShare

LocalShare is a standalone Tauri desktop application for direct, encrypted device-to-device chat and file sharing over a reachable local network. It has no cloud, account, signaling, STUN, TURN, WebRTC, or relay dependency.

## Stack

- Tauri v2 desktop shell
- React 19, TypeScript, Vite
- Reusable Rust `localshare-core` for identity, TCP/TLS, protocol, SQLite, pairing, chat, transfer, and discovery
- Rust `localshare-test-node` for headless dual-device testing

Electron is not part of this project.

## Build and test

```bash
npm install
npm run typecheck
npm run build
npm test
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
```

Build the macOS application bundle:

```bash
npm run tauri:build:app
```

The bundle is written to `target/debug/bundle/macos/LocalShare.app` for the debug app build. Signing and notarization are separate release steps.

## Virtual lab

```bash
/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab/scripts/start-both.sh
/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab/scripts/status.sh
/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab/scripts/stop-all.sh
/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab/scripts/reset-lab.sh
```

The two headless nodes use independent profiles, identities, SQLite databases, download roots, temporary roots, and logs. The normal lab uses `127.0.0.1:33201/34201` for A and `127.0.0.1:33202/34202` for B. This is single-machine dual-instance verification, not two physical computers.

## Current verified scope

The current runtime evidence covers startup health, TCP in both directions, TLS and fingerprint rejection, HELLO, heartbeat, control-message ACKs, reconnect cycles, persistent pairing, persistent chat and queued chat, duplicate-safe file naming/content policy, size ladder through 100 MB, LAN-address protocol operation on `192.168.1.6`, and sender/receiver crash recovery with matching SHA-256.

The current implementation does not yet provide user pause/resume, transport throttling/fault injection, folder manifests, or a completed Windows build. Native mDNS registration starts, but same-host Bonjour visibility was not proven in this environment and is reported as blocked/partial.

See the linked audit and release documents in this repository for exact evidence and PASS/FAIL/BLOCKED/NOT IMPLEMENTED classifications.

## Security notes

Production Tauri identity private keys use the OS keychain through `keyring`; only public identity metadata is written to the app-data JSON. The test node intentionally uses `0600` plaintext generated identity files inside the isolated lab. Peer trust is fingerprint-based and persisted; discovery alone does not trust a peer. Received names are constrained to a single safe filename and completed files are collision-safe and hash-verified before rename.

