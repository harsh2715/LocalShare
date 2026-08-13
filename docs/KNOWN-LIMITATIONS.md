# Known Limitations

- Trust UI, persisted cryptographic key storage, certificate pinning, and changed-key warnings are incomplete.
- mDNS discovery is implemented and the listener probes 32123 then an available port, but this has not been verified on a second physical LAN device.
- Incoming transfer requests now wait for the UI Accept/Reject decision when the setting is enabled; this flow is source-wired but not physically tested.
- File sender and receiver paths exist, single-file/chat are covered by the virtual harness, but multi-file, large-file, cancellation, resume, and reconnect runtime verification are incomplete.
- Chat and transfer history tables and pages exist; packaged navigation is verified, but end-to-end history population is not physically tested.
- macOS app and plain DMG packaging are verified on this Mac. Windows NSIS packaging and hosted smoke launch are verified by GitHub Actions run `31724824519`.
- The final Windows installer exists at `release/windows/LocalShare-Setup.exe` and is also available as the `LocalShare-Setup` GitHub artifact. No Mac binary was renamed or relabeled as `.exe`.
- Windows offline WebView2 mode was built and silently installed on the hosted runner; Internet-off installation on a user machine, Windows firewall behavior, mDNS, GUI visual behavior, and Mac↔Windows runtime remain unverified.
- No physical two-computer test has been performed.
