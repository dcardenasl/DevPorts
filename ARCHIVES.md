# ARCHIVES — DevPorts

> Evidencia de las tareas completadas de [`TASKS.md`](TASKS.md). Una entrada por tarea, la más reciente arriba.
> Formato: `ID — título · fecha · commits · verificación (comando y resultado) · hallazgos`.

## T0.2 — `ProcessSource` de macOS y sonda `ls` de solo lectura · 2026-10-17
- Commits: `build: relax unsafe_code to deny and add libproc, sysinfo and jiff`, `feat(core): add process source trait with fake and macos libproc adapter`, `test(core): verify the macos process source against real listeners`, `test(core): confine unsafe code to the macos adapter`, `feat(core): add phase 0 probe that lists listeners and counts opens`.
- Verificación local: `cargo fmt --all --check` ok · `cargo clippy --workspace --all-targets -- -D warnings` sin avisos · `cargo test --workspace` en verde, incluidas 5 pruebas con procesos reales (servidor, varios puertos, servidor detenido, cliente que solo conecta, `process_info`) y 5 de arquitectura. Prueba negativa: un `unsafe { }` en el CLI hace fallar `unsafe_is_confined_to_the_macos_adapter`.
- **V1 favorable:** `libproc` lista los procesos del usuario y sus sockets TCP en `LISTEN` sin privilegios. **V2 favorable:** con `php -S 127.0.0.1:18081`, `node -e ...listen(18082)` y `python3 -m http.server 18083` lanzados desde carpetas distintas, la sonda mostró puerto, pid, carpeta y comando correctos de los tres. **V3 favorable:** con 24 procesos escuchando, un escaneo completo cuesta unos 4 a 5 ms de CPU (release, 40 ejecuciones en 0,34 s de pared incluyendo arranque del proceso) y 7 MB de memoria máxima; el presupuesto es 15 ms por ciclo de 1,5 s (1 %). Plan B `lsof` no hace falta.
- Pendiente de confirmar en G0: medición a lo largo del día con el núcleo residente (T0.5/T2.7); aquí solo se midió el escaneo aislado.
- Hallazgos: (1) `libproc::pidcwd` **no está implementado en macOS** (devuelve error siempre) y `libproc` no lee `argv`; ambos salen de `sysinfo`. (2) Leer el protocolo de un socket exige `unsafe` (unión de C), así que `unsafe_code` pasó de `forbid` a `deny` con una sola excepción vigilada por test (ADR-025, punto 3). (3) El CLI no puede depender del núcleo, así que el `ls` sin núcleo es el binario temporal `devports-probe`, que se retira en T1.10. (4) La Mac de David tiene un proceso de `com.docker.docker` (Docker Desktop) escuchando en 3306, 8888 y otros: pista de que usa Docker Desktop; se confirma en T0.4. (5) Un proceso lanzado desde un subshell que termina queda con `ppid` 1, el caso que el vínculo medio de ARQUITECTURA §7 debe cubrir.

## T0.1 — Workspace de Cargo, lints, toolchain y CI mínimo · 2026-10-17
- Commits: `build: add cargo workspace with proto, core, cli and app crates`, `test: enforce workspace dependency rules and cli smoke tests`, `ci: add dev-check and quality workflows`, `docs(adr): record adr-025 core concurrency and platform boundaries`.
- Verificación local: `cargo fmt --all --check` ok · `cargo clippy --workspace --all-targets -- -D warnings` sin avisos · `cargo test --workspace`: 3 pruebas de arquitectura y 2 del CLI en verde · `cargo deny check`: advisories, bans, licenses y sources ok · `cargo audit`: sin vulnerabilidades en 15 crates.
- Prueba negativa: añadir `devports-core` a las dependencias del CLI hace fallar `cli_depends_on_proto_only` con "devports-cli must not depend on devports-core"; se revirtió.
- CI: los dos workflows son YAML válido (ruby); **no se han ejecutado en GitHub** porque no hay remoto. Primera ejecución real: al crear el remoto.
- Hallazgos: `clippy::single_match_else` obligó a `if let` en el CLI. Los *helpers* de pruebas de integración no los cubren los `allow-*-in-tests` de `clippy.toml`, así que llevan un `#![allow]` explícito en su cabecera. `cargo deny` avisa de licencias permitidas aún sin uso (esperable hasta que entren dependencias). El módulo `process_spawn` no se crea todavía: llega con su primer uso (T0.8/T1.7). `crates/devports-app` es un binario vacío hasta el spike T0.5.

## B-4 — Hooks de git versionados · 2026-10-17
- Commit: `chore(hooks): add commit-msg and pre-commit hooks with installer` (el propio commit pasó por `commit-msg`).
- Verificación: `commit-msg` probado con 7 mensajes: acepta el válido y uno que menciona `--replace`; rechaza mayúscula tras los dos puntos, punto final, cuerpo con trailer, mención a Claude y tipo inválido. `pre-push` rechaza `refs/heads/main`.
- Hallazgo: `pre-commit` activa `cargo fmt` y las comprobaciones de `ui/` solo cuando existen `Cargo.toml` y `ui/node_modules`, así que es válido desde el primer commit.

## B-3 — Documentos de continuidad · 2026-10-17
- Commit: `docs: add agent context, task queue and changelog`.
- Verificación: `CLAUDE.md` + `TASKS.md` bastan para arrancar en frío; `CHANGELOG.md` con `[Unreleased]` vacío.

## B-2 — Saneamiento de docs y ADR-023/024/025 · 2026-10-17
- Commits: `docs(decisions): record adr-023 and adr-024 and drop --replace from v1`, `docs: align cli, architecture, tests and roadmap with adr-023`, `docs(ui): add panel and context reference screenshots`, `chore(tools): add baseline sampler for duplicate server measurement`, `docs: update snapshot and asset sheet after adr-023 and adr-024`.
- Verificación: `grep -rniE "replace|approvals" docs/ SNAPSHOT.md ASSET.md` solo encuentra ADR-023, las notas de retiro y la fuente original de VentureOS.
- Hallazgos: el ID 15 de código de salida, U-12 e I-12 quedan retirados y **no se reutilizan**. ADR-025 queda reservado para T0.1.

## B-1 — Repositorio, ramas y `.gitignore` · 2026-10-17
- Commits: `chore: initialize repository` (en `main`, vacío), `chore(gitignore): ignore build output, brag renders and local settings` (en `dev`).
- Verificación: `git branch -a` muestra `main` (1 commit) y `dev`; `brag-output/` no aparece en `git status`.
