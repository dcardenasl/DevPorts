#!/usr/bin/env bash
# check.sh
# Runs locally what CI runs, and stops at the first failure with a non-zero exit
# code (a plain `cmd | tail` hides it):
#
#   tools/check.sh          formatting, clippy with -D warnings, all tests
#   tools/check.sh quick    formatting and clippy only
#   tools/check.sh full     the above plus `cargo deny check`
#
set -Eeuo pipefail
cd "$(dirname "$0")/.."

mode="${1:-}"

echo "== fmt";    cargo fmt --all --check
echo "== matrix"; python3 tools/check_matrix.py
echo "== clippy"; cargo clippy --workspace --all-targets --features devports-core/bindings -- -D warnings
if [ "$mode" != "quick" ]; then
  echo "== test"; cargo test --workspace --features devports-core/bindings
fi
if [ "$mode" = "full" ]; then
  echo "== deny"; cargo deny check
fi
echo "ok"
