# LocalShare final release report

Date: 2026-09-11  
Repository: https://github.com/harsh2715/LocalShare

## Result

LocalShare was audited, hardened, tested, and published as [v0.1.2](https://github.com/harsh2715/LocalShare/releases/tag/v0.1.2).

- Release source tag: `v0.1.2` at commit `703b367257883d02b4966015072fa8b0789c9908`
- Current `main` also contains the follow-up release-workflow fix that uploads the portable Windows executable.
- Successful release workflow: [run 34593025756](https://github.com/harsh2715/LocalShare/actions/runs/34593025756)
- The earlier `v0.1.0` and `v0.1.1` tags remain as failed-release history and have no published release.

## Main problems found and corrected

- The TLS peer verifier accepted any certificate. It now validates a self-signed Ed25519 certificate, validity period, key type/length, signature, and TLS 1.3-only handshake policy.
- The application protocol had weak identity and message validation. Protocol v2 now signs the nonce handshake, binds the advertised port and peer fingerprint, validates capabilities, bounds frames/files/chunks, checks hashes, and rejects unsafe paths and reserved names.
- File receiving could overwrite or leave unsafe partial files. Receiving now uses safe paths, refuses overwrite, tracks manifest completion, validates aggregate transfer state, cleans stale partials, and uses bounded timeouts/control channels.
- Listener startup had an address/port race and weak IPv6 handling. Startup and address handling were corrected.
- The frontend had duplicate transfer events, stale state, missing drop cleanup, and no test/lint harness. Event normalization/deduplication, manifest preview, cleanup, Vitest tests, ESLint, and native completion notifications were added.
- The release pipeline used the wrong Windows updater suffix, failed to preserve the matching signature name, omitted Android from publishing, and did not align project versions with the release tag. These are fixed for `v0.1.2` and future releases.
- Android initialization/capability/native-entry-point issues were resolved; CI now regenerates and patches the Android manifest and builds the aarch64 APK.

## Security posture

The transport is LAN-oriented and encrypted with TLS 1.3. The application handshake additionally authenticates the peer’s Ed25519 identity and signed nonce. Persistent identity keys use the OS keyring where available, with a protected local fallback. Incoming files are constrained to a safe receive directory and are hash-checked before completion.

This is not a complete user-facing pairing/authorization system yet. A future release should add explicit trusted-peer management, revocation, and clearer first-contact approval UX.

## Validation performed

- Frontend: `npm run lint`, `npm run test` — 3 Vitest tests passed, and `npm run build` passed.
- Dependency audit: production `npm audit --omit=dev` reported zero vulnerabilities.
- Rust: locked formatting/check/clippy/test validation passed in the hosted Windows and macOS jobs; the Rust unit suite previously passed with 13 tests after the security refactor.
- Transfer harness: 1 MiB, 10 MiB, and 100 MiB local two-node transfers passed; the hosted Windows job also passed the 100 MiB transfer.
- macOS hosted job: validation, signed updater bundle, app archive, and DMG collection passed.
- Windows hosted job: validation, PE checks, signed NSIS installer, signature collection, checksum generation, and artifact upload passed.
- Android hosted job: native Rust/Gradle build and aarch64 debug APK collection passed.

## Published artifacts

All asset hashes below are the hashes reported by GitHub and recorded in the corrected release `SHA256SUMS.txt`:

| Asset | Size | SHA-256 |
|---|---:|---|
| `LocalShare-Setup.exe` | 221,066,571 | `e840200ccb86c5a3325761a2c030cf4176f76f3ce681ac5eca75581ae7908a74` |
| `LocalShare-Setup.exe.sig` | 420 | `e29d0e9e62b63b93180bf0428e3846606aa01e97a23adbbfcf6d44a2257164ee` |
| `LocalShare.app.tar.gz` | 8,258,763 | `d3312037a023c8e7bc00b37d01923d0805a9006bfdb222f26831c5900abb9f88` |
| `LocalShare.app.tar.gz.sig` | 408 | `150ddef79e6e9f4db9dd5a32fef44d7679f3483bf74264642f59c87b399f9a8b` |
| `LocalShare_0.1.2_aarch64.dmg` | 8,798,653 | `fa7f5e87dc6a46e5751ef8d13e54f9c7addf06a03c541beed4f32b5f18555f54` |
| `LocalShare-android-aarch64-debug.apk` | 227,937,350 | `e8bb1eb213126fde0a78f06d32a08850f045633698dd0afd4f6c78ca40019b88` |

Downloaded verification also confirmed the APK is a valid ZIP archive, the macOS updater archive is readable, the DMG is a valid UDZO image, and both updater manifest signatures exactly match their published `.sig` files.

## Copy audit and cleanup

The canonical checkout is `/Users/harshsinghal/Documents/Codex/2026-09-11/https-github-com-harsh2715-localshare-https/work/LocalShare`.

- The older Git checkout under `Documents/Codex/2026-09-06/.../work/LocalShare` contains unique uncommitted mobile work and was preserved.
- The Android-final non-git workspace contains unique Android/vendor source and was preserved.
- The Desktop clones were not deleted because they are source copies and deletion was not necessary for release correctness.
- Only generated dependency/build caches were removed from the inspected copies. Source, Android generated project inputs, and unique uncommitted work were preserved.

## Remaining limitations

- No physical second-computer LAN session was available on this host, so real Wi-Fi discovery/firewall behavior remains to be tested on two machines.
- No physical Windows device or Android device was attached; Android `adb` had no device listed.
- Full Xcode and Apple signing/notarization credentials were unavailable locally. The hosted macOS build passed, but the DMG/app is not claimed to be Apple Developer ID signed or notarized.
- The Android artifact is a debug/validation APK, not a Play Store production-signed build; an Android keystore and release signing configuration are still required.
- The `two_node` harness exercises the hardened protocol/TLS choreography but does not yet invoke the private production `handle_tls_stream`/`AppState` path. A future refactor should expose a testable production transport service and run the harness through it.
- Transfer resume/retry is not implemented; interrupted transfers require restarting.
