# Architecture

The Tauri window is presentation only. Rust owns identity, listener lifecycle, discovery, sessions, protocol validation, file I/O, and hashing.

```text
React UI → Tauri commands/events → Rust state
                                  ├─ mDNS advertise/browse
                                  ├─ TLS 1.3 LAN listener
                                  ├─ framed protocol/session
                                  ├─ bounded file streaming
                                  └─ SQLite local state
```

The current session implementation uses one TLS connection per peer and
separate bounded control/data channels so chat and heartbeat frames are not
queued behind file chunks. Incoming file data is streamed to a temporary file,
validated with SHA-256, and atomically renamed into the configured receive
directory. File and folder manifests carry safe normalized relative paths.

The port is 32123 with a fallback probe when occupied. First-seen fingerprints
require explicit trust; trusted fingerprints are stored in SQLite. The updater
is a separate optional Internet-only Tauri plugin pointed at the LocalShare
GitHub Release metadata and signed with a private CI key.
