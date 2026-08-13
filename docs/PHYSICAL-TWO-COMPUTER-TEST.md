# Physical Two-Computer Test

Status: NOT VERIFIED in this environment. The current build has passing virtual TLS chat/single-file/hash coverage, but physical LAN and mDNS behavior still require a second computer.

1. Put a macOS computer and a Windows/macOS computer on the same Wi-Fi or Ethernet LAN.
2. Disconnect the router WAN, or use a local hotspot with no upstream Internet.
3. Confirm an ordinary website cannot load.
4. Build/install LocalShare on both machines and allow Local Network / Private Network access when the OS asks.
5. Launch both apps. Confirm each appears under Nearby devices through mDNS; if not, use Connect manually with the displayed private IP and port 32123.
6. Connect, confirm the peer fingerprint, and send chat both ways.
7. Send a zero-byte file, a Unicode filename, one file, and multiple files. Compare SHA-256 hashes with `shasum -a 256` on macOS or `Get-FileHash` on Windows.
8. Send a generated 100 MB file while exchanging chat messages. Confirm progress and no UI freeze.
9. Cancel a transfer, send a chat, and send another file.
10. Disconnect/reconnect Wi-Fi and verify rediscovery. Resume is not implemented in this build; retry fresh.
11. Close/reopen both apps and confirm the UUID/fingerprint shown in Diagnostics remains stable.
12. Capture traffic if possible. Expected destinations are private LAN addresses and mDNS multicast only.
