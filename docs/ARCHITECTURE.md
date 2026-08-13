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

The current session implementation uses one TLS connection per peer and a bounded sender channel. Incoming file data is streamed to disk. The port is 32123; the code currently does not yet implement automatic fallback to a different port when occupied.
