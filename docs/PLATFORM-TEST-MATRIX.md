# Platform Test Matrix

| Area | macOS | Windows |
|---|---|---|
| Rust compile | PASS on current macOS | BLOCKED_BY_ENVIRONMENT — no Windows guest/toolchain |
| Frontend build | PASS | build pipeline not run |
| Packaged app | PASS — app and mounted DMG contain only `Contents/MacOS/localshare` | BLOCKED_BY_ENVIRONMENT — no genuine Windows artifact |
| mDNS/firewall | Mac advertisement observed with `dns-sd`; physical discovery NOT VERIFIED | NOT VERIFIED — no Windows runtime |
| File picker | Packaged UI control present and wired | NOT VERIFIED on Windows |
| Local network permission | NOT VERIFIED | NOT VERIFIED — firewall behavior needs Windows |
