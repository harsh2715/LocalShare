# PairDrop Reference Analysis

Reviewed the public PairDrop README, FAQ, host-your-own guide, and technical behavior documentation on 2026-08-13. PairDrop is a browser-based file-sharing application with a simple nearby-peer experience. The public project describes a Node server, WebRTC data channels, WebSocket signaling/fallback, browser persistence, and optional STUN/TURN configuration.

## Concepts reused

- Nearby device list and a lightweight peer-oriented flow.
- One-to-one text messages alongside file requests.
- File manifests, progress, acceptance, and simple failure states.
- Local persistence of UX state.

## Concepts deliberately replaced

PairDrop’s browser signaling and WebRTC path is not the basis of LocalShare. PairDrop’s host-your-own documentation explicitly discusses signaling servers, STUN/TURN, and a WebSocket fallback. LocalShare instead uses native direct LAN sockets, mDNS/DNS-SD, and TLS-encrypted sessions, so no Internet signaling or NAT traversal service is required.

## Licensing

PairDrop is GPL-3.0. This project does not copy PairDrop source code, assets, branding, or UI markup. The design is an original implementation based on public behavior and protocol/networking concepts. PairDrop links used for research: [README](https://github.com/schlagmichdoch/PairDrop/blob/master/README.md), [FAQ](https://github.com/schlagmichdoch/PairDrop/blob/master/docs/faq.md), and [host-your-own](https://github.com/schlagmichdoch/PairDrop/blob/master/docs/host-your-own.md).
