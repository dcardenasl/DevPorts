# ARCHIVES — DevPorts

> Evidencia de las tareas completadas de [`TASKS.md`](TASKS.md). Una entrada por tarea, la más reciente arriba.
> Formato: `ID — título · fecha · commits · verificación (comando y resultado) · hallazgos`.

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
