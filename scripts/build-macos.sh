#!/usr/bin/env bash
set -euo pipefail
LLVM_BIN="$(brew --prefix llvm)/bin"
RUST_TARGET_BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | awk '/host:/ {print $2}')/bin"
export PATH="$LLVM_BIN:$RUST_TARGET_BIN:$PATH"
command -v rust-objcopy >/dev/null
npm run tauri build -- --bundles app,dmg
