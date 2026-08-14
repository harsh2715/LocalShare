# LocalShare test report

## Verified in this workspace

- `npm run build`: PASS.
- `cargo check --all-targets --manifest-path src-tauri/Cargo.toml`: PASS.
- `cargo test --all-targets --manifest-path src-tauri/Cargo.toml`: PASS, 6
  protocol tests in the library target and 9 tests in the production binary
  target (including receive-path, duplicate-name, and nested/empty-folder
  helper tests).
- `LOCALSHARE_TEST_ROOT=.localshare-test-work/two-node cargo run --features
  two-node --bin two_node`: PASS for real TLS framing, bidirectional chat,
  streaming transfer, and SHA-256 equality.
- `LOCALSHARE_TEST_SIZE_MB=100`: PASS for a 100 MiB streaming transfer,
  SHA-256 equality, and chat A↔B while the transfer was active.
- `LOCALSHARE_TEST_SIZE_MB=1024`: PASS for a 1 GiB streaming transfer and
  SHA-256 equality in the loopback harness. Evidence: `artifacts/test-results/
  two-node-large-files.txt`.
- Protocol tests: PASS for Unicode, malformed version/manifest, absolute and
  traversal paths, and normalized folder-relative paths.
- UI build wiring: PASS for Messages, history, settings, diagnostics,
  received-folder actions, folder selection, updater check, trust prompt, and
  transfer approval actions.
- Windows hosted workflow: PASS on run `31748837359`; Rust check/tests,
  Windows two-node transport, NSIS packaging, PE/hash validation, silent
  install/process launch, signed updater artifact generation, and artifact
  upload all passed.

## Implemented but not physically verified

- Multi-file and folder manifest generation/receive path, including empty
  directories, safe relative paths, duplicate names, and symlink rejection.
- Platform Downloads/LocalShare default and duplicate naming.
- Optional signed updater path and GitHub secret configuration.
- Persisted peer fingerprint check and first-contact trust UI.

## Not tested here

- Real Mac↔Windows mDNS, firewall prompts, Internet-off home-LAN behavior.
- Multi-file/folder end-to-end batch over two production GUI nodes, cancel,
  reconnect, physical 100 MiB/1 GiB transfer, and a real installed
  update/relaunch remain untested.
- Resume remains not implemented.
