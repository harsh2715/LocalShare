# Cleanup audit

Audit date: 2026-08-14. Scope is only `/Users/harshsinghal/Desktop/Local Network`.

## Keep

- `.git`, `.github`, source, Tauri/Cargo/npm manifests and lockfiles.
- Production icons, build scripts, Windows/macOS launch/build helpers.
- `README.md`, architecture/security/storage/offline/release/test documentation.
- Current test evidence that records real verification.

## Regenerable and ignored

- `node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`.
- `src-tauri/artifacts/`, `*.tsbuildinfo`, and `__pycache__`.
- `.localshare-test-work/`, local SQLite/WAL files, and `.localshare-private/`.
- Release installer binaries are ignored; their checksums/release notes remain.

## Removed in this cleanup

- The tracked `artifacts/test-files/` fixture set. New generated fixtures stay
  under ignored `.localshare-test-work/` only.
- Superseded build/debug logs and empty result placeholders under
  `artifacts/test-results/` (`tauri-*`, old two-node logs, old packaged-runtime
  logs, and the timestamped mDNS log). Current runner, packaging, offline,
  mDNS, failure, and large-file evidence remains.
- The ignored `.localshare-test-work/` directory, including the local 100 MiB
  and 1 GiB transfer outputs, after verification. It occupied approximately
  2.4 GiB and is recreated only when tests run.

## Stale or review-only

- Historical Windows VM report: retained because it explains why hosted CI is
  used and records that no VM is required.
- Existing reports contain prior evidence and were updated rather than deleted
  blindly; no source or production test harness was removed.

No files outside the project boundary were modified intentionally.
