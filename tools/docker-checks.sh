#!/usr/bin/env bash
# docker-checks.sh
# The checks that need a real Docker daemon (GitHub's macOS runners have none):
# run them on the Mac before every release.
#
#   tools/docker-checks.sh
#
# covers: D-01, D-02, D-03, D-04, D-05
#   D-01 and D-03  the source against the daemon (labels, events in 3 s)
#   D-02 and D-04  the whole product: a container Temporal is stopped and kept,
#                  a data container is left running
#   D-05           reports which runtime is in use (Docker Desktop, OrbStack, Colima)
#
# Needs the ubuntu:24.04 image. Everything it creates is named devports-test-*
# and removed at the end. Compatible with bash 3.2.
set -Eeuo pipefail
cd "$(dirname "$0")/.."

if ! docker info >/dev/null 2>&1; then
  echo "Docker no responde: inicia Docker Desktop, OrbStack o Colima" >&2
  exit 1
fi

echo "== D-05 runtime"
echo "contexto:  $(docker context show)"
echo "servidor:  $(docker info --format '{{.Name}} · {{.OperatingSystem}} · {{.ServerVersion}}')"
case "$(docker info --format '{{.OperatingSystem}}')" in
  *OrbStack*) echo "runtime:   OrbStack" ;;
  *Docker\ Desktop*) echo "runtime:   Docker Desktop" ;;
  *) echo "runtime:   $(docker info --format '{{.OperatingSystem}}') (Colima u otro)" ;;
esac

docker image inspect ubuntu:24.04 >/dev/null 2>&1 || docker pull ubuntu:24.04

echo "== D-01, D-03 the source against the daemon"
cargo test -p devports-core --test docker_engine -- --ignored

echo "== D-02, D-04 the product against the daemon"
cargo test -p devports-cli --test docker_real -- --ignored

echo "ok"
