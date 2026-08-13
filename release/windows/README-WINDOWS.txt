LocalShare Windows release handoff
==================================

This directory does not contain a Windows executable yet. The current host is
an Apple Silicon Mac without a Windows VM or Windows build toolchain, so no
fake .exe has been created.

The preferred build path is `.github/workflows/windows-release.yml` on a
GitHub-hosted `windows-latest` runner. Push this existing project to its
intended GitHub repository and run the `LocalShare Windows release` workflow.
The workflow runs the frontend build, Rust checks/tests, an explicit
x86_64-pc-windows-msvc Tauri NSIS bundle, PE validation,
production-binary validation, artifact copying, and SHA-256 generation. It
writes/uploads the genuine primary installer as:

    release\windows\LocalShare-Setup.exe

It also writes LocalShare.exe, RUN-LOCALSHARE.cmd, README-WINDOWS.txt, and
SHA256SUMS.txt when those artifacts are actually built. The normal end user
needs only LocalShare-Setup.exe; Node.js, npm, Rust, Cargo, Git, and the
source tree are not required after installation.

The configured installer uses Tauri's offline WebView2 installer mode. The
resulting installer is expected to be large because it carries the WebView2
runtime. GitHub Actions builds it but does not perform Windows GUI, LAN, or
physical Mac↔Windows testing.
