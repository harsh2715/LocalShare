# Pairdrop Virtual Lab — Stage 1 Startup

Date: 2026-08-12
Lab: `/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab`
Project: `/Users/harshsinghal/Desktop/Projects/Pairdrop`

## Setup

- Device A userData: `device-a/user-data`
- Device B userData: `device-b/user-data`
- Device A bind fallback: `127.0.0.1:33201`
- Device B bind fallback: `127.0.0.1:33202`
- No pre-existing user files were used as test data.
- The initial `127.0.0.2`/`127.0.0.3` arrangement was attempted and failed with `EADDRNOTAVAIL`; the allowed localhost/port fallback was then used.

## Evidence

- Both Electron processes started and remained running.
- Both listeners were observed with `lsof`:
  - Device A: `127.0.0.1:33201 (LISTEN)`
  - Device B: `127.0.0.1:33202 (LISTEN)`
- Fresh identity files exist independently:
  - `device-a/user-data/identity.json`, device ID `82ce0d9c-8f69-4c97-8972-bb431ea3b4d7`
  - `device-b/user-data/identity.json`, device ID `7619d287-8313-4e27-9cee-c9bf9999d654`
- Separate SQLite databases were initialized in both profiles.
- The production build and strict typecheck passed before launch.

## Result

**BLOCKED at Stage 1.** Backend startup, identity initialization, SQLite initialization, and TLS listeners passed. Renderer loading and two-window inspection could not be verified because the Mac was locked and the computer-control layer could not unlock it automatically.

No later stage was run, per the strict gating rule.

## Resume

After manually unlocking the Mac, run:

```zsh
/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab/start-devices.sh
```

Then inspect both Electron windows before proceeding to Stage 2.
