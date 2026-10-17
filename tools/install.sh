#!/usr/bin/env bash
# install.sh
# Builds the release bundle and installs it in ~/Applications, replacing a
# previous copy (the running app is quit first). The `devports` command and the
# start-at-login switch are in the app's menu (right click on the icon).
set -Eeuo pipefail
cd "$(dirname "$0")/.."

tools/bundle.sh
dest="$HOME/Applications/DevPorts.app"
mkdir -p "$HOME/Applications"
pkill -f "DevPorts.app/Contents/MacOS/DevPorts" 2>/dev/null || true
sleep 1
rm -rf "$dest"
cp -R target/bundle/DevPorts.app "$dest"
codesign --force --deep --sign - "$dest"
open "$dest"
echo "Instalada y abierta: $dest"
