#!/usr/bin/env bash
# bundle.sh
# Builds DevPorts.app (the menu-bar application with the core inside) and the
# `devports` command, and seals the bundle with an ad-hoc signature, which
# macOS needs before it lets the app post notifications (T0.6, V6).
#
#   tools/bundle.sh            builds into target/bundle/
#   tools/bundle.sh --debug    faster build, bigger and slower binaries
#
# The result:
#   target/bundle/DevPorts.app                      the application
#   target/bundle/DevPorts.app/Contents/Resources/bin/devports   the CLI
#
# Compatible with bash 3.2 (the one macOS ships).
set -Eeuo pipefail
cd "$(dirname "$0")/.."

profile="release"
flag="--release"
if [ "${1:-}" = "--debug" ]; then profile="debug"; flag=""; fi

out="target/bundle"
app="$out/DevPorts.app"
version="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"

echo "== interface"
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui run build

echo "== rust ($profile)"
# shellcheck disable=SC2086
cargo build $flag -p devports-app -p devports-cli

echo "== bundle"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/bin"
cp "target/$profile/devports-app" "$app/Contents/MacOS/DevPorts"
cp "target/$profile/devports" "$app/Contents/Resources/bin/devports"

# The icon: the PNG as an .icns.
if sips -s format icns crates/devports-app/icons/icon.png --out "$app/Contents/Resources/AppIcon.icns" >/dev/null 2>&1; then
  icon_key='<key>CFBundleIconFile</key><string>AppIcon</string>'
else
  icon_key=''
fi

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>DevPorts</string>
  <key>CFBundleDisplayName</key><string>DevPorts</string>
  <key>CFBundleIdentifier</key><string>com.davidcardenas.devports</string>
  <key>CFBundleExecutable</key><string>DevPorts</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  $icon_key
</dict>
</plist>
PLIST

echo "== seal"
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict "$app"

echo
echo "Listo: $app"
echo "Abrir:   open $app"
echo "CLI:     ln -sf \"\$PWD/$app/Contents/Resources/bin/devports\" ~/.local/bin/devports"
