# LocalShare release process

## Normal Windows build

The `LocalShare Windows release` workflow runs on `windows-latest` for pushes
to `main` and uploads the unsigned/installer release artifact used for CI
smoke validation. It does not claim physical LAN or GUI visual verification.

## Signed release and updater metadata

1. Update the version consistently in `package.json`, `src-tauri/Cargo.toml`,
   and `src-tauri/tauri.conf.json`.
2. Run the local frontend/Rust checks and the Windows workflow.
3. Commit and push the version change.
4. Create and push a matching tag, for example `git tag v0.1.1 && git push origin v0.1.1`.
5. `.github/workflows/release.yml` builds signed Windows and macOS updater
   bundles, creates `latest.json`, and publishes the tag release.

The workflow uses only the `TAURI_SIGNING_PRIVATE_KEY` secret configured on
`harsh2715/LocalShare`; the private key is never stored in Git. The public key
is embedded in `src-tauri/tauri.conf.json`.

Existing installations created before updater support was added cannot update
themselves. Install one updater-enabled release manually first; later signed
releases can then be detected by the optional updater.

## Offline behavior

The updater is not part of LAN startup, discovery, chat, or file I/O. If
Internet access is unavailable, update checks fail gracefully and LocalShare
continues to use direct mDNS/TLS LAN communication.
