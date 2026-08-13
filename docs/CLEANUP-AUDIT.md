# Cleanup audit

Audit date: 2026-08-14. Scope is only `/Users/harshsinghal/Desktop/Local Network`.

## Keep

- `.git`, `.github`, source, Tauri/Cargo/npm manifests and lockfiles.
- Production icons, build scripts, Windows/macOS launch/build helpers.
- `README.md`, architecture/security/storage/offline/release/test documentation.
- Existing protocol fixtures and test evidence that records real verification.

## Regenerable and ignored

- `node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`.
- `artifacts/two-node/`, `src-tauri/artifacts/`, `*.tsbuildinfo`, `__pycache__`.
- `.localshare-test-work/`, local SQLite/WAL files, and `.localshare-private/`.
- Release installer binaries are ignored; their checksums/release notes remain.

## Stale or review-only

- Historical Windows VM report: retained because it explains why hosted CI is
  used and records that no VM is required.
- Existing reports contain prior evidence and are being updated rather than
  deleted blindly; no source or test harness was removed.

No broad cleanup command was run. No files outside the project boundary were
modified intentionally.
