#!/usr/bin/env bash
# devports-baseline.sh
# Mide la linea base del problema: cuantos servidores locales hay a la vez
# y cuantos parecen duplicados (misma carpeta + mismo comando, distinto puerto).
# SOLO LEE (lsof, ps, docker ps). No mata ni modifica nada.
# Compatible con bash 3.2 (el de macOS).
#
# Uso:
#   ./baseline.sh start    inicia el muestreo en segundo plano (cada 30 s)
#   ./baseline.sh status   dice si esta corriendo
#   ./baseline.sh once     toma una sola muestra (para probar)
#   ./baseline.sh report   resumen por dia, por hora y top de duplicados
#   ./baseline.sh stop     detiene el muestreo
#
# Datos: ~/.devports-baseline/samples.tsv (cambia con DEVPORTS_BASELINE_DIR)
# Intervalo: DEVPORTS_INTERVAL=30 (segundos)

set -u
DIR="${DEVPORTS_BASELINE_DIR:-$HOME/.devports-baseline}"
LOG="$DIR/samples.tsv"
PIDFILE="$DIR/sampler.pid"
INTERVAL="${DEVPORTS_INTERVAL:-30}"
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
TAB=$(printf '\t')
mkdir -p "$DIR"

is_running() {
  [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null
}

sample_once() {
  local ts
  ts=$(date +%Y-%m-%dT%H:%M:%S)
  [ -s "$LOG" ] || printf 'ts\tkind\tpid\tppid\tport\tcwd\tstarted\targs\n' > "$LOG"

  # Procesos del host que escuchan TCP. lsof -F: lineas "p<pid>" y "n<direccion:puerto>"
  lsof -nP -iTCP -sTCP:LISTEN -Fpn 2>/dev/null | awk '
    /^p/ { pid = substr($0, 2) }
    /^n/ { n = substr($0, 2); sub(/.*:/, "", n); print pid "\t" n }
  ' | sort -u | while IFS="$TAB" read -r pid port; do
    args=$(ps -o args= -p "$pid" 2>/dev/null | tr '\t' ' ')
    [ -z "$args" ] && continue
    ppid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')
    started=$(ps -o lstart= -p "$pid" 2>/dev/null | sed 's/  */ /g')
    cwd=$(lsof -a -p "$pid" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p' | head -1)
    printf '%s\thost\t%s\t%s\t%s\t%s\t%s\t%s\n' "$ts" "$pid" "$ppid" "$port" "$cwd" "$started" "$args"
  done >> "$LOG"

  # Contenedores Docker (si hay docker y el daemon responde)
  if command -v docker >/dev/null 2>&1; then
    docker ps --format '{{.Names}}|{{.Image}}|{{.Ports}}|{{.Label "com.docker.compose.project"}}' 2>/dev/null \
      | while IFS='|' read -r name image ports project; do
          printf '%s\tdocker\t-\t-\t%s\t%s\t-\t%s %s\n' "$ts" "$ports" "$project" "$name" "$image"
        done >> "$LOG"
  fi
}

cmd_start() {
  if is_running; then echo "Ya esta corriendo (pid $(cat "$PIDFILE"))."; return 0; fi
  nohup "$SELF" _loop >/dev/null 2>&1 &
  echo $! > "$PIDFILE"
  echo "Muestreo iniciado cada ${INTERVAL}s. Datos en: $LOG"
}

cmd_stop() {
  if is_running; then
    kill "$(cat "$PIDFILE")" && rm -f "$PIDFILE" && echo "Detenido."
  else
    rm -f "$PIDFILE"; echo "No estaba corriendo."
  fi
}

cmd_status() {
  if is_running; then echo "Corriendo (pid $(cat "$PIDFILE"))."; else echo "Detenido."; fi
  [ -s "$LOG" ] && echo "Lineas en el log: $(wc -l < "$LOG")"
}

# mode: day | hour | top
agg() {
  awk -F'\t' -v home="$HOME" -v mode="$1" '
    NR == 1 && $1 == "ts" { next }
    { ts = $1; allts[ts] = 1 }
    $2 == "host" && index($6, home) == 1 {
      pk = ts SUBSEP $3
      if (!(pk in seen)) {
        seen[pk] = 1; cnt[ts]++
        a = $8; gsub(/[0-9][0-9][0-9][0-9][0-9]?/, "N", a)
        ik[ts SUBSEP $6 "|" a]++
      }
    }
    END {
      for (x in ik) {
        split(x, p, SUBSEP)
        dup[p[1]] += ik[x] - 1
        if (ik[x] > 1) { k = p[2]; if (ik[x] > best[k]) best[k] = ik[x]; times[k]++ }
      }
      if (mode == "top") {
        for (k in best) printf "%d\t%d\t%s\n", best[k], times[k], k
        exit
      }
      klen = (mode == "hour") ? 13 : 10
      for (ts in allts) {
        d = substr(ts, 1, klen)
        ns[d]++; sum[d] += cnt[ts]
        if (cnt[ts] > mx[d]) mx[d] = cnt[ts]
        if (dup[ts] > 0) nd[d]++
        if (dup[ts] > md[d]) md[d] = dup[ts]
      }
      for (d in ns) printf "%-14s %8d %10d %11.1f %11d %13.0f%%\n", d, ns[d], mx[d], sum[d]/ns[d], md[d], 100*nd[d]/ns[d]
    }' "$LOG"
}

cmd_report() {
  [ -s "$LOG" ] || { echo "Sin datos todavia. Ejecuta: $0 start"; exit 1; }
  echo "Solo cuenta procesos con carpeta de trabajo dentro de $HOME."
  echo "Duplicado = misma carpeta + mismo comando (ignorando el numero de puerto) en mas de un proceso. Es una aproximacion."
  echo
  echo "== Por dia =="
  printf '%-14s %8s %10s %11s %11s %14s\n' periodo muestras max_simult prom_simult max_duplic pct_con_dup
  agg day | sort
  echo
  echo "== Por hora =="
  printf '%-14s %8s %10s %11s %11s %14s\n' periodo muestras max_simult prom_simult max_duplic pct_con_dup
  agg hour | sort
  echo
  echo "== Top duplicados (max instancias a la vez | veces visto | carpeta|comando) =="
  agg top | sort -rn | head -10
}

case "${1:-}" in
  start)  cmd_start ;;
  stop)   cmd_stop ;;
  status) cmd_status ;;
  once)   sample_once; echo "Muestra tomada en $LOG" ;;
  report) cmd_report ;;
  _loop)  trap 'rm -f "$PIDFILE"; exit 0' TERM INT
          while true; do sample_once; sleep "$INTERVAL"; done ;;
  *) sed -n '2,17p' "$SELF" ;;
esac
