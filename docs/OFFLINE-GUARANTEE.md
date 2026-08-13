# Offline Guarantee Audit

## Source audit

The application source contains no configured `http://`, `https://`, `ws://`, `wss://`, `stun:`, or `turn:` runtime endpoint. The only `https://` string is the Tauri configuration schema URL and documentation links; it is not fetched by the runtime. Frontend assets are bundled locally and no web fonts, analytics, crash reporting, updater, remote config, or cloud database is configured.

## Allowed runtime traffic

- Loopback during Vite development only.
- Private LAN addresses for LocalShare sessions.
- mDNS multicast for `_localshare._tcp.local.` discovery.

## Status

Static audit: PASS. Packaged process-socket audit: PASS for LocalShare-owned sockets; evidence: `artifacts/test-results/offline-runtime-audit.txt`. Packet capture with Internet disabled: NOT VERIFIED. Physical isolated-LAN test: NOT VERIFIED.
