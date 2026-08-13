# Known Limitations

- Trust UI, persisted cryptographic key storage, certificate pinning, and changed-key warnings are incomplete.
- mDNS discovery is implemented, but the listener uses a fixed 32123 port and has no occupied-port fallback.
- Incoming transfer requests are currently auto-accepted in the backend; the review/accept/reject UI is incomplete.
- File sender and receiver paths exist, but multi-file UI review, cancellation, resume, and reconnect state machines are incomplete.
- Chat and transfer history tables exist, but history screens and clear/open actions are incomplete.
- macOS app packaging and Windows packaging have not been executed.
- No physical two-computer test has been performed.
