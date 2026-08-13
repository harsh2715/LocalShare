# Platform Test Matrix

| Area | macOS | Windows |
|---|---|---|
| Rust compile | PASS on current macOS | NOT VERIFIED |
| Frontend build | PASS | build pipeline not run |
| Packaged app | PASS — `src-tauri/target/release/bundle/macos/LocalShare.app` | BLOCKED_BY_ENVIRONMENT |
| mDNS/firewall | Packaged mDNS sockets PASS; physical discovery NOT VERIFIED | NOT VERIFIED physically |
| File picker | UI path present | NOT VERIFIED |
| Local network permission | NOT VERIFIED | NOT VERIFIED |
