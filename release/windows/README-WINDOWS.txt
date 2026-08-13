LocalShare Windows release
==========================

Primary installer: LocalShare-Setup.exe

This installer was built by Tauri/NSIS on GitHub Actions Windows runner run
31724824519. The downloaded artifact is a genuine PE32 Nullsoft Installer
self-extracting archive.

SHA-256:
26614a23763f11e78d3ab9bda9e0fd50c0d4b69a974944295de8e21895d69e02  LocalShare-Setup.exe

The installer uses Tauri's offline WebView2 installer mode. LocalShare's
normal runtime communication is LAN-only and uses the _localshare._tcp.local.
mDNS service. Physical LAN, firewall, GUI visual, and Internet-off tests are
not claimed by the hosted runner.

On first launch, allow LocalShare through Windows Defender Firewall on Private
networks if Windows presents a firewall prompt. Do not enable Public network
access unless your network policy requires it.
