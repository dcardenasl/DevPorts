#!/usr/bin/env bash
# measure.sh
# covers: M-14
# Measures the running DevPorts against the idle budget of NF-01 (M-14):
# memory under 50 MB and CPU under 1 % with the panel closed.
#
#   tools/measure.sh [seconds]     default 3600 (one hour); use 60 for a quick look
#
# Memory is the physical footprint that `footprint(1)` reports, the number
# Activity Monitor shows as "Memory"; RSS counts the shared system frameworks and
# overstates the cost several times over. CPU is the average over the whole window. Leave the panel closed while it runs. With the panel
# open, WebKit runs the page in helper processes that are not children of the
# app: open it for a minute afterwards and read their footprints in Activity
# Monitor (process names starting with "com.apple.WebKit").
# Compatible with bash 3.2 (the one macOS ships).
set -u

SECONDS_TOTAL="${1:-3600}"
PID="$(pgrep -f 'DevPorts.app/Contents/MacOS/DevPorts' | head -n 1)"
if [ -z "$PID" ]; then
  echo "DevPorts no está en marcha" >&2
  exit 2
fi

footprint_mb() {
  footprint -p "$PID" 2>/dev/null | awk '{
    for (n = 1; n < NF; n++) if ($n == "Footprint:") {
      value = $(n + 1); unit = $(n + 2)
      if (unit == "KB") value = value / 1024
      else if (unit == "GB") value = value * 1024
      printf "%.1f\n", value; exit
    } }'
}

# CPU: the process's accumulated CPU time at the start and at the end of the
# window, divided by the window. (`top` and `ps %cpu` are decayed or per-sample
# figures that misread a quiet process; the accumulated time is exact to 10 ms.)
cpu_seconds() {
  ps -o time= -p "$PID" | awk -F: '{ if (NF == 3) print $1 * 3600 + $2 * 60 + $3; else print $1 * 60 + $2 }'
}
cpu_start="$(cpu_seconds)"

# Memory: the footprint at the start, the middle and the end of the window.
mem_min=""
mem_max=0
mem_last=0
third=$((SECONDS_TOTAL / 3))
[ "$third" -lt 1 ] && third=1
for _ in 1 2 3; do
  mem="$(footprint_mb)"
  if [ -n "$mem" ]; then
    mem_last="$mem"
    if [ -z "$mem_min" ] || [ "$(echo "$mem < $mem_min" | bc -l)" = "1" ]; then mem_min="$mem"; fi
    if [ "$(echo "$mem > $mem_max" | bc -l)" = "1" ]; then mem_max="$mem"; fi
  fi
  sleep "$third"
done
cpu_end="$(cpu_seconds)"
cpu_avg="$(echo "scale=2; ($cpu_end - $cpu_start) * 100 / ($third * 3)" | bc -l)"
cpu_max="$cpu_avg"

echo "pid $PID, ventana de $((third * 3)) s"
echo "memoria (footprint): min ${mem_min:-?} MB · máx $mem_max MB · última $mem_last MB   (presupuesto: < 50 MB)"
echo "CPU: media $cpu_avg %                       (presupuesto: < 1 %)"
fail=0
[ "$(echo "$mem_max >= 50" | bc -l)" = "1" ] && { echo "FALLA: memoria sobre el presupuesto"; fail=1; }
[ "$(echo "$cpu_avg >= 1" | bc -l)" = "1" ] && { echo "FALLA: CPU sobre el presupuesto"; fail=1; }
[ "$fail" -eq 0 ] && echo "Dentro del presupuesto."
exit "$fail"
