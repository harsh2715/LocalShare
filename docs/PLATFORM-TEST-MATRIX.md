# Platform Test Matrix

| Area | macOS | Windows |
|---|---|---|
| Rust compile | PASS on current macOS | PASS on GitHub Windows runner |
| Frontend build | PASS | build pipeline not run |
| Packaged app | PASS — app and mounted DMG contain only `Contents/MacOS/localshare` | PASS installer/process smoke test; GUI visual NOT VERIFIED |
| mDNS/firewall | Mac advertisement observed with `dns-sd`; physical discovery NOT VERIFIED | NOT VERIFIED — no Windows runtime |
| File picker | Packaged UI control present and wired | NOT VERIFIED visually on Windows |
| Local network permission | NOT VERIFIED | NOT VERIFIED — hosted runner cannot validate user firewall behavior |
