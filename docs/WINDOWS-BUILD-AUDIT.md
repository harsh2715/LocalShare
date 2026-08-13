# Windows Build Audit

Audit date: 2026-08-13. This is an evidence-based source and host audit; no Windows executable or installer is claimed from this macOS host.

## Production binary configuration

- `src-tauri/Cargo.toml` declares the production binary explicitly as `localshare` at `src-tauri/src/main.rs`.
- The test harness is `two_node` at `src-tauri/src/bin/two_node.rs` and is gated behind the opt-in `two-node` Cargo feature. A default production build does not enable that feature.
- `src-tauri/tauri.conf.json` sets `mainBinaryName` to `localshare` and `productName` to `LocalShare`.
- `scripts/build-windows.ps1` explicitly builds the `x86_64-pc-windows-msvc` production target, rejects `two_node.exe` in the bundle, validates the production file as x86_64 PE, and copies the NSIS setup executable to `release/windows/LocalShare-Setup.exe`.
- `.github/workflows/windows-release.yml` runs this pipeline on GitHub’s `windows-latest` hosted runner: exact checkout, Node `.nvmrc`, `npm ci`, frontend build, stable Rust/MSVC target setup, Rust checks/tests, NSIS packaging, PE checks, SHA-256, and artifact upload.

## Cross-platform source audit

- Frontend assets use relative Vite URLs in `vite.config.ts`, which is required by the packaged Tauri asset scheme.
- Application data and SQLite use `dirs::data_local_dir()` in `src-tauri/src/storage.rs`; this resolves to the platform-local application-data directory rather than a macOS-only path.
- `open_location` in `src-tauri/src/main.rs` uses `explorer` under `target_os = "windows"` and `open` under macOS.
- `private_addresses` in `src-tauri/src/main.rs` prefers Wi-Fi/Ethernet-style interfaces, private IPv4 addresses, and usable IPv6, while excluding common Docker, WSL, Hyper-V, VMware, VirtualBox, VPN, virtual, and loopback names.
- mDNS is implemented by `mdns-sd` in `start_mdns`; the advertised service is `_localshare._tcp.local.`. Windows firewall/multicast behavior still requires a real Windows runtime test.
- There is no macOS Keychain API in the Windows path. The current identity file is a persistent UUID, not a private key. Cryptographic key persistence and certificate pinning remain incomplete; see `docs/SECURITY.md`.
- File picker, drag/drop, transfer approval, chat, history, settings, diagnostics, and manual-IP troubleshooting fallback are wired in `src/main.tsx` and the Tauri commands in `src-tauri/src/main.rs`. Their Windows GUI behavior is not verified without Windows.

## Windows installer strategy

- The primary installer is NSIS, normalized to `release/windows/LocalShare-Setup.exe` by `scripts/build-windows.ps1`.
- An MSI is also requested on Windows where WiX is available; it is optional and is copied as `release/windows/LocalShare.msi` only when actually emitted.
- `src-tauri/tauri.conf.json` uses Tauri `offlineInstaller` WebView2 mode. This is intentionally selected for no-Internet installation, but it increases the installer substantially and has not been built or installed in this environment.
- The generated Windows release also includes a PE-validated `LocalShare.exe`, `RUN-LOCALSHARE.cmd`, `README-WINDOWS.txt`, and `SHA256SUMS.txt` only after a real Windows build.

## Host evidence and blocker

Host: Apple Silicon `MacBookAir10,1`, macOS 26.5.1, 8 CPU cores, 8 GiB RAM, approximately 45 GiB free on the system volume.

Checked on 2026-08-13:

- no installed VMware Fusion, Parallels Desktop, or VirtualBox application at initial audit; official UTM 4.7.5 was subsequently installed from the Homebrew cask, but no guest was created;
- no Windows VM disk or Windows ISO found under the project/user search scope;
- no `x86_64-w64-mingw32-gcc`, `zig`, `cargo-xwin`, `cargo-tauri`, or `rustup` executable;
- the host Rust installation has no installed `x86_64-pc-windows-msvc` standard library;
- `cargo build --target x86_64-pc-windows-msvc --release --bin localshare` failed with `can't find crate for core/std` and reported that the target is not installed.

The local-host result is:

```text
WINDOWS BUILD: NOT VERIFIED — hosted workflow configured but not executed
WINDOWS EXE: NOT VERIFIED — hosted workflow configured but not executed
WINDOWS INSTALLER: NOT VERIFIED — hosted workflow configured but not executed
WINDOWS GUI: NOT VERIFIED
```

The local Mac cannot execute the Windows target, but this is no longer a reason to create a VM. The valid build path is `.github/workflows/windows-release.yml` on GitHub’s `windows-latest` hosted runner. Windows runtime, LAN, and physical Mac↔Windows testing remain separate.

The preferred Windows build path is now GitHub Actions, not a local VM. This checkout has no Git remote, so the workflow cannot run or produce an artifact download URL until the repository is pushed to a GitHub repository where Actions is enabled. After a run, download the `LocalShare-Setup` artifact from that workflow’s Actions run page; the workflow summary and uploaded `LocalShare-Windows-release-metadata` artifact contain the exact SHA-256.
