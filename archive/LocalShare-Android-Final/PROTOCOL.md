# LocalShare Protocol v1

## Transport

Each peer connection is a real TCP socket upgraded to a rustls session. Both sides present self-signed certificates. The current implementation validates the certificate fingerprint against the expected manual-connect fingerprint and against the persisted trusted-peer record; the peer HELLO fingerprint must match the certificate presented by that session.

The protocol is not a localhost shortcut. The same TCP, TLS, framing, identity, and handlers are used by the Tauri application and `localshare-test-node`.

## Frame format

```text
4-byte unsigned big-endian JSON byte length
JSON object
```

The maximum encoded frame is 256 KiB. A frame contains:

```json
{
  "version": 1,
  "kind": "hello",
  "id": "uuid-v4",
  "timestamp": 0,
  "payload": {}
}
```

Reads use `read_exact`, so arbitrary TCP fragmentation is supported. Empty, oversized, invalid-JSON, unsupported-version, and unknown-operation frames are rejected.

## Operations currently implemented

`hello`, `heartbeat`, `heartbeat.ack`, `control.test`, `control.ack`, `pair.request`, `pair.response`, `chat.message`, `chat.ack`, `file.offer`, `file.accept`, `file.chunk`, `file.complete`, `file.complete.ack`, and `file.cancel`.

HELLO carries the device ID, display name, protocol version, capabilities, and certificate fingerprint. It is exchanged after TLS and before normal chat/file operations.

## Chat

Chat messages carry a UUID, body, and peer context. The sender persists `QUEUED` then `SENT`; the receiver persists with `INSERT OR IGNORE`, sends `chat.ack`, and the sender records `DELIVERED`. Queued outgoing messages are flushed after a trusted reconnect.

## Files

An offer includes transfer ID, original filename, byte size, and SHA-256. The receiver validates the filename and metadata, checks verified completed-file candidates, and either cancels an exact physical duplicate or allocates a collision-safe destination. Accepted data is sent in bounded 64 KiB chunks encoded in JSON as base64. The receiver writes a `.part` file, streams SHA-256 while writing, verifies size/hash, atomically renames into the download root, and persists completion metadata.

The current implementation hashes the source before offering it, then streams the transfer. It supports automatic resume after a network or process restart by rehashing the retained partial file and resuming from its length. It does not yet expose user pause/resume or a test-only throttle/fault-injection layer.

## Trust and failure rules

The peer ID, certificate fingerprint, and persisted trusted record are separate from the transfer ID and file-content hash. A display name or IP address cannot establish trust. Changed trusted identity is rejected and requires re-pairing. Blocked peers are rejected. Transfer source size/mtime is checked before streaming; a source change fails the transfer.

