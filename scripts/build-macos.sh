#!/usr/bin/env bash
set -euo pipefail
LLVM_BIN="$(brew --prefix llvm)/bin"
RUST_TARGET_BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | awk '/host:/ {print $2}')/bin"
export PATH="$LLVM_BIN:$RUST_TARGET_BIN:$PATH"
command -v rust-objcopy >/dev/null

# Updater artifacts are signed only when the ignored local key or a CI secret
# is available. The private key is never committed or printed.
if [[ -f ".localshare-private/updater.key" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY="$(<.localshare-private/updater.key)"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
fi

# Tauri/Cargo can leave binaries from an earlier multi-target build in the
# generated app directory. Remove only this generated bundle, never sources.
BUNDLE_DIR="src-tauri/target/release/bundle/macos"
APP_DIR="$BUNDLE_DIR/LocalShare.app"
DMG_PATH="$BUNDLE_DIR/LocalShare_0.1.0_$(rustc -vV | awk '/host:/ {print $2}' | sed 's/-apple-darwin//').dmg"
rm -rf "$APP_DIR"
rm -f "$BUNDLE_DIR"/rw.*.dmg "$DMG_PATH"
rm -f "src-tauri/target/release/two_node"

# Build and verify the application bundle first. A plain hdiutil image keeps
# packaging usable in headless/offline sessions where Tauri's Finder styling
# AppleScript can hang while the app itself is already correct.
npm run tauri build -- --bundles app
# Cargo exposes the test harness as a second bin target. Tauri asks Cargo to
# build all bins, so remove that test-only executable from the app payload
# after bundling; it remains available through `cargo run --bin two_node`.
rm -f "$APP_DIR/Contents/MacOS/two_node"
test -x "$APP_DIR/Contents/MacOS/localshare"
test ! -e "$APP_DIR/Contents/MacOS/two_node"
hdiutil create -volname LocalShare -srcfolder "$APP_DIR" -ov -format UDZO "$DMG_PATH" >/dev/null
echo "Built application: $APP_DIR"
echo "Built disk image: $DMG_PATH"
