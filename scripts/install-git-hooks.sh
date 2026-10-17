#!/usr/bin/env bash
#
# Sync the repository's git hooks into .git/hooks.
#
# Runs from npm's `prepare` lifecycle, which also fires in places that have no
# git directory at all: an extracted archive, a vendored copy, a container
# build, a CI job that fetched without history. Installing developer hooks is a
# convenience, so its absence must never fail the install.
#
# This syncs rather than only copies: a hook removed from the repository is
# removed from .git/hooks too. Otherwise a hook deleted upstream keeps running
# on every machine that installed it once.
#
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

HOOKS_DIR="${ROOT_DIR}/.git/hooks"

if [[ ! -d "${HOOKS_DIR}" ]]; then
    echo "No .git/hooks directory; skipping git hook installation."
    exit 0
fi

# Hooks this repository manages. Listed explicitly so the sync never touches a
# hook someone installed for themselves.
MANAGED_HOOKS=(pre-commit pre-push commit-msg)

for hook in "${MANAGED_HOOKS[@]}"; do
    source_file="${ROOT_DIR}/${hook}"
    target_file="${HOOKS_DIR}/${hook}"

    if [[ -f "${source_file}" ]]; then
        chmod +x "${source_file}"
        cp "${source_file}" "${target_file}"
        chmod +x "${target_file}"
        echo "Installed ${hook} hook."
    elif [[ -f "${target_file}" ]]; then
        rm -f "${target_file}"
        echo "Removed ${hook} hook; it is no longer part of the repository."
    fi
done
