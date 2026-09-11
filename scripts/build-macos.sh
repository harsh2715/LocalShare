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
APP_TAR_PATH="$BUNDLE_DIR/LocalShare.app.tar.gz"
APP_VERSION="$(awk -F'"' '/"version"/ {print $4; exit}' src-tauri/tauri.conf.json)"
DMG_PATH="$BUNDLE_DIR/LocalShare_${APP_VERSION}_$(rustc -vV | awk '/host:/ {print $2}' | sed 's/-apple-darwin//').dmg"
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

# Tauri's linker signature is not a valid app-bundle signature. Without a
# complete bundle seal, macOS Gatekeeper reports a downloaded DMG app as
# damaged. Use the available Developer ID identity when CI provides one;
# otherwise create a complete ad-hoc seal so the bundle is internally valid.
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --deep --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$APP_DIR"
else
  codesign --force --deep --sign - "$APP_DIR"
fi
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

# Tauri creates the updater archive before the post-build bundle cleanup and
# seal above. Repack it from the final app so updater downloads contain the
# same sealed bundle as the DMG. Re-sign the new archive when the updater key
# is available; never leave a signature that belongs to an older archive.
rm -f "$APP_TAR_PATH" "$APP_TAR_PATH.sig"
tar -czf "$APP_TAR_PATH" -C "$BUNDLE_DIR" "$(basename "$APP_DIR")"
if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  npm run tauri signer -- sign "$APP_TAR_PATH"
fi
hdiutil create -volname LocalShare -srcfolder "$APP_DIR" -ov -format UDZO "$DMG_PATH" >/dev/null
echo "Built application: $APP_DIR"
echo "Built disk image: $DMG_PATH"
