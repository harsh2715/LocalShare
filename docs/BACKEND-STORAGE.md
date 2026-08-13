# LocalShare backend storage

LocalShare keeps application state in the platform application-data directory
and keeps received user files in the platform Downloads directory. It never
uses the source repository as a production data directory.

## Locations

| Data | macOS | Windows |
| --- | --- | --- |
| SQLite and identity | `~/Library/Application Support/LocalShare/` | `%LOCALAPPDATA%\\LocalShare\\` |
| Database | `localshare.sqlite` in that directory | `localshare.sqlite` in that directory |
| Device identity | `device-id` in that directory | `device-id` in that directory |
| Received files | `~/Downloads/LocalShare/` by default | `%USERPROFILE%\\Downloads\\LocalShare\\` by default |
| Partial incoming files | beside the final destination as `*.localshare-part` | beside the final destination as `*.localshare-part` |

The paths are resolved by `dirs::data_local_dir()` and
`dirs::download_dir()` in `src-tauri/src/storage.rs`; no username is
hardcoded. The received directory is created on the first approved incoming
transfer and can be changed in Settings.

## What SQLite stores

`localshare.sqlite` contains:

- `settings`: device name, received-files directory, receive-approval choice,
  notifications choice, and optional update-check choice.
- `trusted_peers`: device UUID, display name, pinned advertised fingerprint,
  and trust timestamp.
- `chat_messages`: message UUID, peer UUID, direction, UTF-8 content, time,
  and delivery state.
- `transfer_history`: transfer UUID, peer UUID, relative/display name, size,
  direction, result, source or received destination path, and timestamp.

The database uses SQLite WAL mode. Settings, chats, trust decisions, history,
and the persistent device UUID survive restart. The current session sockets,
peer availability, progress events, and in-memory cancellation flags do not.

## Filesystem and cleanup

Received files are written to a temporary `.localshare-part` name, streamed in
bounded chunks, hashed with SHA-256, and renamed to the final collision-safe
name only after size and hash verification. Existing names become, for
example, `report.pdf`, `report (1).pdf`, and `report (2).pdf`.

Deleting the SQLite file resets settings, chat/history, trust decisions, and
the device identity on the next launch. It does not delete received files.
Users can delete `.localshare-part` files only after confirming no transfer is
active. Uninstall behavior is platform-installer dependent; user Downloads
files are intentionally separate from the application database.

## Security boundary

TLS protects the direct LAN session. The persistent `device-id` and pinned
fingerprints are not private keys. The current certificate key is generated
for the process, while the peer fingerprint is persisted and checked before a
trusted session is accepted. Private key material is not printed or stored in
SQLite. The updater signing private key is not application data and is kept
outside Git in the ignored project-only `.localshare-private/` directory/CI
secret.

```text
React frontend
    ↓ Tauri commands/events
Rust backend
    ├─ SQLite: settings, messages, trust, history
    ├─ application data: device-id
    ├─ Downloads/LocalShare: verified received files
    └─ TLS/TCP + mDNS: live LAN state only
```
