# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **`devports-probe ls` and `opens`** — Phase 0 read-only listing of the current user's TCP listeners (port, pid, working directory, command line) with a per-day count of how often it was opened, to measure the baseline before the core exists. Removed when `devports ls` talks to the core (T1.10).
- **`devports-probe projects`** — lists the projects found under `~/developer` (or `DEVPORTS_ROOTS`) with their services, and `ls` now shows the project and service each listener belongs to.
- **`devports-probe docker`** — lists the containers of the active Docker context (Docker Desktop, OrbStack or Colima) with their state, published ports and the project they belong to by manifest or compose labels.

### Fixed

- **Command fingerprint** — a service declared as `match: python3 -m http.server N` is now recognised when the kernel reports Homebrew's Python as `Python`: programs are compared by family (lower case, no trailing version) in the program position only.

### Changed

- **Panel memory** — the panel uses a non-persistent web data store, which lets WebKit release one of its two shared helper processes after it closes (about 7 MB less at rest).
