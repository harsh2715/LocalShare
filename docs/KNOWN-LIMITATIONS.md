# Known Limitations

- Trust UI, persisted cryptographic key storage, certificate pinning, and changed-key warnings are incomplete.
- mDNS discovery is implemented and the listener probes 32123 then an available port, but this has not been verified on a second physical LAN device.
- Incoming transfer requests now wait for the UI Accept/Reject decision when the setting is enabled; this flow is source-wired but not physically tested.
- File sender and receiver paths exist, single-file/chat are covered by the virtual harness, but multi-file, large-file, cancellation, resume, and reconnect runtime verification are incomplete.
- Chat and transfer history tables and pages exist; packaged navigation is verified, but end-to-end history population is not physically tested.
- macOS app and plain DMG packaging are verified on this Mac. Windows packaging remains blocked by environment.
- The Windows pipeline is prepared, but the primary `release/windows/LocalShare-Setup.exe` does not exist yet. A real Windows VM/build host is required; no Mac binary has been renamed or relabeled as `.exe`.
- Windows offline WebView2 mode is configured but not built or installed; Windows firewall, mDNS, GUI, and Mac↔Windows runtime behavior remain unverified.
- No physical two-computer test has been performed.
