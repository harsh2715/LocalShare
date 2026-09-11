# Security

## Implemented

- Direct LAN transport is encrypted with TLS 1.3.
- Files are written outside the final name until size and SHA-256 verification succeed.
- Receiver filenames are basenames only; separators, NUL, `.` and `..` are rejected.
- Network frames and metadata have explicit bounds.
- Private file contents and chat text are not logged by the current code.
- A persistent device UUID is created under LocalShare application data. A persistent Ed25519 public key is advertised by its SHA-256 fingerprint; the private key is stored in the OS keychain or a protected PKCS#8 fallback file.

## Identity and trust

The application handshake signs the device metadata and fresh nonces with the
persistent Ed25519 key. The same key signs the self-signed TLS certificate. A
first-seen peer is shown as `trust_required` and must be explicitly trusted.
Trusted public-key fingerprints are stored in SQLite; a known device whose
fingerprint changes is rejected. The full secret material is never included in
diagnostics.

The TLS client verifier rejects chains, non-Ed25519 certificates, expired
certificates, malformed certificates, and certificates whose self-signature is
invalid. The signed application handshake remains the peer-identity and trust
boundary: it binds the certificate-backed session to the advertised persistent
Ed25519 public key and requires explicit first-contact approval. Rustls provider
initialization is explicit at process startup.
