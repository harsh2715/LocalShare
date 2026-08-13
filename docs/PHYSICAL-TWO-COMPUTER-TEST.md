# Physical Two-Computer Test

Status: NOT VERIFIED in this environment. The current build has passing virtual TLS chat/single-file/hash coverage and Mac package/UI verification, but physical LAN and mDNS behavior still require a second computer.

1. Put a macOS computer and a Windows/macOS computer on the same Wi-Fi or Ethernet LAN. Windows must use the real installed `LocalShare-Setup.exe`; the current Mac workspace does not contain one yet.
2. Disconnect the router WAN, or use a local hotspot with no upstream Internet.
3. Confirm an ordinary website cannot load.
4. On Mac, open `src-tauri/target/release/bundle/macos/LocalShare.app`. On Windows, double-click the installed LocalShare Start Menu entry after running `LocalShare-Setup.exe`. The portable `RUN-LOCALSHARE.cmd` path is only a fallback. Allow LocalShare on Private networks if Windows Firewall asks.
5. Launch both apps. Wait for each device to appear under Nearby devices through mDNS and for the state to become Connected. Do not enter an IP during the normal test; manual IP is troubleshooting-only.
6. Click the connected peer and send chat both ways. Inspect Diagnostics on both computers for private LAN IP, listener port, mDNS, TLS, and database state.
7. Send a zero-byte file, a Unicode filename, one file, and multiple files. Compare SHA-256 hashes with `shasum -a 256` on macOS or `Get-FileHash` on Windows.
8. Send a generated 100 MB file while exchanging chat messages. Confirm progress and no UI freeze.
9. Cancel a transfer, send a chat, and send another file.
10. Disconnect/reconnect Wi-Fi and verify rediscovery. Resume is not implemented in this build; retry fresh.
11. Close/reopen both apps and confirm the UUID/fingerprint shown in Diagnostics remains stable.
12. Capture traffic if possible. Expected destinations are private LAN addresses and mDNS multicast only.
