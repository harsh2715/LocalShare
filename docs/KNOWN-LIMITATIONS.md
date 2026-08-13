# Known Limitations

- Trust UI, persisted cryptographic key storage, certificate pinning, and changed-key warnings are incomplete.
- mDNS discovery is implemented and the listener probes 32123 then an available port, but this has not been verified on a second physical LAN device.
- Incoming transfer requests are currently auto-accepted in the backend; the review/accept/reject UI is incomplete.
- File sender and receiver paths exist, single-file/chat are covered by the virtual harness, but multi-file UI review, cancellation, resume, and reconnect state machines are incomplete.
- Chat and transfer history tables exist, but history screens and clear/open actions are incomplete.
- macOS app packaging and Windows packaging have not been executed.
- No physical two-computer test has been performed.
