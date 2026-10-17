#!/usr/bin/env bash
# spike-notify.sh
# Builds DevPorts.app with the notification spike, seals it, runs it and shows
# what happens, so the person at the Mac can answer V6 by looking at the screen:
#
#   1. macOS shows "Notificaciones de DevPorts": move the pointer over the banner
#      and press "Permitir" (the first run only);
#   2. a notification "api vence en 1 minuto" appears with a button
#      "Extender 30 min": press it, then run the script again and press the
#      body of the notification instead;
#   3. this script prints "notify: user performed Extend" or "... Opened".
#
# UNUserNotificationCenter refuses an application whose bundle is not sealed
# (it answers "not allowed" without asking), and Tauri's debug bundle only has a
# linker signature that is not bound to the Info.plist, so the bundle is sealed
# with an ad hoc signature here, as the release pipeline must do too.
# Compatible with bash 3.2.

set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/target/debug/bundle/macos/DevPorts.app"
LOG="$(mktemp)"

(cd "$ROOT/crates/devports-app" && npx --yes @tauri-apps/cli@2.12.3 build --debug --bundles app --ci -- --features spike-notify)
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

pkill -x devports-app 2>/dev/null || true
open -g -a "$APP" --env DEVPORTS_SPIKE_NOTIFY_AFTER=2 --stderr "$LOG"
echo "Waiting 60 s for you to act on the notification. Log: $LOG"
tail -f "$LOG" &
TAIL=$!
sleep 60
kill "$TAIL" 2>/dev/null || true
pkill -x devports-app 2>/dev/null || true
