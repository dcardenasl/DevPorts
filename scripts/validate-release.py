#!/usr/bin/env python3
"""Checks that a release tag agrees with the code it points to.

    python scripts/validate-release.py v0.1.0

The version in the tag must equal the workspace version, the one of the Tauri
configuration and the one of the interface package, and CHANGELOG.md must have a
section for it (`## [0.1.0]`), which is where the release notes come from.
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def main() -> int:
    if len(sys.argv) != 2 or not re.fullmatch(r"v\d+\.\d+\.\d+", sys.argv[1]):
        print("usage: validate-release.py vX.Y.Z", file=sys.stderr)
        return 2
    version = sys.argv[1][1:]
    errors = []

    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r"\[workspace\.package\][^\[]*?^version\s*=\s*\"([^\"]+)\"", cargo, re.M | re.S)
    found = {
        "Cargo.toml (workspace)": match.group(1) if match else None,
        "crates/devports-app/tauri.conf.json": json.loads(
            (ROOT / "crates/devports-app/tauri.conf.json").read_text(encoding="utf-8")
        ).get("version"),
        "ui/package.json": json.loads((ROOT / "ui/package.json").read_text(encoding="utf-8")).get("version"),
    }
    for where, value in found.items():
        if value != version:
            errors.append(f"{where} says {value!r}, the tag says {version!r}")

    changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    if not re.search(rf"^## \[{re.escape(version)}\]", changelog, re.M):
        errors.append(f"CHANGELOG.md has no section '## [{version}]'")

    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if not errors:
        print(f"ok: {version}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
