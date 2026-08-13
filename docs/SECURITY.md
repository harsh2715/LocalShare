# Security

## Implemented

- Direct LAN transport is encrypted with TLS 1.3.
- Files are written outside the final name until size and SHA-256 verification succeed.
- Receiver filenames are basenames only; separators, NUL, `.` and `..` are rejected.
- Network frames and metadata have explicit bounds.
- Private file contents and chat text are not logged by the current code.
- A persistent device UUID is created under LocalShare application data and a short SHA-256-derived fingerprint is advertised.

## Important limitation

The current build generates a self-signed certificate for each process and uses a custom certificate verifier that accepts the TLS certificate without validating it against a persisted public key. This gives encryption against passive observers but does not yet provide authenticated peer identity or MITM protection. The UI does not yet show Trust once / Always trust confirmation. This is `PARTIAL`, not production-complete security.

Secure Keychain/Credential Manager storage, Ed25519 identity keys, certificate pinning, changed-key warnings, and a trust database schema are planned but not fully connected to the session handshake. Rustls provider initialization is now explicit at process startup, preventing the previous packaged runtime panic.
