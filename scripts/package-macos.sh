#!/usr/bin/env bash
set -euo pipefail

# Packages Glow Reading into a macOS .app bundle and drag-to-Applications .dmg.
# Optional signing & notarization env vars:
#   APPLE_SIGNING_IDENTITY   e.g. "Developer ID Application: Glow Reading Studio (TEAMID)"
#   APPLE_NOTARY_PROFILE     keychain profile configured via `xcrun notarytool store-credentials`
#   GLOW_MACOS_TARGET        optional Rust target triple (e.g. aarch64-apple-darwin or x86_64-apple-darwin)
#   SKIP_BUILD=1             reuse already compiled release binary

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

VERSION="$(python3 -c 'import tomllib, pathlib; print(tomllib.loads(pathlib.Path("Cargo.toml").read_text())["workspace"]["package"]["version"])')"
ARCH_LABEL="${GLOW_MACOS_ARCH:-$(uname -m)}"
TARGET_FLAG=()
BIN_PATH="target/release/glow-reading"
if [[ -n "${GLOW_MACOS_TARGET:-}" ]]; then
  TARGET_FLAG=(--target "$GLOW_MACOS_TARGET")
  BIN_PATH="target/${GLOW_MACOS_TARGET}/release/glow-reading"
fi

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  (cd web && npm ci --no-fund && npm run build)
  cargo build --release --locked -p glow-reading "${TARGET_FLAG[@]}"
fi

DIST_DIR="$ROOT_DIR/dist"
STAGE_DIR="$DIST_DIR/macos-${ARCH_LABEL}"
APP_BUNDLE="$STAGE_DIR/Glow Reading.app"
DMG_PATH="$DIST_DIR/glow-reading-${VERSION}-macos-${ARCH_LABEL}.dmg"

rm -rf "$STAGE_DIR" "$DMG_PATH" "${DMG_PATH}.sha256"
mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources/docs"

cp "$BIN_PATH" "$APP_BUNDLE/Contents/MacOS/glow-reading"
chmod +x "$APP_BUNDLE/Contents/MacOS/glow-reading"
cp README.md LICENSE THIRD_PARTY_NOTICES.md "$APP_BUNDLE/Contents/Resources/"
cp assets/SOURCES.md assets/quotes.json assets/glow-icon.png "$APP_BUNDLE/Contents/Resources/"
cp docs/ROADMAP.md docs/TRYOUT.md docs/VALIDATION.md "$APP_BUNDLE/Contents/Resources/docs/"

cat > "$APP_BUNDLE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>zh_CN</string>
  <key>CFBundleDisplayName</key>
  <string>微光阅读</string>
  <key>CFBundleExecutable</key>
  <string>glow-reading</string>
  <key>CFBundleIdentifier</key>
  <string>studio.glowreading.desktop</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Glow Reading</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>15.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "Signing app bundle with Developer ID..."
  codesign --force --deep --options runtime --timestamp \
    --sign "$APPLE_SIGNING_IDENTITY" "$APP_BUNDLE"
  codesign --verify --deep --strict "$APP_BUNDLE"
fi

ln -s /Applications "$STAGE_DIR/Applications"
hdiutil create -volname "Glow Reading ${VERSION}" \
  -srcfolder "$STAGE_DIR" -ov -format UDZO "$DMG_PATH"

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$DMG_PATH"
fi

if [[ -n "${APPLE_NOTARY_PROFILE:-}" ]]; then
  echo "Submitting DMG for Apple notarization..."
  xcrun notarytool submit "$DMG_PATH" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG_PATH"
fi

shasum -a 256 "$DMG_PATH" | awk '{print $1}' > "${DMG_PATH}.sha256"
echo "Packaged: $DMG_PATH"
