# LocalShare Final Software Report

Audit date: 2026-08-14. Scope: `/Users/harshsinghal/Desktop/Local Network`
and the single authorized repository `harsh2715/LocalShare`.

## Result

The existing LocalShare project was hardened in place. macOS packaging and
GUI smoke verification pass. A genuine Windows x64 Tauri/NSIS build now passes
on GitHub Actions, including a 100 MiB Windows loopback stream with chat during
the transfer, PE validation, silent installation, installed-process launch, and
artifact upload. Physical Mac↔Windows LAN, cancellation, reconnect, multi-file
batch, folder E2E, and installed update/relaunch remain unverified where the
available environment cannot prove them. Resume is not implemented.

## Original problems found and fixes

- The production bundle had previously been at risk of containing the
  test-only `two_node`; Cargo/Tauri now explicitly select `localshare`, and
  packaging rejects/removes `two_node` from the app payload.
- Received files previously defaulted under application data. Defaults now
  resolve through `dirs::download_dir()` to `~/Downloads/LocalShare` on macOS
  and `%USERPROFILE%\\Downloads\\LocalShare` on Windows.
- Folder manifests now preserve normalized nested relative paths, empty
  directories, and safe duplicate names while rejecting symlinks/traversal.
- Incoming transfers now wait for Accept/Reject before writing data by default.
- Partial files use `filename.ext.localshare-part` and are renamed only after
  size and SHA-256 verification.
- Transfer cancellation, duplicate-session rejection, bounded reconnect retry,
  failed/rejected/cancelled history records, and malformed hash/chunk checks
  were hardened in the Rust transport path.
- Messages selection now loads persisted SQLite history before opening the
  conversation.
- Diagnostics now expose real version, platform, architecture, IPs, listener,
  mDNS, TLS, database, receive-directory, peer, and updater state without keys.
- The Windows path-safety regression and Windows test-harness stack overflow
  were found by hosted CI and fixed before the final successful run.

## UI controls

Nearby devices, Messages, Transfer History, Settings, Diagnostics, Refresh,
device cards, Send, Select Files, Folder, drag/drop, Accept, Reject, Cancel,
Open File, Show in Folder, Change Folder, Open Folder, Copy Diagnostics,
Trust Device, Clear History, and Check for Updates are source-wired and were
inspected in the rebuilt packaged macOS GUI. The old dead actions menu was
removed. Windows GUI visual inspection remains NOT TESTED on the hosted
runner.

## Storage and received-file behavior

| Data | macOS | Windows |
| --- | --- | --- |
| SQLite/application state | `~/Library/Application Support/LocalShare/` | `%LOCALAPPDATA%\\LocalShare\\` |
| Database | `localshare.sqlite` | `localshare.sqlite` |
| Device identity | `device-id` in app-data directory | `device-id` in app-data directory |
| Default received files | `~/Downloads/LocalShare/` | `%USERPROFILE%\\Downloads\\LocalShare\\` |
| Partial files | beside destination as `*.localshare-part` | beside destination as `*.localshare-part` |

SQLite contains settings, chat messages, trusted-peer fingerprints, and
transfer history. Transfer history stores direction, peer, name, size, result,
timestamp, and source/destination path. TLS certificate material is generated
in process memory; no production private key is written as plaintext. The
updater preference is SQLite setting `check_for_updates`; there is no separate
LocalShare updater database or application log file. See
`docs/BACKEND-STORAGE.md` for the source-traced details.

Settings provides the current receive path, Change folder, Open folder, device
name, receive approval, notifications/in-app notices, trusted-device summary,
and optional update checking. History provides Open file and Show in folder
when a recorded path still exists.

## Transfer capability and evidence

- Streaming: PASS. Production sender/receiver use bounded heap buffers and
  framed chunks; no `read_to_end` path exists in the transfer implementation.
- Wire-size fields: `u64`; manifest sums use checked arithmetic. SQLite history
  is limited by SQLite's signed 64-bit INTEGER representation.
- Actual maximum tested: 1 GiB in the virtual TLS loopback harness on macOS,
  with incremental source/destination SHA-256. A 100 MiB run also passed.
- Windows hosted loopback: PASS for 100 MiB streaming, SHA-256 equality, and
  chat A↔B while the transfer was active.
- Single file: PASS A→B in the virtual harness at 1 MiB, 100 MiB, and 1 GiB;
  reverse-direction and physical LAN results are NOT TESTED.
- Multiple files: PARTIAL. Manifest and path handling are implemented and
  unit-tested; a complete multi-file E2E batch was NOT TESTED.
- Folder/nested/empty folders: PARTIAL. Recursive enumeration, empty-folder
  creation, safe relative paths, duplicate naming, and symlink rejection are
  unit-tested; a complete two-node folder E2E batch was NOT TESTED.
- Accept/Reject: PASS in source wiring; physical E2E approval is NOT TESTED.
- Progress/speed/ETA: PARTIAL. Progress and byte counts are rendered; an
  explicit calculated speed/ETA display is not implemented.
- Duplicate names: PASS in helper tests; deterministic `name (1).ext` policy.
- SHA-256/integrity: PASS in virtual harness and receiver implementation.
- Cancel: PARTIAL. Sender/receiver command and protocol paths preserve the
  peer session and clean partial files, but end-to-end cancel-then-chat/new-file
  was NOT TESTED.
- Reconnect: PARTIAL. Trusted lower-UUID peers retry up to five times with
  bounded exponential delays; Wi-Fi interruption and post-reconnect transfer
  were NOT TESTED.
- Resume: NOT IMPLEMENTED.

## Security and offline boundary

TLS 1.3-capable rustls protects the direct TCP session. First-contact peers
require explicit fingerprint trust; persisted fingerprints are checked and a
changed identity is rejected. Certificate-key persistence is incomplete, so
trust/pinning is PARTIAL rather than production-complete cryptographic key
pinning.

The static offline audit is PASS: no PairDrop, cloud signaling, STUN/TURN,
telemetry, public WebSocket, CDN, or remote-font runtime path exists. The
packaged process-socket audit is PASS for the limited observation: TCP listener
and mDNS sockets only; the optional updater is the documented GitHub HTTPS
exception. Internet-off router capture and physical LAN runtime are NOT TESTED.

## Updater

The Tauri updater is configured for the authorized GitHub Releases endpoint,
signed `.sig` artifacts, public-key verification, download/install, and app
relaunch. Settings supports automatic checking and Check for updates; progress
events are surfaced as in-app notices. The signing private key is ignored and
the `TAURI_SIGNING_PRIVATE_KEY` secret is configured only on
`harsh2715/LocalShare`. A real installed version-A→version-B update/relaunch
is NOT TESTED. If Internet is unavailable, the updater fails gracefully and
does not block mDNS, chat, files, or folders.

## macOS build/package

```text
MACOS FRONTEND BUILD: PASS
MACOS RUST CHECK/TEST: PASS
MACOS APP: PASS
MACOS DMG: PASS
PACKAGED EXECUTABLE: PASS — Contents/MacOS/localshare
TEST HARNESS IN APP: PASS — two_node absent from app payload
PACKAGED GUI: PASS — non-blank native window and pages inspected
MACOS LAN IP: PASS — 192.168.1.17 displayed
MACOS mDNS ADVERTISEMENT: PASS — _localshare._tcp.local observed
```

## Windows hosted release

```text
WINDOWS RUNNER: PASS — run 31783746842
WINDOWS BUILD: PASS
WINDOWS RUST CHECK: PASS
WINDOWS RUST TESTS: PASS
WINDOWS TWO-NODE LOOPBACK: PASS — 100 MiB stream, chat during transfer, SHA-256
LOCALSHARE.EXE: PASS — x86_64 PE
TWO_NODE.EXE PRODUCTION REJECTION: PASS
NSIS BUILD: PASS
INSTALLER PE VALIDATION: PASS
WINDOWS INSTALL AUTOMATION: PASS — silent NSIS install
WINDOWS PROCESS LAUNCH: PASS
WINDOWS GUI VISUAL: NOT TESTED
```

Final installer artifact: [LocalShare-Setup](https://github.com/harsh2715/LocalShare/actions/runs/31783746842/artifacts/9213523891)

Metadata artifact: [LocalShare-Windows-release-metadata](https://github.com/harsh2715/LocalShare/actions/runs/31783746842/artifacts/9213524244)

Signed updater artifact: [LocalShare-updater-artifacts](https://github.com/harsh2715/LocalShare/actions/runs/31783746842/artifacts/9213525558)

Installer SHA-256:
`a48eacbc290e7caa333d26317ff4b0625265162b8ed65470341b30821ffec1fa`

GitHub artifact archive size: 217,517,315 bytes (approximately 207.4 MiB).
The hosted-runner action validated the raw installer before upload; the raw
installer was not copied back to this Mac, so no different local file is being
represented as the Windows installer.

## Cleanup and repository state

The tracked fixture set, obsolete build/debug logs, empty result placeholders,
and the ignored 2.4 GiB `.localshare-test-work` outputs were removed. Source,
lockfiles, workflows, current evidence, release notes, and reproducible test
harnesses were retained. Generated `node_modules`, `dist`, `target`, release
binaries, SQLite/WAL files, test workspace, and signing keys are ignored.

Final branch: `main`

Latest verified source/test commit: `9e2b600aa22bc9b640f4488c4782d3cfa3678d1b`

The final repository HEAD also includes the documentation/evidence update
commits recorded in the handoff below.

Push result: PASS — pushed only to `origin`
`https://github.com/harsh2715/LocalShare.git`.

No files outside `/Users/harshsinghal/Desktop/Local Network` were intentionally
modified. No GitHub repository other than `harsh2715/LocalShare` was modified.
