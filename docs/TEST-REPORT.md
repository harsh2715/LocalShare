# LocalShare test report

## Verified in this workspace

- `npm run build`: PASS.
- `cargo check --all-targets --manifest-path src-tauri/Cargo.toml`: PASS.
- `cargo test --all-targets --manifest-path src-tauri/Cargo.toml`: PASS, 5
  tests per production target.
- `LOCALSHARE_TEST_ROOT=.localshare-test-work/two-node cargo run --features
  two-node --bin two_node`: PASS for real TLS framing, bidirectional chat,
  1 MiB streaming transfer, and SHA-256 equality.
- Protocol tests: PASS for Unicode, malformed version/manifest, absolute and
  traversal paths, and normalized folder-relative paths.
- UI build wiring: PASS for Messages, history, settings, diagnostics,
  received-folder actions, folder selection, updater check, trust prompt, and
  transfer approval actions.
- Windows hosted workflow: previously PASS on run `31724824519`; a new run
  is required for this change set and must be read from Actions before a new
  installer status is claimed.

## Implemented but not physically verified

- Multi-file and folder manifest generation/receive path.
- Platform Downloads/LocalShare default and duplicate naming.
- Optional signed updater path and GitHub secret configuration.
- Persisted peer fingerprint check and first-contact trust UI.

## Not tested here

- Real Mac↔Windows mDNS, firewall prompts, Internet-off home-LAN behavior.
- 100 MB/1 GB physical transfer, chat during a physical large transfer,
  cancel/reconnect across Wi-Fi, and a real installed update/relaunch.
- Resume remains not implemented.
