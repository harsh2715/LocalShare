# Offline Guarantee Audit

## Source audit

The LAN core contains no public signaling, STUN, TURN, cloud database, or
telemetry endpoint. The optional updater has one explicit HTTPS GitHub
Releases endpoint and is isolated from startup/LAN services; it runs only when
the user or the saved update preference requests it. Frontend assets remain
bundled locally and no remote font/CDN is used.

## Allowed runtime traffic

- Loopback during Vite development only.
- Private LAN addresses for LocalShare sessions.
- mDNS multicast for `_localshare._tcp.local.` discovery.
- GitHub Releases HTTPS only for the optional signed software updater.

## Status

Static LAN audit: PASS. Packaged process-socket audit: prior evidence covers
the LAN-only build before updater activation; a fresh runtime capture should
classify the updater request separately. Packet capture with Internet
disabled: NOT VERIFIED. Physical isolated-LAN test: NOT VERIFIED.
