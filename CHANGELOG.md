# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Light theme** — the panel now has a light and a dark theme, both meeting WCAG AA; by default it follows the system appearance, live, and opens already wearing the right one.

- **Projects tab layout** — each project is one header row (name, folder, branch and its occupied ports, still visible when folded), several projects can stay open at once with Plegar todo / Abrir todo, active projects open by themselves and are listed first, and a service row opens its own details.

- **Connection recovery** — if the panel loses DevPorts it keeps showing the last data (with its age), retries on its own a bounded number of times and offers Reintentar; Docker being unreachable and manifest or configuration warnings are now explained on every tab, with the details in Configuración.

- **Scan folders in the panel** — a Configuración gear lets you add or remove the folders projects are searched in, with each folder checked, the projects that start or stop being listed shown before saving, and discovery applied at once; removing a folder deletes no file and stops no process.

- **Auxiliary tools group** — loose processes are listed as development servers, unclassified, or «Posibles herramientas auxiliares» (folded, with their origin such as Homebrew or VS Code); the guess is advice only, can be corrected per exact folder and command, and never ignores, adopts or stops anything.

- **Ignored services** — «Ignorar» is now its own action that shows the exact rule (folder and command, or container and Docker context) before saving, and «Ver ignorados» lists every saved rule, including ones from earlier versions, with a one-click restore that starts or stops nothing.

- **Shared resources panel** — manage which projects use a shared service from its card or from a project, with a file-by-file review, per-project results and retry of pending ones; a stopped resource offers its declared start command, and a browser link only when `shared.yml` declares an HTTP endpoint.

- **Shared-service consumers** — the core can list which projects declare a shared service and save the association in several manifests at once: every manifest is validated before any write, each file reports its own result, and repeating a save never declares a service twice.

- **Reviewed adoption** — adopting a process or container now goes destination → editable review → save in the panel, with recoverable drafts, per-field errors, typed conflicts and results that survive closing the panel; destinations the discovery cannot reach are refused with the next step.

- **Adoption reviews** — the core now exposes eligible destinations, frozen previews, editable-field validation and idempotent saves with typed conflicts for the panel review workflow.

- **Instance diagnostics** — all instance views now expose expandable origin, identity, observed ports, working folder, launcher chain and explained Git age hints, with compact titles and masked command argument values.

- **Project stop** — preview and confirm an exact list of project applications and Temporals while preserving data services, shared resources and later arrivals; each target is revalidated independently.

- **Manual stop confirmation** — every stop route now shows the target identity, port and origin; shared consumers and data-container effects are explicit. Stop requests revalidate the confirmed process/container identity and port before dispatch, so a stale view cannot stop a replacement.
- **Panel operations** — confirmed stop, extend, keep and adoption actions now run in the app process, with idempotent requests, per-target progress/results, bounded in-memory recovery and explicit retries.
- **DevPorts app** — menu-bar app that hosts the core: a panel with Atención, Proyectos, Compartidos, Sin proyecto and Actividad, system notifications (lease about to end with an "Extender" button, grouped duplicates), a counter in the menu bar, adopting running processes and database containers into manifests, and start at login.
- **`devports` command** — `run` (one official server per service, test instances with a lease, `--reuse`, fail-open), `ls`, `stop`, `extend`, `log` and `adopt`, with stable exit codes and `--json`.
- **Safe stopping** — only registered test instances with a lease are stopped automatically, by `(pid, start)` identity, never by process group, never `docker rm`.

### Fixed

- **Panel opening** — the panel now appears as soon as its first screen has painted, showing a progress bar and «Cargando la información de proyectos y puertos…» while the data arrives, instead of staying invisible until the first snapshot.

- **Stop dialog and Activity** — stopping an unregistered process now names the program instead of a short id, the Clasificar sheet shows its choices first, and reclassified or restored events read in Spanish in Actividad.

- **Panel accessibility** — the five sections are real tabs (arrow keys, Home and End, linked to their panel), field errors are tied to their field, sheet actions stay reachable on long content, long paths wrap, smooth scrolling respects reduced motion, and colour contrast is checked by a test.
- **Activity log** — an unreadable activity store is now an error (in the panel and in `devports log`) instead of looking like «no activity»; the panel names its 24-hour window, counts only the events it shows, filters on typed topics and keeps the last good read if a refresh fails.
- **Ignore rules** — new rules keep the command as a list of arguments and scope containers to their Docker context, so two different commands can no longer collide and nothing wider than the rule you saw is ever hidden.
- **Manifest editing** — optimistic file versions, unique temporary files and no-clobber creation protect detectable external edits, preserve existing permission modes and keep comments and line endings intact.

- **Instance state** — the panel and CLI distinguish stopped, unhealthy and unknown instances; registered containers become unknown when Docker observation is lost while retaining their identity and duplicate-Principal protection.

- **Stop results** — manual operations now wait for actual termination and retain per-target failures instead of reporting acceptance as successful completion.

- **Panel dialogs** — adoption and stop confirmations now keep keyboard focus inside, cancel with Escape, restore focus to their opener, and let adoption steps go back without losing the project name.

- **Command fingerprint** — a service declared as `match: python3 -m http.server N` is now recognised when the kernel reports Homebrew's Python as `Python`: programs are compared by family (lower case, no trailing version) in the program position only.

### Changed

- **Panel context** — reopening the panel restores its tab, expanded project, activity filter and resumable adoption draft while the app stays running; stale drafts are discarded without taking action.
- **Panel memory** — the panel uses a non-persistent web data store, which lets WebKit release one of its two shared helper processes after it closes (about 7 MB less at rest).
