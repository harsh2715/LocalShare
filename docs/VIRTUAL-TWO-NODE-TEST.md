# Virtual Two-Node Test

Status: NOT IMPLEMENTED.

The intended harness will launch two backend nodes with isolated data directories, identities, ports, and download directories, then verify bidirectional chat, single/multiple file hashes, chat during transfer, cancellation, and reconnect. The current Tauri binary owns its listener and application-data path, so a deterministic harness has not yet been added.
