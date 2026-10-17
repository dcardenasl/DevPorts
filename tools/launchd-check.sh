#!/usr/bin/env bash
# launchd-check.sh
# Checks the supervision contract of docs/ARQUITECTURA.md section 11 against the
# real application, with a throwaway launchd job (M-15, V7):
#
#   1. the app starts when the job is loaded (RunAtLoad);
#   2. if it crashes (SIGKILL) launchd starts it again (KeepAlive);
#   3. a clean quit (SIGTERM, which is what `launchctl bootout` and a system
#      shutdown send, and what the tray's "Salir" amounts to: exit code 0) is NOT
#      restarted, and the socket is removed;
#   4. unloading the job stops the app.
#
#   tools/bundle.sh && tools/launchd-check.sh
#
# covers: M-15
# The app runs against a throwaway HOME so it touches nothing of yours; the job
# lives only in this login session (the plist is bootstrapped from a temporary
# folder, never written to ~/Library/LaunchAgents). Rebooting the Mac and then
# checking that the app is there is the one step this cannot do: do it once by
# hand. Compatible with bash 3.2 (the one macOS ships).
set -u
cd "$(dirname "$0")/.."

BIN="${1:-target/bundle/DevPorts.app/Contents/MacOS/DevPorts}"
THROTTLE=5
if [ ! -x "$BIN" ]; then
  echo "falta $BIN: ejecuta tools/bundle.sh" >&2
  exit 2
fi
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
CLI="$(cd "$(dirname "$BIN")/../Resources/bin" && pwd)/devports"

DOMAIN="gui/$(id -u)"
LABEL="com.davidcardenas.devports.check.$$"
WORK="$(mktemp -d)"
PLIST="$WORK/$LABEL.plist"
SOCKET="$WORK/devports.sock"
FAILED=0

cleanup() {
  launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1
  rm -rf "$WORK"
}
trap cleanup EXIT

check() { # check <description> <command...>
  local description="$1"
  shift
  if "$@"; then echo "  ok    $description"; else echo "  FAIL  $description"; FAILED=1; fi
}

job_pid() {
  launchctl print "$DOMAIN/$LABEL" 2>/dev/null | awk '/^[[:space:]]*pid = / { print $3; exit }'
}

wait_for_pid() { # wait_for_pid <different-from> <seconds>; prints the new pid
  local old="$1" limit="$2" waited=0 pid
  while [ "$waited" -lt $((limit * 10)) ]; do
    pid="$(job_pid)"
    if [ -n "$pid" ] && [ "$pid" != "$old" ]; then echo "$pid"; return 0; fi
    sleep 0.1
    waited=$((waited + 1))
  done
  return 1
}

# The core answers as this very process: a socket file alone can be a stale
# one left by the process that crashed.
wait_for_core() { # wait_for_core <pid> <seconds>
  local waited=0
  while [ "$waited" -lt $(($2 * 10)) ]; do
    if DEVPORTS_SOCKET="$SOCKET" "$CLI" ping 2>/dev/null | grep -q "pid $1)"; then return 0; fi
    sleep 0.1
    waited=$((waited + 1))
  done
  return 1
}

mkdir -p "$WORK/home/.config/devports"
echo "version: 1
roots: []" > "$WORK/home/.config/devports/config.yml"

cat > "$PLIST" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key><array><string>$BIN</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>ThrottleInterval</key><integer>$THROTTLE</integer>
  <key>EnvironmentVariables</key><dict>
    <key>HOME</key><string>$WORK/home</string>
    <key>DEVPORTS_SOCKET</key><string>$SOCKET</string>
  </dict>
  <key>StandardErrorPath</key><string>$WORK/stderr.log</string>
</dict>
</plist>
PLIST_EOF
plutil -lint "$PLIST" >/dev/null || exit 1

echo "== 1. RunAtLoad"
launchctl bootstrap "$DOMAIN" "$PLIST" || exit 1
PID1="$(wait_for_pid "" 8)"
check "the app started under launchd (pid ${PID1:-none})" test -n "$PID1"
[ -z "$PID1" ] && exit 1
check "its core answers as pid $PID1" wait_for_core "$PID1" 15

echo "== 2. a crash (SIGKILL) is restarted"
kill -9 "$PID1"
START="$(date +%s)"
PID2="$(wait_for_pid "$PID1" $((THROTTLE + 8)))"
ELAPSED=$(($(date +%s) - START))
check "a new process replaced pid $PID1 (pid ${PID2:-none}) after about ${ELAPSED}s" test -n "$PID2"
[ -z "$PID2" ] && exit 1
check "the new core answers as pid $PID2 (the stale socket was replaced)" wait_for_core "$PID2" 15

echo "== 3. a clean quit (SIGTERM, exit code 0) is not restarted"
kill -TERM "$PID2"
waited=0
while kill -0 "$PID2" 2>/dev/null && [ "$waited" -lt 80 ]; do sleep 0.1; waited=$((waited + 1)); done
check "the app quit" bash -c "! kill -0 $PID2 2>/dev/null"
check "its socket was removed" test ! -S "$SOCKET"
sleep $((THROTTLE + 6))
PID3="$(job_pid)"
check "no new instance appeared after ${THROTTLE}s+6s (pid ${PID3:-none})" test -z "$PID3"
LAST_EXIT="$(launchctl print "$DOMAIN/$LABEL" 2>/dev/null | awk '/last exit code/ { print $5 }')"
check "launchd saw exit code 0 (got ${LAST_EXIT:-unknown})" test "${LAST_EXIT:-}" = "0"

echo "== 4. unloading the job stops a running app"
launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1
launchctl bootstrap "$DOMAIN" "$PLIST" || exit 1
PID4="$(wait_for_pid "" 8)"
check "the job started again (pid ${PID4:-none})" test -n "$PID4"
launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1
sleep 2
check "bootout stopped the app" bash -c "! kill -0 ${PID4:-0} 2>/dev/null"
check "launchd no longer knows the job" bash -c "! launchctl print $DOMAIN/$LABEL >/dev/null 2>&1"

echo
if [ "$FAILED" -eq 0 ]; then echo "All checks passed."; else echo "Some checks FAILED."; fi
exit "$FAILED"
