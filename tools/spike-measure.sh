#!/usr/bin/env bash
# spike-measure.sh
# Measures the menu-bar app's memory and CPU through its autopilot timeline
# (idle, panel open, closed, open, closed, counter), to answer V5 and V10 of
# docs/ARQUITECTURA.md: does destroying the panel's webview give the memory back,
# and does the idle app fit the budget (< 50 MB, < 1 % CPU)?
#
# The binary must be built with the `spike-autopilot` feature:
#   cargo build --release -p devports-app --features spike-autopilot
#   tools/spike-measure.sh target/release/devports-app [seconds-per-phase]
#
# Memory is the physical footprint reported by `footprint(1)`, the number Activity
# Monitor shows as "Memory". RSS counts the system frameworks shared with every
# other app and overstates the cost several times over (a bare tray app has an
# RSS near 100 MB and a footprint of about 35 MB), so RSS is printed only for
# reference.
#
# WebKit runs the page in separate helper processes that are not children of the
# app, so they are found by diffing the set of WebKit processes before and after
# launch. Any other app that starts a web view while this runs adds noise.
# Compatible with bash 3.2 (the one macOS ships). Read-only apart from launching
# the app, which quits by itself at the end of the timeline.

set -u
BIN="${1:-}"
PHASE="${2:-20}"
INTERVAL=2

if [ -z "$BIN" ] || [ ! -x "$BIN" ]; then
  sed -n '2,17p' "$0"
  exit 2
fi

OUT="$(mktemp -d)"
ERR="$OUT/stderr.log"
SAMPLES="$OUT/samples.tsv"

footprint_mb() {
  # `footprint -p PID` prints "Footprint: 35 MB" (KB, MB or GB).
  footprint -p "$1" 2>/dev/null | awk '/Footprint:/ {
      for (i = 1; i <= NF; i++) if ($i == "Footprint:") { v = $(i + 1); u = $(i + 2); break }
      if (u == "KB") v = v / 1024; else if (u == "GB") v = v * 1024
      printf "%.1f", v; exit }'
}

webkit_pids() {
  ps -axo pid=,command= | awk '/com\.apple\.WebKit/ && !/awk/ { print $1 }' | sort -n
}

cpu_seconds() {
  # `ps -o time=` prints [[h:]m:]s.cc; convert to seconds.
  ps -o time= -p "$1" 2>/dev/null | awk -F: '{ s = 0; for (i = 1; i <= NF; i++) s = s * 60 + $i; print s }'
}

BASE="$(webkit_pids | tr '\n' ' ')"

DEVPORTS_SPIKE_PHASE="$PHASE" "$BIN" 2>"$ERR" &
APP=$!
sleep 1
if ! kill -0 "$APP" 2>/dev/null; then
  echo "The app exited immediately:"
  cat "$ERR"
  exit 1
fi

printf 't\tmarker\tmain_mb\tmain_rss_mb\tcpu_pct\twebkit_n\twebkit_mb\n' > "$SAMPLES"
START=$(date +%s)
LAST_CPU="$(cpu_seconds "$APP")"
while kill -0 "$APP" 2>/dev/null; do
  sleep "$INTERVAL"
  NOW=$(date +%s)
  MARKER="$(grep -o 'autopilot: .*' "$ERR" | tail -1 | sed 's/autopilot: //')"
  MAIN_KB="$(ps -o rss= -p "$APP" 2>/dev/null | tr -d ' ')"
  [ -z "$MAIN_KB" ] && break
  CPU="$(cpu_seconds "$APP")"
  PCT="$(awk -v a="$LAST_CPU" -v b="$CPU" -v i="$INTERVAL" 'BEGIN { printf "%.2f", (b - a) / i * 100 }')"
  LAST_CPU="$CPU"
  MAIN_FP="$(footprint_mb "$APP")"
  WEB_N=0
  WEB_MB=0
  for PID in $(ps -axo pid=,command= | awk '/com\.apple\.WebKit/ && !/awk/ { print $1 }'); do
    case " $BASE " in *" $PID "*) continue ;; esac
    FP="$(footprint_mb "$PID")"
    [ -z "$FP" ] && continue
    WEB_N=$((WEB_N + 1))
    WEB_MB="$(awk -v a="$WEB_MB" -v b="$FP" 'BEGIN { printf "%.1f", a + b }')"
  done
  printf '%s\t%s\t%s\t%.1f\t%s\t%s\t%s\n' "$((NOW - START))" "${MARKER:-start}" "${MAIN_FP:-0}" "$(awk -v k="$MAIN_KB" 'BEGIN { print k / 1024 }')" "$PCT" "$WEB_N" "$WEB_MB" >> "$SAMPLES"
done
wait "$APP" 2>/dev/null

echo "== Samples (every ${INTERVAL}s, ${PHASE}s per phase) =="
cat "$SAMPLES"
echo
echo "== Per phase: footprint MB of the app (avg), its RSS (avg), CPU % (avg), WebKit helpers (max) and their footprint MB (max) =="
awk -F'\t' 'NR > 1 {
    m = $2; n[m]++; fp[m] += $3; rss[m] += $4; cpu[m] += $5
    if ($6 > wn[m]) wn[m] = $6
    if ($7 > wm[m]) wm[m] = $7
    if (!(m in order)) { order[m] = ++count; names[count] = m }
  }
  END {
    printf "%-10s %10s %9s %8s %7s %9s %9s\n", "phase", "app_fp_mb", "app_rss", "cpu_pct", "web_n", "web_fp_mb", "total_mb"
    for (i = 1; i <= count; i++) {
      m = names[i]
      printf "%-10s %10.1f %9.1f %8.2f %7d %9.1f %9.1f\n", m, fp[m] / n[m], rss[m] / n[m], cpu[m] / n[m], wn[m], wm[m], fp[m] / n[m] + wm[m]
    }
  }' "$SAMPLES"
echo
echo "App messages:"
cat "$ERR"
echo "Raw data kept in $OUT"
