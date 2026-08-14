# Windows VM Build Report

Audit date: 2026-08-13.

This VM report is retained as historical host evidence only. The current
Windows release path does not use a local VM; it uses
`.github/workflows/windows-release.yml` on GitHub's Windows hosted runner.

## Host

| Item | Evidence |
| --- | --- |
| Host OS | macOS 26.5.1, build 25F80 |
| Host model | MacBookAir10,1 (Apple Silicon / M1-class) |
| CPU | 8 logical cores |
| RAM | 8 GiB |
| Free disk observed | approximately 45 GiB on `/` |
| Guest network | none available because no guest exists |

## Existing virtualization inventory

The initial inventory found no installed virtualization application. During this task, the official Homebrew UTM cask was installed at `/Applications/UTM.app` (UTM 4.7.5) and `/opt/homebrew/bin/utmctl` was linked. No Windows guest has been created.

The following application locations were checked before installing UTM and were absent:

- `/Applications/UTM.app`
- `/Applications/VMware Fusion.app`
- `/Applications/Parallels Desktop.app`
- `/Applications/VirtualBox.app`

No `.utm`, `.vmwarevm`, `.pvm`, `.vdi`, `.qcow2`, `.vhdx`, or Windows `.iso` was found in the inspected user scope. `VBoxManage`-style files under `/usr/local/bin` are stale wrappers pointing to a missing VirtualBox application. `utmctl list` reported no registered VM. Docker is installed as a client, but its local daemon/socket was unavailable; Docker would not satisfy the required Windows desktop runtime validation in any case.

## Historical local-VM boundary (not used for the release)

There is no safe automated way to create a licensed Windows installation here. Installing UTM was completed from the trusted Homebrew cask, but inspecting the UTM setup window is currently blocked because macOS is locked and Computer Use cannot unlock it. After the user unlocks the Mac, the next step is to create a VM and select legitimate Windows media. Accepting a Windows license and approving macOS security prompts may require user decisions, credentials, or GUI interaction. Those steps are the exact current blocker.

To resume the build, the user must provide or open a legitimate Windows 11 ARM64 VM on this Mac, with:

1. at least 4 guest vCPUs and enough RAM for Node/Rust/Tauri (6 GiB is a practical minimum, but this 8 GiB host is resource-constrained);
2. at least 60 GiB expandable guest storage (the current host has only about 45 GiB free, so additional disk space may be required);
3. Windows networking that can be switched to Bridged for mDNS/LAN testing;
4. a shared-folder or other explicit source-copy path for this existing LocalShare repository.

If the VM application shows an installer or security dialog, choose the legitimate Windows 11 ARM64 installation media, accept the applicable license terms, and approve only the requested virtualization/network permissions. Do not disable the host firewall or use an unofficial Windows image.

## Preferred hosted-runner build steps

Push this existing project to its intended GitHub repository and run the `LocalShare Windows release` workflow. It provisions Node 22 from `.nvmrc`, stable Rust, the x64 MSVC target, and executes the locked build/test/package pipeline. No local VM or Windows installation is required.

For a manual Windows developer build only, copy the complete repository to a clean folder such as `C:\LocalShare`, excluding stale macOS `node_modules`, `dist`, and `src-tauri\target` if desired. Install the project’s required Windows build prerequisites and run:

```bat
C:\LocalShare\BUILD-WINDOWS.cmd
```

The script performs the locked frontend install, frontend build, Rust checks/tests, an explicit `x86_64-pc-windows-msvc` Tauri NSIS/MSI bundle, production-binary check, PE check, release copying, and SHA-256 generation. On an ARM64 guest, install that x64 Rust target and the Visual C++ x64 build tools; the script does not silently produce an ARM64 artifact. It does not claim completion unless `release\windows\LocalShare-Setup.exe` actually exists.

## Current classification

```text
WINDOWS VM: NOT REQUIRED
WINDOWS RUNNER: PASS — GitHub Actions run 31783746842
VM NETWORK: NOT APPLICABLE TO HOSTED BUILD
WINDOWS BUILD: PASS — hosted workflow completed
```
