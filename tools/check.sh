#!/usr/bin/env bash
# check.sh
# Runs locally what CI runs, and stops at the first failure with a non-zero exit
# code (a plain `cmd | tail` hides it):
#
#   tools/check.sh          formatting, clippy with -D warnings, all tests
#   tools/check.sh quick    formatting and clippy only
#   tools/check.sh full     the above plus `cargo deny check`
#
# The spike features are enabled so that the code behind them is linted too.
set -Eeuo pipefail
cd "$(dirname "$0")/.."

mode="${1:-}"
features="devports-app/spike-autopilot,devports-app/spike-notify"

echo "== fmt";    cargo fmt --all --check
echo "== clippy"; cargo clippy --workspace --all-targets --features "$features" -- -D warnings
if [ "$mode" != "quick" ]; then
  echo "== test"; cargo test --workspace --features "$features"
fi
if [ "$mode" = "full" ]; then
  echo "== deny"; cargo deny check
fi
echo "ok"
