# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **DevPorts app** — menu-bar app that hosts the core: a panel with Atención, Proyectos, Compartidos, Sin proyecto and Actividad, system notifications (lease about to end with an "Extender" button, grouped duplicates), a counter in the menu bar, adopting running processes and database containers into manifests, and start at login.
- **`devports` command** — `run` (one official server per service, test instances with a lease, `--reuse`, fail-open), `ls`, `stop`, `extend`, `log` and `adopt`, with stable exit codes and `--json`.
- **Safe stopping** — only registered test instances with a lease are stopped automatically, by `(pid, start)` identity, never by process group, never `docker rm`.

### Fixed

- **Command fingerprint** — a service declared as `match: python3 -m http.server N` is now recognised when the kernel reports Homebrew's Python as `Python`: programs are compared by family (lower case, no trailing version) in the program position only.

### Changed

- **Panel memory** — the panel uses a non-persistent web data store, which lets WebKit release one of its two shared helper processes after it closes (about 7 MB less at rest).
