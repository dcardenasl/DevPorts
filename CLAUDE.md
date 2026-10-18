# CLAUDE.md — DevPorts

Contexto para retomar este proyecto sin la conversación previa. Escrito para que cualquier modelo o sesión nueva continúe sin repetir decisiones ya cerradas.

## Qué es
App de barra de menús para macOS que mantiene el orden de los servidores de desarrollo (qué corre, de qué proyecto, cuál es el oficial y cuáles son pruebas con vencimiento) más un CLI `devports run` para que los agentes de IA no dupliquen servidores. Nombre de trabajo; el definitivo se decide en la Fase 4 (ADR-020).
**Stack:** Rust (núcleo y CLI), Tauri v2 (envoltura), React + TypeScript (panel), SQLite. Solo macOS en v1.
**Estado:** núcleo, CLI y app de barra construidos (fases 1 y 2); esperan las puertas G1/G2 (uso real). Ver `TASKS.md` y `SNAPSHOT.md`.

## Cómo retomar en frío
1. Leer [`TASKS.md`](TASKS.md): primera tarea sin marcar con dependencias cumplidas. Para las correcciones UXR, usar las [fichas de sesión](TASKS.md#uxr-sesiones): contienen fuentes, alcance, aceptación y registro de entrega. Leer en ARCHIVES.md el resultado de sus dependencias; una sesión no necesita recuperar la conversación original.
2. Leer los docs que esa tarea cita. Orden general: `docs/DECISIONES.md` → `PRD.md` → `ARQUITECTURA.md` → `MANIFIESTO.md` → `CLI.md` → `PRUEBAS.md` → `ROADMAP.md`.
3. `git branch --show-current` debe decir `dev`; `git status`; `git log --oneline | head -15` (copiar el estilo real).
4. Cada prueba automatizada del plan lleva `// covers: ID` y `tools/check_matrix.py` falla si falta una (PRUEBAS.md §4).

## Documentos
| Archivo | Cuándo cargarlo |
|---|---|
| `TASKS.md` | Siempre. Cola de trabajo y puertas |
| `docs/DECISIONES.md` | Siempre antes de implementar. **Fuente de verdad** (ADR-001..) |
| `docs/PRD.md` | Validar alcance y criterios (RF/NF) |
| `docs/ARQUITECTURA.md` | Construir núcleo, IPC, datos, UI shell; verificaciones V1–V12 |
| `docs/MAPA-DEL-CODIGO.md` | Orientarse en las carpetas: qué módulo hace qué, en qué orden leerlos, dónde tocar para cada cambio |
| `docs/MANIFIESTO.md` | Parsers, identidad, `adopt` |
| `docs/CLI.md` | Comandos, códigos de salida, mensajes |
| `docs/PRUEBAS.md` | Escribir pruebas; matriz requisito→prueba |
| `docs/ROADMAP.md` | Fases y puertas |
| `docs/UI-REFERENCIA.md` + `docs/ui-referencia/` | Construir el panel (capturas y tokens) |
| `docs/plan/tema-e-idioma.md` | Tema claro/oscuro e i18n (UXR-9.x): diseño y riesgos |
| `ARCHIVES.md` | Evidencia de tareas cerradas |
| `SNAPSHOT.md` / `ASSET.md` | Estado de la última sesión / ficha del asset (actualizar al cerrar sesión) |
| Plan rector | `~/.claude/plans/analiza-la-documentaci-n-referencias-enchanted-hummingbird.md` (razonamiento; `TASKS.md` manda) |

Marcas en los docs: **[C]** confirmado por David (no cambiar sin preguntar); **[D]** derivado; **[P]** propuesta a validar (ajustable con ADR).

## Ramas
`main` solo tiene el commit de creación. **Todo el trabajo va a `dev`.** Remoto y push solo cuando David lo ordene. `main` cambia únicamente por el PR `dev → main` del flujo de release.

## Commits — reglas de `/commit-flow` (siempre, se invoque el skill o no)
`git commit -m "type(scope): subject in english"` — una sola línea, un solo `-m`, inglés, imperativo, minúscula tras los dos puntos, sin punto final. Tipos: `feat fix docs chore refactor test perf style ci build`.
Prohibido: cuerpo o trailers (`Co-Authored-By` incluido), mencionar Claude/AI/Anthropic (ni en PRs), `--no-verify`, `--amend` tras fallo de hook, `git add .`/`-A` (nombrar archivos), subjects vagos.
Nunca sin confirmación de David: `git reset --hard`, `git clean -fd`, `git push --force`, `git checkout -- .`.
Si un hook falla: mostrar el error, arreglar la causa, commit **nuevo**.
Antes de cada commit: `git diff --cached --stat`. Una tarea = 1 a 3 commits ordenados base→superficie (`proto → core → cli → app → ui → tests → docs`).
CHANGELOG (`## [Unreleased]`) solo para `feat/fix/perf`; nunca inventar número de versión. Release solo con `/release` (PR aprobado por David, merge sin squash).

## Reglas técnicas no negociables
1. `devports-core` no depende de `tauri`. `devports-cli` no depende de `devports-core` ni de `tauri` (solo `devports-proto`). Un test inspecciona `cargo metadata`.
2. Cero `unwrap`, `expect`, `panic!`, `todo!`, indexación cruda ni aritmética sin control en producción (lints `deny`). Errores tipados con `Result`.
3. `unsafe_code` es `deny` y solo se relaja en tres archivos (`platform/macos.rs` del núcleo, `window_style.rs` y `notifications.rs` de la app; ADR-025; un test lo vigila). El núcleo no crea procesos: `Command::new` está prohibido salvo en un módulo justificado del CLI (`exec`, `open`), uno de la app (`launchctl`) y las pruebas (`#![allow]` en su cabecera).
4. Un proceso se identifica por `(pid, start_time)`, nunca solo por `pid`.
5. Solo se detiene automáticamente lo registrado con vencimiento (ADR-006). Parada: SIGTERM al proceso y sus descendientes, nunca a un grupo ajeno; prohibiciones absolutas de ARQUITECTURA §8.
6. El socket solo expone lo que el CLI puede hacer: `ping`, `run.register`, `ls`, `stop`/`extend` (con token), `log`, `adopt.propose`. Detener Principales o no registrados, escribir manifiestos y marcar legítimo existen solo en comandos Tauri (ADR-009).
7. Los tokens nunca aparecen en `ls` ni `log`; solo se guarda su hash SHA-256.
8. El núcleo es un actor que posee el estado (canal `mpsc`); sin `Mutex` compartido sobre el dominio. Reloj, `ProcessSource` y `ContainerSource` inyectables.
9. Sin cuestionar al usuario: `devports run` nunca pregunta salvo `DEVPORTS_INTERACTIVE=1` (ADR-008). `--replace` **no existe** en v1 (ADR-023).
10. `ui/src/bindings.ts` es generado desde los tipos de Rust (`specta`, prueba `crates/devports-core/tests/bindings.rs`), nunca a mano; CI falla si hay deriva. Los nombres de los comandos de Tauri se escriben una vez en `ui/src/api.ts`. Cero `any`; `strict` + `noUncheckedIndexedAccess` + `exactOptionalPropertyTypes`. El webview se crea al abrir el panel y se destruye al cerrarlo.
11. Sin `TODO` sueltos: lo pendiente es una tarea en `TASKS.md`.

## Fuera de la v1 (ADR-015)
Servidor MCP, adaptadores por agente, proxy de nombres estables, versión Pro/equipos, gestor tipo pm2, borrar contenedores/volúmenes, Linux, Windows. Principio de David: simple pero robusto; nada que hoy no se necesite.

## Idioma
Código, comentarios, commits y CHANGELOG en **inglés**. `docs/`, `TASKS.md`, `CLAUDE.md` y la salida del CLI en **español**; el panel, la bandeja y las notificaciones siguen el idioma elegido (es/en, ADR-041). Indentación: 4 espacios Rust, 2 JS/TS. Nombres: `PascalCase` tipos/componentes, `camelCase`/`snake_case` según el lenguaje, `kebab-case` archivos web.

## Puertas humanas
G0 (criterio de éxito ADR-022), G1 (+ revisión de portafolio), G2, G3, G4. Al llegar a una: detenerse y pedir la decisión a David. La Fase 3 se hace en el repo `ci4-kickstart`, en sesión aparte.

## Verificación
Local: `tools/check.sh full` (fmt, matriz de pruebas, clippy `-D warnings`, todas las pruebas con `devports-core/bindings`, `cargo deny`) y en `ui/`: `pnpm run typecheck && pnpm run lint && pnpm run format:check && pnpm test`. En la Mac, antes de cada versión: `tools/docker-checks.sh` y `tools/launchd-check.sh`; para el presupuesto, `tools/measure.sh`. Instalar: `tools/install.sh`.
CI: `dev-check` (ubuntu, ligero) en push a `dev`; `quality` (ubuntu + macOS, integración con procesos reales) en PR a `main`. Docker real solo en la Mac de David con `tools/docker-checks.sh`.

## Skills opcionales
`/code-review` y `/security-review` antes de G1 y del PR de release (T1.8 siempre con revisión manual). `/auditoria-procesos` solo si David lo pide.
