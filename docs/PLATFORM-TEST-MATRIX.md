# Platform Test Matrix

| Area | macOS | Windows |
|---|---|---|
| Rust compile | PASS on current macOS | NOT VERIFIED |
| Frontend build | PASS | build pipeline not run |
| Packaged app | BLOCKED_BY_ENVIRONMENT (`rust-objcopy` missing during `npm run tauri build`) | BLOCKED_BY_ENVIRONMENT |
| mDNS/firewall | NOT VERIFIED physically | NOT VERIFIED physically |
| File picker | UI path present | NOT VERIFIED |
| Local network permission | NOT VERIFIED | NOT VERIFIED |
