#!/usr/bin/env bash
# spike-launchd.sh
# Checks, with a throwaway launchd job, the supervision contract of
# docs/ARQUITECTURA.md section 11 (V7):
#
#   1. the app starts when the job is loaded (RunAtLoad);
#   2. if it dies from a crash (SIGKILL) launchd starts it again (KeepAlive);
#   3. if it quits cleanly with exit code 0, as the tray's "Salir" does, launchd
#      does NOT start it again (KeepAlive with SuccessfulExit = false);
#   4. unloading the job stops the app.
#
# The binary must be built with the autopilot feature, which makes the app quit
# with code 0 on its own after six phases:
#   cargo build --release -p devports-app --features spike-autopilot
#   tools/spike-launchd.sh target/release/devports-app
#
# The job lives only in the current login session: the plist is written to a
# temporary folder and bootstrapped from there, never into ~/Library/LaunchAgents,
# and an exit trap removes it. Compatible with bash 3.2 (the one macOS ships).

set -u
BIN="${1:-}"
PHASE="${DEVPORTS_SPIKE_PHASE:-4}"
THROTTLE=5

if [ -z "$BIN" ] || [ ! -x "$BIN" ]; then
  sed -n '2,20p' "$0"
  exit 2
fi
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

DOMAIN="gui/$(id -u)"
LABEL="com.davidcardenas.devports.spike.$$"
WORK="$(mktemp -d)"
PLIST="$WORK/$LABEL.plist"
FAILED=0

cleanup() {
  launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1
  rm -rf "$WORK"
}
trap cleanup EXIT

check() { # check <description> <command...>
  local description="$1"
  shift
  if "$@"; then
    echo "  ok    $description"
  else
    echo "  FAIL  $description"
    FAILED=1
  fi
}

job_pid() {
  launchctl print "$DOMAIN/$LABEL" 2>/dev/null | awk '/^[[:space:]]*pid = / { print $3; exit }'
}

wait_for_pid() { # wait_for_pid <different-from> <seconds>; prints the new pid
  local old="$1" limit="$2" waited=0 pid
  while [ "$waited" -lt $((limit * 10)) ]; do
    pid="$(job_pid)"
    if [ -n "$pid" ] && [ "$pid" != "$old" ]; then
      echo "$pid"
      return 0
    fi
    sleep 0.1
    waited=$((waited + 1))
  done
  return 1
}

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
  <key>EnvironmentVariables</key><dict><key>DEVPORTS_SPIKE_PHASE</key><string>$PHASE</string></dict>
  <key>StandardErrorPath</key><string>$WORK/stderr.log</string>
</dict>
</plist>
PLIST_EOF
plutil -lint "$PLIST" >/dev/null || exit 1

echo "== 1. RunAtLoad"
launchctl bootstrap "$DOMAIN" "$PLIST" || exit 1
PID1="$(wait_for_pid "" 5)"
check "the app started under launchd (pid ${PID1:-none})" test -n "$PID1"
[ -z "$PID1" ] && exit 1

echo "== 2. crash (SIGKILL) is restarted"
sleep 1
kill -9 "$PID1"
START="$(date +%s)"
PID2="$(wait_for_pid "$PID1" $((THROTTLE + 8)))"
ELAPSED=$(($(date +%s) - START))
check "a new process replaced pid $PID1 (pid ${PID2:-none}) after about ${ELAPSED}s" test -n "$PID2"
[ -z "$PID2" ] && exit 1

echo "== 3. exit code 0 is not restarted"
# The autopilot ends with app.exit(0), the same call as the tray's "Salir".
waited=0
while kill -0 "$PID2" 2>/dev/null && [ "$waited" -lt $((PHASE * 10 * 8)) ]; do
  sleep 0.1
  waited=$((waited + 1))
done
check "the second instance quit by itself" bash -c "! kill -0 $PID2 2>/dev/null"
sleep $((THROTTLE + 6))
PID3="$(job_pid)"
check "no third instance appeared after ${THROTTLE}s+6s (pid ${PID3:-none})" test -z "$PID3"
LAST_EXIT="$(launchctl print "$DOMAIN/$LABEL" 2>/dev/null | awk '/last exit code/ { print $5 }')"
echo "  info  last exit code reported by launchd: ${LAST_EXIT:-unknown}"

echo "== 4. unloading the job stops a running app"
launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1
launchctl bootstrap "$DOMAIN" "$PLIST" || exit 1
PID4="$(wait_for_pid "" 5)"
check "the job started again (pid ${PID4:-none})" test -n "$PID4"
launchctl bootout "$DOMAIN/$LABEL" >/dev/null 2>&1
sleep 2
check "bootout stopped the app" bash -c "! kill -0 ${PID4:-0} 2>/dev/null"
check "launchd no longer knows the job" bash -c "! launchctl print $DOMAIN/$LABEL >/dev/null 2>&1"

echo
if [ "$FAILED" -eq 0 ]; then echo "All checks passed."; else echo "Some checks FAILED."; fi
exit "$FAILED"
