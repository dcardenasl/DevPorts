#!/usr/bin/env bash
# demo.sh
# Runs DevPorts against an isolated, invented setup so the interface can be tried
# without touching the real ~/.config, ~/Library or the projects in ~/Developer:
# a fake HOME under /tmp with a project "tienda-demo" (three repos), a shared
# MySQL entry, one official server, a test instance about to expire, duplicates
# that did not go through DevPorts and a stray process.
#
#   tools/demo.sh start    builds nothing; needs tools/bundle.sh to have run
#   tools/demo.sh stop     stops the demo servers and the app
#   tools/demo.sh shell    prints the environment to use the CLI against the demo
#
# The servers are plain `python3 -m http.server` on ports 28080 and up.
set -Eeuo pipefail
cd "$(dirname "$0")/.."

root="$PWD"
app="$root/target/bundle/DevPorts.app"
demo_home="${DEVPORTS_DEMO_HOME:-/tmp/devports-demo-home}"
run="$demo_home/run"
cli="$app/Contents/Resources/bin/devports"

env_for_demo() {
  export HOME="$demo_home"
  export DEVPORTS_SOCKET="$demo_home/s/devports.sock"
  export DEVPORTS_NO_AUTOSTART=1
}

stop() {
  if [ -d "$run" ]; then
    for file in "$run"/*.pid; do
      [ -e "$file" ] || continue
      kill "$(cat "$file")" 2>/dev/null || true
      rm -f "$file"
    done
  fi
  pkill -f "$app/Contents/MacOS/DevPorts" 2>/dev/null || true
  echo "demo detenida"
}

spawn() { # name dir command...
  local name="$1" dir="$2"
  shift 2
  (cd "$dir" && exec "$@" >/dev/null 2>&1) &
  echo $! > "$run/$name.pid"
}

start() {
  [ -x "$app/Contents/MacOS/DevPorts" ] || { echo "falta $app: ejecuta tools/bundle.sh" >&2; exit 1; }
  stop >/dev/null
  rm -rf "$demo_home"
  mkdir -p "$demo_home/.config/devports" "$demo_home/work" "$run" "$demo_home/s"
  local work="$demo_home/work"
  for repo in tienda-api tienda-admin tienda-bff; do
    mkdir -p "$work/tienda-demo/$repo/.git" "$work/tienda-demo/$repo/public"
    echo "ref: refs/heads/main" > "$work/tienda-demo/$repo/.git/HEAD"
  done
  echo "ref: refs/heads/develop" > "$work/tienda-demo/tienda-bff/.git/HEAD"
  mkdir -p "$work/scratch"

  cat > "$demo_home/.config/devports/config.yml" <<YAML
version: 1
roots: [$work]
observation:
  host_interval: 1s
YAML
  cat > "$demo_home/.config/devports/shared.yml" <<YAML
version: 1
services:
  mysql:
    docker: mysql
    kind: data
    port: 3306
    start: docker start mysql
YAML
  cat > "$work/tienda-demo/.devports.yml" <<YAML
version: 1
project: tienda-demo
uses: [mysql]
services:
  api:
    cwd: ./tienda-api
    port: 28080
    start: python3 -m http.server \${PORT} --bind 127.0.0.1
    match: python3 -m http.server
  admin:
    cwd: ./tienda-admin
    port: 28081
    start: python3 -m http.server \${PORT} --bind 127.0.0.1
    match: python3 -m http.server
  bff:
    cwd: ./tienda-bff
    port: 28082
    start: python3 -m http.server \${PORT} --bind 127.0.0.1
    match: python3 -m http.server
YAML

  # The app, inside the fake home so it reads and writes nothing real.
  (
    env_for_demo
    "$app/Contents/MacOS/DevPorts" --show >"$run/app.log" 2>&1 &
    echo $! > "$run/app.pid"
  )
  for _ in $(seq 1 50); do
    [ -S "$demo_home/s/devports.sock" ] && break
    sleep 0.2
  done
  sleep 6 # the first catalog pass

  (
    env_for_demo
    # The official servers go through DevPorts.
    (cd "$work/tienda-demo/tienda-api" && exec "$cli" run api >/dev/null 2>&1) &
    echo $! > "$run/api.pid"
    (cd "$work/tienda-demo/tienda-admin" && exec "$cli" run admin >/dev/null 2>&1) &
    echo $! > "$run/admin.pid"
    # A test instance that is about to expire.
    (cd "$work/tienda-demo/tienda-api" && exec "$cli" run api --as test --reason "QA login" --ttl 5m \
      -- python3 -m http.server {PORT} --bind 127.0.0.1 >/dev/null 2>&1) &
    echo $! > "$run/temporal.pid"
  )
  sleep 2
  # Servers started by hand, bypassing DevPorts: duplicates of api.
  spawn dup1 "$work/tienda-demo/tienda-api" python3 -m http.server 28090 --bind 127.0.0.1
  spawn dup2 "$work/tienda-demo/tienda-api" python3 -m http.server 28091 --bind 127.0.0.1
  # A stray process in a folder no project owns.
  spawn stray "$work/scratch" python3 -m http.server 28099 --bind 127.0.0.1

  echo
  echo "demo en marcha (HOME aislado en $demo_home)"
  echo "  CLI:   source <(tools/demo.sh shell) && devports ls"
  echo "  parar: tools/demo.sh stop"
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  shell)
    echo "export HOME=$demo_home"
    echo "export DEVPORTS_SOCKET=$demo_home/s/devports.sock"
    echo "export DEVPORTS_NO_AUTOSTART=1"
    echo "export PATH=$app/Contents/Resources/bin:\$PATH"
    ;;
  *) sed -n '2,15p' "$0"; exit 2 ;;
esac
