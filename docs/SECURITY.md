# Security

## Implemented

- Direct LAN transport is encrypted with TLS 1.3.
- Files are written outside the final name until size and SHA-256 verification succeed.
- Receiver filenames are basenames only; separators, NUL, `.` and `..` are rejected.
- Network frames and metadata have explicit bounds.
- Private file contents and chat text are not logged by the current code.
- A persistent device UUID is created under LocalShare application data and a short SHA-256-derived fingerprint is advertised.

## Identity and trust

The current build generates a self-signed certificate for each process, but
the application handshake advertises a persistent device UUID-derived
fingerprint. A first-seen peer is shown as `trust_required` and must be
explicitly trusted. Trusted fingerprints are stored in SQLite; a known device
whose fingerprint changes is rejected. The fingerprint is not a private key,
and the full secret material is never included in diagnostics.

The custom verifier still accepts the TLS certificate itself after the
fingerprint/identity exchange, so this remains `PARTIAL` rather than a
complete public-key certificate-pinning design. A future hardening step can
persist a certificate/public key in native secure storage without changing the
LAN protocol boundary.

Secure Keychain/Credential Manager storage, persistent public-key identity, and certificate pinning are still planned. The current SQLite trust database is connected to first-contact approval and changed-fingerprint rejection, but the fingerprint is derived from the persistent device UUID rather than the process certificate. Rustls provider initialization is explicit at process startup, preventing the previous packaged runtime panic.
