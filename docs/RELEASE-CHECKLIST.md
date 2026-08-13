# Release Checklist

- [x] Frontend build
- [x] Rust check
- [x] Rust unit tests
- [x] Direct LAN listener source path
- [x] mDNS source path
- [x] Bounded file I/O source path
- [x] Atomic partial-file rename and hash check source path
- [ ] Trust/pinning complete
- [x] Virtual two-node TLS chat/single-file/hash slice
- [ ] Physical LAN test
- [ ] Offline packet capture
- [x] Packaged macOS runtime backend startup
- [x] Native packaging fixed with `scripts/build-macos.sh` (Homebrew LLVM/Rust target bin PATH)
- [ ] Windows VM/build host available — BLOCKED_BY_ENVIRONMENT; see `docs/WINDOWS-VM-BUILD-REPORT.md`
- [ ] Packaged Windows runtime — BLOCKED_BY_ENVIRONMENT
- [ ] Windows installer install/launch — BLOCKED_BY_ENVIRONMENT
- [ ] Resume and cancellation
