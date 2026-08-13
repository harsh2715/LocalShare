# Current LocalShare project audit

Audit date: 2026-08-14. Repository root: `/Users/harshsinghal/Desktop/Local Network`.
Origin: `https://github.com/harsh2715/LocalShare.git`. Branch: `main`.

## Source of truth

| Area | Source | Current finding |
| --- | --- | --- |
| Frontend | `src/main.tsx`, `src/styles.css`, `vite.config.ts` | React/Vite UI builds locally; navigation, chat, settings, history, diagnostics, folder action, trust prompt, and updater UI are wired. |
| Tauri | `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json` | Tauri v2; production binary is explicitly `localshare`; signed updater artifacts are enabled. |
| Rust entrypoint | `src-tauri/src/main.rs` | Listener, mDNS, TLS, session queues, commands, transfer streaming, receive approval, folder manifests, safe destinations, trust checks. |
| Protocol | `src-tauri/src/protocol.rs` | Framed JSON with bounded frames; 64-bit file sizes; safe filename/relative-path validation; backward-compatible directory list. |
| Storage | `src-tauri/src/storage.rs` | SQLite in platform local app data; defaults/migration for Downloads/LocalShare; settings, trust, chat, and history tables. |
| Identity | `src-tauri/src/identity.rs` | Persistent UUID in app data; advertised fingerprint is derived from the UUID. Certificate key is still process-generated. |
| Test harness | `src-tauri/src/bin/two_node.rs` | Real TLS loopback harness; currently verifies bidirectional chat, 1 MiB streaming transfer, and SHA-256. |
| Windows CI | `.github/workflows/windows-release.yml` | Hosted Windows check/test/package/PE/install smoke path; signed updater environment is configured. |
| Signed releases | `.github/workflows/release.yml`, `scripts/create-update-manifest.py` | Tag-triggered Windows/macOS signed bundles plus GitHub Release metadata. |

## Verified commands

- `npm run build`: PASS.
- `cargo check --all-targets --manifest-path src-tauri/Cargo.toml`: PASS.
- `cargo test --all-targets --manifest-path src-tauri/Cargo.toml`: PASS, 5 tests per target.
- `LOCALSHARE_TEST_ROOT=.localshare-test-work/two-node cargo run --features two-node --bin two_node --manifest-path src-tauri/Cargo.toml`: PASS for real TLS chat both ways, single-file stream, and SHA-256.
- `./scripts/build-macos.sh`: PASS; packaged app contains only `Contents/MacOS/localshare`, and the app binary checksum matches `target/release/localshare`.

## Not physically verified

Real Mac↔Windows mDNS, Windows firewall behavior, Internet-off home-LAN
operation, 100 MB/1 GB physical transfer, chat during a physical large
transfer, cancel/reconnect on Wi-Fi, and a real installed update remain
unverified. Resume remains not implemented.
