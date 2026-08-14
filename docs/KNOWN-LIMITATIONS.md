# Known Limitations

- Certificate public-key persistence is incomplete; device fingerprint trust, persisted pins, first-contact approval, and changed-identity rejection are implemented.
- mDNS discovery is implemented and the listener probes 32123 then an available port, but this has not been verified on a second physical LAN device.
- Incoming transfer requests now wait for the UI Accept/Reject decision when the setting is enabled; this flow is source-wired but not physically tested.
- File/folder sender and receiver paths are implemented with streaming, safe relative manifests, duplicate naming, and approval. The loopback harness passed 100 MiB and 1 GiB streaming/hash checks and chat in both directions during the 100 MiB transfer; multi-file/folder batch, cancellation, resume, and reconnect runtime verification remain incomplete.
- Chat and transfer history tables and pages exist; packaged navigation is verified, but end-to-end history population is not physically tested.
- macOS app and plain DMG packaging are verified on this Mac. Windows NSIS packaging, signed updater artifacts, and hosted smoke launch are verified by GitHub Actions run `31748837359`.
- The final Windows installer exists at `release/windows/LocalShare-Setup.exe` and is also available as the `LocalShare-Setup` GitHub artifact. No Mac binary was renamed or relabeled as `.exe`.
- Windows offline WebView2 mode was built and silently installed on the hosted runner; Internet-off installation on a user machine, Windows firewall behavior, mDNS, GUI visual behavior, and Mac↔Windows runtime remain unverified.
- No physical two-computer test has been performed.
- The updater is configured and signed in CI, but a real installed version-A → version-B update/relaunch has not been run.
