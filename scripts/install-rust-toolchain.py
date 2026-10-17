#!/usr/bin/env python3
"""Install the Rust version declared by the workspace and toolchain file."""

from __future__ import annotations

import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent

with (ROOT / "Cargo.toml").open("rb") as cargo_file:
    rust_version = tomllib.load(cargo_file)["workspace"]["package"]["rust-version"]

with (ROOT / "rust-toolchain.toml").open("rb") as toolchain_file:
    toolchain = tomllib.load(toolchain_file)["toolchain"]

channel = toolchain["channel"]
channel_parts = channel.split(".")
rust_version_parts = rust_version.split(".")
if len(channel_parts) != 3 or channel_parts[: len(rust_version_parts)] != rust_version_parts:
    raise SystemExit(
        f"rust-toolchain.toml channel ({channel}) must pin a patch release "
        f"on the Cargo.toml rust-version line ({rust_version})."
    )

install_command = [
    "rustup",
    "toolchain",
    "install",
    channel,
    "--profile",
    toolchain["profile"],
]
for component in toolchain["components"]:
    install_command.extend(["--component", component])
subprocess.run(install_command, check=True)

if len(sys.argv) > 1:
    subprocess.run(
        ["rustup", "target", "add", *sys.argv[1:], "--toolchain", channel],
        check=True,
    )
