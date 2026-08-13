# Dependency Audit

| Dependency | Role | License / status |
|---|---|---|
| Tauri 2 | Desktop shell | MIT/Apache ecosystem; build checked |
| React, Vite, TypeScript | Local UI | MIT; bundled locally |
| Tokio | Async runtime | MIT |
| rustls, tokio-rustls | TLS transport | Apache-2.0/MIT; compiled |
| mdns-sd | mDNS/DNS-SD | MIT; compiled |
| rusqlite (bundled SQLite) | Local database | MIT; compiled |
| serde / serde_json | Protocol encoding | Apache-2.0/MIT |
| sha2 / hex | File integrity | MIT/Apache-2.0 |
| rcgen | Development certificates | MIT/Apache-2.0 |

`package-lock.json` and `src-tauri/Cargo.lock` record resolved versions. No CDN or runtime Internet dependency is intentionally used.
