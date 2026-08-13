# LocalShare Protocol v1

Messages are UTF-8 JSON preceded by a 4-byte big-endian length. Frames are capped at 8 MiB. Every message is validated before use. File chunks are capped at 4 MiB and the sender uses 1 MiB chunks.

Implemented message types: `hello`, `hello_ack`, `ping`, `pong`, `chat`, `transfer_request`, `transfer_accept`, `transfer_reject`, `transfer_cancel`, `file_start`, `file_chunk`, `file_end`, `transfer_complete`, and `error`.

`transfer_request` carries an ID, an array of file IDs, safe basename filenames, optional relative paths (currently unused), sizes, and a checked total. `file_start` carries a SHA-256 hash. `file_chunk` carries a file ID and exact offset. The receiver requires offsets to be contiguous, hashes bytes while writing, verifies size and hash, then atomically renames the partial file.

Security limits include maximum chat length, maximum file count, checked size sums, filename traversal rejection, frame-size rejection, and protocol-version validation.
