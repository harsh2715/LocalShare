# LocalShare Protocol v2

Messages are UTF-8 JSON preceded by a 4-byte big-endian length. Frames are capped at 8 MiB. Every message is validated before use. File chunks are capped at 4 MiB and the sender uses 1 MiB chunks.

Implemented message types: `hello`, `hello_ack`, `ping`, `pong`, `chat`, `transfer_request`, `transfer_accept`, `transfer_reject`, `transfer_cancel`, `file_start`, `file_chunk`, `file_end`, `transfer_complete`, and `error`.

`hello` carries a persistent Ed25519 public key, its SHA-256 fingerprint, a listening port, a fresh nonce, capabilities, and a signature. `hello_ack` signs both nonces and both device IDs. `transfer_request` carries a complete file manifest with safe basenames, normalized relative paths, sizes, and a checked total. The receiver pins that manifest before accepting any `file_start`. `file_start` carries a SHA-256 hash. `file_chunk` carries a file ID and exact offset. The receiver requires offsets to be contiguous, hashes bytes while writing, verifies size and hash, then commits the collision-safe partial file without overwriting an existing destination.

Security limits include maximum chat length, maximum file count, checked size sums, filename traversal rejection, frame-size rejection, and protocol-version validation.
