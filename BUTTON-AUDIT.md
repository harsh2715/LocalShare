# LocalShare control audit

Audit date: 2026-08-14. This is source/build verification; physical
Mac↔Windows interaction remains a separate test.

| Control | Screen | Original status | Fix performed | Final status | Test |
| --- | --- | --- | --- | --- | --- |
| Nearby devices | Sidebar | WORKING | Kept automatic peer view | PASS | Build + packaged macOS launch |
| Messages | Sidebar | PARTIAL | Kept peer list and conversation persistence | PASS | Build + source wiring |
| Transfer history | Sidebar | PARTIAL | Added Open file and Show in folder actions | PASS | Build + source wiring |
| Settings | Sidebar | PARTIAL | Added received-folder actions and update preference | PASS | Build + source wiring |
| Diagnostics | Sidebar | WORKING | Kept redacted copy action | PASS | Build + source wiring |
| Refresh | Header | WORKING | Kept local-info/peer refresh | PASS | Build |
| Device card | Nearby | PARTIAL | Trust-required cards now open trust prompt | PASS | Build + source wiring |
| Send message | Conversation | WORKING | Kept SQLite + direct control channel | PASS | Two-node chat both ways |
| Select files | Conversation | WORKING | Kept native multi-file picker | PASS | Build |
| Send folder | Conversation | MISSING | Added directory picker and recursive safe manifest | PASS source; E2E NOT VERIFIED | Build + protocol tests |
| Drag/drop | Conversation | WORKING | Kept native drop event path | PASS source | Build |
| Accept | Incoming modal | WORKING | Kept real pending decision | PASS source | Build |
| Reject | Incoming modal | WORKING | Kept real pending decision | PASS source | Build |
| Cancel transfer | Conversation | PARTIAL | Kept command/protocol path | PARTIAL | E2E NOT VERIFIED |
| Open file | History | PARTIAL | Added platform file opener | PASS source | Build |
| Show in folder | History | MISSING | Added platform folder opener | PASS source | Build |
| Change folder | Settings | MISSING | Added directory picker and persistence | PASS source | Build |
| Open folder | Settings | MISSING | Added configured-folder action | PASS source | Build |
| Check for updates | Settings | MISSING | Added signed Internet-only updater check | PASS source | Build |
| Trust device | Trust modal | MISSING | Added fingerprint prompt and persisted trust | PASS source | Build |
