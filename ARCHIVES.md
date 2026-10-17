# ARCHIVES — DevPorts

> Evidencia de las tareas completadas de [`TASKS.md`](TASKS.md). Una entrada por tarea, la más reciente arriba.
> Formato: `ID — título · fecha · commits · verificación (comando y resultado) · hallazgos`.

## B-2 — Saneamiento de docs y ADR-023/024/025 · 2026-10-17
- Commits: `docs(decisions): record adr-023 and adr-024 and drop --replace from v1`, `docs: align cli, architecture, tests and roadmap with adr-023`, `docs(ui): add panel and context reference screenshots`, `chore(tools): add baseline sampler for duplicate server measurement`, `docs: update snapshot and asset sheet after adr-023 and adr-024`.
- Verificación: `grep -rniE "replace|approvals" docs/ SNAPSHOT.md ASSET.md` solo encuentra ADR-023, las notas de retiro y la fuente original de VentureOS.
- Hallazgos: el ID 15 de código de salida, U-12 e I-12 quedan retirados y **no se reutilizan**. ADR-025 queda reservado para T0.1.

## B-1 — Repositorio, ramas y `.gitignore` · 2026-10-17
- Commits: `chore: initialize repository` (en `main`, vacío), `chore(gitignore): ignore build output, brag renders and local settings` (en `dev`).
- Verificación: `git branch -a` muestra `main` (1 commit) y `dev`; `brag-output/` no aparece en `git status`.
