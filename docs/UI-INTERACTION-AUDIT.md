# LocalShare UI interaction audit

Audit date: 2026-08-14; usability pass updated 2026-09-06. This records source-level wiring plus direct inspection of the rebuilt packaged macOS GUI and browser-based accessibility/layout checks; a physical two-computer interaction session remains separate and is not claimed here.

| Control | Result | Implementation/evidence |
| --- | --- | --- |
| Nearby devices | PASS | `src/main.tsx` page navigation and automatic peer list; verified in packaged `.app` |
| Messages | PASS | `src/main.tsx` route, peer list, conversation view, persisted `chat_history` load when a peer is selected; packaged navigation verified |
| Transfer history | PASS | `src/main.tsx` route, SQLite-backed `transfer_history` command; packaged navigation verified |
| Settings | PASS | `src/main.tsx` form and persistent `save_settings` command; packaged navigation verified |
| Diagnostics | PASS | `src/main.tsx` real version/platform/IP/listener/mDNS/TLS/SQLite/receive/updater status view and Copy diagnostics action; packaged navigation verified |
| Refresh | PASS | reloads local info and peers |
| Device card | PASS | selects peer and opens conversation; automatic connection remains the normal path |
| Send message | PASS | `send_chat` command and framed `Chat` message |
| Multiline/Unicode chat | PASS source/unit coverage | textarea and UTF-8 protocol validation |
| Select files | PASS | native multi-file picker calls `send_files` |
| Send folder | PASS | native directory picker recursively enumerates a safe relative-path manifest |
| Transfer progress | PASS source wiring | backend progress events render in the conversation; bounded 100 MiB/1 GiB harness evidence is recorded separately |
| Cancel transfer | PARTIAL | command/protocol/UI wiring exists and leaves the peer session intact by design; end-to-end cancellation is not yet verified |
| Open file | PASS when path exists | platform-specific `open`/`explorer` command |
| Show in folder | PASS when path exists | platform-specific folder opener |
| Change folder | PASS source wiring | directory picker persists the received-files location |
| Open received folder | PASS source wiring | Settings action opens the configured folder |
| Clear history | PASS | SQLite delete action |
| Copy diagnostics | PASS source wiring | excludes private keys |
| Troubleshooting / Connect using IP | PASS | intentionally retained as fallback only |
| Drag and drop | PASS source wiring | Tauri webview drop events call the same multi-file `send_files` command |
| Accept/reject incoming transfer | PASS source wiring | `transfer-request` opens a modal; `accept_transfer`/`reject_transfer` resolve the real protocol decision |
| Trust new device | PASS source wiring | fingerprint prompt, persisted trust, identity-change rejection |
| Check for updates | PASS source wiring | signed Tauri updater check is Internet-only and failure is non-fatal |
| First-run guidance | PASS | Empty Nearby state gives a three-step same-network and fingerprint checklist plus IP fallback |
| Offline conversations | PASS source wiring | Saved chat remains discoverable through persisted trusted-device names when a peer is offline |
| Desktop notifications | PASS source wiring | Official Tauri notification plugin is permission-gated and only notifies for completed receives |
| Validation and feedback | PASS | Device name, message length, IPv4/IPv6, and port validation have inline/friendly feedback and busy states |
| Keyboard/accessibility | PASS source/browser inspection | Semantic labels, focus-visible states, live notices, dialog roles, Escape close, reduced-motion support, and Enter/Shift+Enter composer behavior |
| Narrow desktop window | PASS browser inspection | The two-column sharing workspace becomes one column before the configured 900 px minimum window width |
| More/actions menu | REMOVED | it was previously a dead visible button |

Windows note: this audit is source-level plus packaged macOS inspection. No Windows executable or installer exists on this host, so Windows GUI controls are NOT VERIFIED.

Build checks used for this audit:

```text
npm run build
cd src-tauri && cargo check --all-targets && cargo test --all-targets
```
