# Virtual Two-Node Test

Status: IMPLEMENTED and passing locally with the `two_node` feature.

The harness creates isolated source/destination directories and two signed Ed25519 identities over a TLS 1.3 loopback connection. It verifies both handshake directions, bidirectional chat, chat during a streamed file transfer, SHA-256 content integrity, bounded framing, and that `transfer_complete` cannot arrive before `file_end`.

Run it with:

```bash
cargo run --locked --features two-node --bin two_node --manifest-path src-tauri/Cargo.toml
```
