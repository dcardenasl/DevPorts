# TASKS — DevPorts

> **Fuente de verdad de ejecución.** Contexto y reglas para sesiones nuevas: [`CLAUDE.md`](CLAUDE.md).
> Evidencia de lo completado: [`ARCHIVES.md`](ARCHIVES.md). Especificación: [`docs/`](docs/) (orden en `docs/LEEME.md`).
> Ante cualquier contradicción manda [`docs/DECISIONES.md`](docs/DECISIONES.md).

**Estado:** v0.0. Fase B completa y T0.1 hecha: workspace de 4 crates, lints, CI y reglas de dependencia verificadas. Sin lógica de dominio todavía.
**Rama de trabajo:** `dev` (nunca `main`). Sin remoto hasta que David lo ordene.

## Cómo retomar en frío (cualquier modelo o sesión)
1. Lee `CLAUDE.md` y este archivo. Toma la **primera tarea sin marcar cuyas dependencias estén completas**.
2. Lee los docs que cita la tarea (columna *Lee*). No reabras decisiones ya cerradas (`docs/DECISIONES.md`). Las marcas **[C]** no se cambian sin preguntar a David; **[D]/[P]** se pueden ajustar si lo dejas escrito en un ADR.
3. `git branch --show-current` → debe decir `dev`. `git status` y `git log --oneline | head -15`.
4. Trabaja la tarea completa (código + pruebas + docs), verifica, commitea con `/commit-flow`.
5. Marca `[x]`, mueve la evidencia a `ARCHIVES.md`, actualiza `SNAPSHOT.md`. Si algo se descubre, añade tarea o nota aquí (nunca `TODO` suelto en el código).
6. **Puertas (G0..G4) son humanas**: al llegar a una, detente y pide a David el criterio; no avances.

## Definition of Done (toda tarea)
- Criterios «Hecho cuando» cumplidos y pruebas citadas en verde.
- `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace` (desde que exista el workspace); desde T2.1 además `npm run typecheck && npm run lint && npm run format:check && npm test`.
- Cero `unwrap/expect/panic!/indexing` crudo en producción; `unsafe` solo en `platform::macos` (ADR-025); el núcleo no crea procesos.
- Docs afectados actualizados en el mismo commit o en uno `docs(...)` contiguo. CHANGELOG si es `feat/fix/perf`.
- Commits según `/commit-flow`: una línea, inglés, `type(scope): subject`, sin trailers ni menciones a Claude/AI. Archivos nombrados uno a uno; `git diff --cached --stat` antes de cada commit.

## Siguiente paso
**La Fase 0 está construida y documentada** (`docs/verificacion-fase0.md`). Falta solo lo humano: **S0** (línea base, aún sin iniciar), una semana de uso de `devports-probe`, las comprobaciones manuales de la sección 3.3 del informe y la **puerta G0**. No empezar T1.1 sin la decisión escrita de David. S0 sigue pendiente de David; además David instala la sonda (ver S0). En paralelo y por David: **S0** (`./tools/baseline.sh once`, luego `start`).

---

## Fase B — Cimientos del repositorio

- [x] **B-1 — Repositorio, ramas y `.gitignore`.** *Deps:* ninguna. *Lee:* `~/Developer/AGENTS.md`.
  `git init -b main`; `.gitignore` (`target/`, `node_modules/`, `.DS_Store`, `brag-output/`, `.claude/settings.local.json`, `*.db`, `dist/`); commit único en `main` y `git checkout -b dev`. No hay remoto.
  *Hecho cuando:* `git branch -a` muestra `main` (1 commit) y `dev`; `brag-output/` no aparece en `git status`.
  → `chore: initialize repository` (en `main`, única excepción) · `chore(gitignore): ignore build output, brag renders and local settings` (en `dev`)

- [x] **B-2 — Saneamiento de docs y ADR-023/024/025.** *Deps:* B-1. *Lee:* `docs/DECISIONES.md`, `CLI.md`, `ARQUITECTURA.md`, `PRUEBAS.md`, `ROADMAP.md`, `PRD.md`, `UI-REFERENCIA.md`, `SNAPSHOT.md`, `ASSET.md`.
  Registrar **ADR-023** (`--replace` eliminado de v1), **ADR-024** (Principal de hecho + raíz `~/developer` confirmadas) y dejar **ADR-025** como reservado para las decisiones de T0.1. Eliminar toda referencia a `--replace`/código 15/`approvals`/`replace_*`/U-12/I-12 en los 7 documentos; cerrar puntos abiertos 1, 3, 4 en DECISIONES; corregir la ruta `assets/devports/tools` en SNAPSHOT; renumeración de T1.11 → «Salud». Añadir a `docs/ARQUITECTURA.md` §10 las cotas del IPC (1 MiB, timeouts) y la nota de longitud del socket.
  *Hecho cuando:* `grep -rniE "replace|approvals|codigo 15|código 15" docs/` solo encuentra la sección de «Decisiones reemplazadas»/ADR-023.
  → `docs(decisions): record adr-023 and adr-024 and drop --replace from v1` · `docs: align cli, architecture, tests and roadmap with adr-023`

- [x] **B-3 — Documentos de continuidad.** *Deps:* B-1. 
  Crear `CLAUDE.md` (contenido: Apéndice B), `TASKS.md` (este archivo), `ARCHIVES.md` (vacío con plantilla de evidencia), `CHANGELOG.md` (`## [Unreleased]`), `.editorconfig` (4 espacios Rust, 2 JS/TS, LF), `README.md` mínimo.
  *Hecho cuando:* una sesión nueva puede arrancar leyendo solo `CLAUDE.md` + `TASKS.md`.
  → `docs: add agent context, task queue and changelog`

- [x] **B-4 — Hooks de git versionados.** *Deps:* B-1, B-3 (para validar los textos). *Referencia:* `~/Developer/Tauri/GitCanvas/{pre-commit,commit-msg,pre-push,scripts/install-git-hooks.sh}`.
  Adaptar: `commit-msg` (formato Conventional Commits, tipos permitidos, una línea, sin trailers ni menciones a Claude/AI/Anthropic); `pre-commit` (`cargo fmt --check` si existe workspace; eslint/prettier solo sobre staged cuando exista `ui/`); `pre-push` ligero; `scripts/install-git-hooks.sh`.
  *Hecho cuando:* un mensaje con `Co-Authored-By` o mayúscula tras los dos puntos es rechazado; uno válido pasa.
  → `chore(hooks): add commit-msg and pre-commit hooks with installer`

## Semana 0 — Línea base (HUMANO, en paralelo desde hoy)

- [ ] **S0 — Medir la línea base.** David ejecuta `./tools/baseline.sh start` (script **no probado en macOS**: primero `./tools/baseline.sh once` y revisar `~/.devports-baseline/samples.tsv`; bash 3.2). Trabajar normal ≥5 días laborales; `report` → `docs/linea-base-AAAA-MM-DD.txt`.
  *Sonda de la Fase 0 (T0.2):* `cargo install --path crates/devports-core --bin devports-probe --locked` instala `devports-probe`. `devports-probe ls` lista los servidores que escuchan (puerto, pid, carpeta, comando) y registra cada apertura en `~/.devports-baseline/opens.log`; `devports-probe opens` cuenta aperturas por día (criterio G0: ≥3 por día laborable).
  *Hecho cuando:* hay ≥5 días registrados. *Si `once` falla en macOS:* corregir el script como tarea `fix(tools):` antes de seguir.

---

## Fase 0 — Sondeo en la Mac (puerta G0)

- [x] **T0.1 — Workspace de Cargo, lints, toolchain y CI mínimo.** *Deps:* B-4. *Lee:* ARQUITECTURA §2, §3; GitCanvas `Cargo.toml`, `clippy.toml`, `rust-toolchain.toml`, `.github/workflows/*`.
  Crear `Cargo.toml` (workspace, `resolver = "2"`, `[workspace.lints]` igual que GitCanvas, `publish = false`, sin licencia hasta ADR-020), `rust-toolchain.toml` (1.98.1), `clippy.toml` (`disallowed-methods` con excepción documentada para `process_spawn`), `deny.toml`, los 4 crates mínimos (`proto`, `core`, `cli`, `app` placeholder compilable), `.github/workflows/{dev-check,quality}.yml` (ubuntu en push a `dev`; ubuntu+macOS en PR a `main`; **sin Windows**). Escribir **ADR-025**: concurrencia por actor, política de `unsafe`/`Command`, `rusqlite`, `bollard`, texto centralizado.
  *Reglas verificables:* `devports-core` no depende de `tauri`; `devports-cli` no depende de `core` ni `tauri` (test que inspecciona `cargo metadata`).
  *Hecho cuando:* `cargo build/clippy/test --workspace` verdes local y en CI; el test de dependencias falla si se rompe la regla.
  → `build: add cargo workspace with proto, core, cli and app crates` · `ci: add dev-check and quality workflows` · `docs(adr): record adr-025 core concurrency and platform boundaries`

- [x] **T0.2 — `ProcessSource` de macOS y `devports ls` de solo lectura.** *Deps:* T0.1. *Lee:* ARQUITECTURA §3-4, CLI §4. *Verifica:* V1, V2, V3.
  Trait `ProcessSource` + `platform::macos` (libproc: PIDs del uid, FDs, sockets TCP LISTEN, cwd, argv vía `KERN_PROCARGS2`, `(pid,start_time,ppid,pgid,uid)`). `devports ls` sin núcleo que escanea localmente y **registra cada invocación** en `~/.devports-baseline/opens.log`. Medir costo (CPU/ms) del escaneo; si supera presupuesto, implementar Plan B `lsof` detrás del mismo trait.
  *Hecho cuando:* `ls` lista `php -S`, `node`, `python -m http.server` lanzados por la prueba con cwd/argv correctos sin privilegios; medición registrada. *Pruebas:* fuente falsa (U), integración real macOS (I-02 parcial).
  → `feat(core): add macos process source on libproc` · `feat(cli): add read-only ls with baseline open log`

- [x] **T0.3 — Descubrimiento de manifiestos y mapeo proceso→proyecto.** *Deps:* T0.2. *Lee:* MANIFIESTO §3, §7. *Verifica:* V11.
  Recorrido de raíces (3 niveles, ignora `node_modules/vendor/.git/target`), carga mínima de `.devports.yml`, normalización (MANIFIESTO §3) y matching. *Pruebas:* U-01, U-02, PT-01, PT-02, I-14.
  → `feat(core): discover projects and match processes to services`

- [x] **T0.4 — Docker: contexto, lista, eventos y etiquetas compose.** *Deps:* T0.1. *Lee:* ARQUITECTURA §4, ADR-012. *Verifica:* V4, V12.
  Trait `ContainerSource` + implementación `bollard`; `docker context inspect` para el endpoint; asignación por etiquetas. **Registrar qué runtime usa David y si usa compose** (cierra punto abierto 2). *Pruebas:* D-01, D-03 (cliente simulado en CI; real en la Mac).
  → `feat(core): add docker container source with compose label mapping`

- [x] **T0.5 — Spike Tauri.** *Deps:* T0.1. *Lee:* ARQUITECTURA §12, §14. *Verifica:* V5, V10.
  Ícono de plantilla con contador, panel 380×520 anclado, política *accessory*, webview creado/destruido por apertura; confirmar que `crates/devports-app` funciona con la CLI de Tauri (si exige `src-tauri/`, decidir y anotar en ADR-025). Medir RSS/CPU en reposo.
  *Hecho cuando:* tabla de mediciones en el informe; ¿cumple <50 MB y <1 % CPU?
  → `feat(app): add tray spike with counter and ephemeral panel`

- [x] **T0.6 — Spike de notificaciones con botón de acción.** *Deps:* T0.5. *Verifica:* V6. Resultado: botones nativos sí/no ⇒ define el diseño de la notificación de vencimiento.
  → `feat(app): add notification action spike`

- [x] **T0.7 — Spike de supervisión y autoarranque.** *Deps:* T0.5. *Verifica:* V7, V8.
  launchd `KeepAlive{SuccessfulExit:false}`; «Salir» no reinicia; latencia de `open -g -a` desde el CLI.
  → `feat(app): add launch agent supervision spike`

- [x] **T0.8 — Spike de `exec`, PID y ascendencia.** *Deps:* T0.2. *Verifica:* V9.
  Confirmar que `sh -c 'exec …'` conserva PID y que `launcher_chain` permite el vínculo fuerte.
  → `test(core): verify exec keeps pid and ancestry`

- [x] **T0.9 — Informe `docs/verificacion-fase0.md` (V1–V12).** *Deps:* T0.2–T0.8. Incluye la **comprobación manual de David sobre el spike de Tauri** (T0.5), que el entorno de desarrollo no pudo hacer: (1) `cargo run -p devports-app` y clic izquierdo en el ícono: el panel aparece **bajo el ícono**; (2) clic fuera del panel: se cierra; (3) clic en el ícono con el panel abierto: se cierra y **no** se reabre; (4) clic derecho: menú «Abrir panel» / «Salir»; (5) «Salir» termina el proceso; (6) **notificación (T0.6):** `cargo run -p devports-app --features spike-notify`, aceptar el aviso de permiso de notificaciones que macOS muestra la primera vez, esperar la notificación «api vence en 1 minuto» y comprobar que trae el botón «Extender 30 min»: pulsarlo escribe `notify: user performed extend` en la terminal y pulsar el cuerpo escribe `tap`. Anotar si el botón aparece directo o dentro de «Opciones». Tabla con resultado y decisión por cada V; actualizar ARQUITECTURA si algún **[P]** cambia (con ADR).
  → `docs: add phase 0 verification report`

- [ ] **G0 — PUERTA FASE 0 (HUMANA, ADR-022).** (a) V1, V2, V3, V9 favorables o Plan B aceptado; (b) tras una semana usando `devports ls`: duplicados simultáneos ≤ ½ de la línea base **y** ≥3 aperturas/día laborable (`opens.log`); (c) presupuesto de reposo medido. **Si (b) falla: parar y replantear. No continuar sin decisión escrita de David.**

---

## Fase 1 — Núcleo y CLI sin interfaz (puerta G1)

- [ ] **T1.1 — `devports-proto`, servidor IPC, cliente y verificación de uid.** *Deps:* G0. *Lee:* ARQUITECTURA §10, CLI §8, NF-04.
  Tipos serde versionados (`v:1`); socket 0600 en dir 0700; `LOCAL_PEERCRED`; instancia única (socket vivo ⇒ salir, huérfano ⇒ borrar); cotas (línea ≤1 MiB, timeouts, máx. conexiones); ruta ≤104 bytes con respaldo; `examples/headless.rs`. Operaciones: `ping` primero.
  *Pruebas:* I-11, longitud de ruta, línea excesiva, JSON inválido, versión desconocida.
  → `feat(proto): add ipc protocol types` · `feat(core): add unix socket server with peer credential check` · `feat(cli): add ping client`

- [ ] **T1.2 — Parsers y validación de `.devports.yml`, `shared.yml`, `config.yml`.** *Deps:* T1.1. *Lee:* MANIFIESTO §1,2,6,9.
  Errores con número de línea, advertencias (clave `test` ⇒ mensaje ADR-010), valores por defecto de `config.yml`. T0.3 ya dejó `manifest::parse/load` (YAML con `serde-saphyr`, versión 1) y los tipos `Manifest`/`ServiceSpec`: T1.2 añade el validador encima (formatos, unicidad, uno de `cwd` o `docker`, advertencias, números de línea con `Spanned`, y **`cwd` que salga de la carpeta del proyecto = error**) y `shared.yml`/`config.yml`. **Decidir y fijar el método de escritura que preserva comentarios** (editor de YAML con CST) validado contra I-15.
  *Pruebas:* U-03 completa (cada error/advertencia), versión mayor rechazada.
  → `feat(core): parse and validate manifest, shared and config files`

- [ ] **T1.3 — Descubrimiento con observador de archivos.** *Deps:* T1.2, T0.3. RF-01. Recarga sin reiniciar, respaldo de raíz git (ADR-001), `import_ops_registry`. *Pruebas:* I-14.
  → `feat(core): watch roots and reload manifests on change`

- [ ] **T1.4 — SQLite: migraciones, instancias, eventos, retención.** *Deps:* T1.1. *Lee:* ARQUITECTURA §9 (sin `approvals`, ADR-023). WAL, un escritor, `schema_meta`, retención 30 días, tabla de «ignorados» **no** (se define en T2.0). 
  *Pruebas:* migración desde vacío, `CHECK` de estados, retención, reapertura tras corte.
  → `feat(core): add sqlite store with migrations and event retention`

- [ ] **T1.5 — Observación, reconciliación, identidad y Principal de hecho.** *Deps:* T1.3, T1.4, T0.4. RF-02, RF-04, RF-21; ARQUITECTURA §4-5.
  Actor del núcleo, diferencias aparece/desaparece/cambia puerto, `(pid,start_time)`. *Pruebas:* U-05, PT-02, PT-03.
  → `feat(core): reconcile observed processes into identified instances`

- [ ] **T1.6 — Registro, vínculo por puerto reservado y máquina de estados.** *Deps:* T1.5. RF-06; ARQUITECTURA §6-7.
  Reservas, vínculo contenedor/fuerte/medio/ninguno (la ascendencia ya existe: `process::ancestry`, T0.8), `released`, `port_stolen`. *Pruebas:* U-10, I-04, I-05, I-13.
  → `feat(core): add port reservation and instance binding rules`

- [ ] **T1.7 — Política completa de `run`.** *Deps:* T1.6. RF-07, RF-17; CLI §3.
  Tabla de decisión (códigos 0,10,11,12,13,14,16,18,19; el 15 queda **sin asignar**), mensajes texto y `--json` con *golden files*, `--reuse`, modo interactivo `DEVPORTS_INTERACTIVE=1`, falla abierto + `open -g -b com.davidcardenas.devports` (módulo del CLI con `#[allow]` justificado para `Command`; el mismo módulo hace el `exec`), sustitución `{PORT}`/`${PORT}`, `sh -c 'exec …'`, variables de entorno. 
  *Pruebas:* U-04, U-11, I-01 (caos 7×), I-09, I-13.
  → `feat(core): decide run requests with deterministic conflict policy` · `feat(cli): add run with exec, reuse and fail-open` · `test: add golden files for run output`

- [ ] **T1.8 — Temporales: puertos, tokens, tope, vencimientos persistentes, parada segura.** *Deps:* T1.7. RF-08, RF-09, NF-06; ARQUITECTURA §8.
  Asignador de puertos, token 128 bits (solo hash), tope 2, TTL/extend ≤4 h, aviso T-60 una vez, parada (SIGTERM→5 s→SIGKILL→2 s; nunca grupos ajenos; prohibiciones absolutas), `expired_alive`, ejecución inmediata de vencidos al arrancar. **Bloqueante de seguridad.**
  *Pruebas:* U-06, U-07, U-08, U-09, PT-03, PT-04, I-03, I-06, I-07, I-10.
  → `feat(core): add temporary instances with persistent leases` · `feat(core): add safe process tree termination`

- [ ] **T1.9 — Docker en el núcleo: eventos, vínculo por contenedor, `stop`.** *Deps:* T1.8, T0.4. RF-03.
  Reintento con espera creciente («desconocido», no «caído»); **suscribirse a eventos antes de listar y tratar los eventos como pistas** (T0.4: pueden repetirse, ver ADR-025); añadir `stop` al trait `ContainerSource` (hoy solo `list` y `events`); `tools/docker-checks.sh` ejecuta `cargo test -p devports-core --test docker_engine -- --ignored` además de D-02/D-04; `docker stop` 10 s; nunca `rm`; `kind: data` jamás se detiene solo. Escribir `tools/docker-checks.sh` (D-01..D-05 en la Mac). *Pruebas:* D-01..D-04 simuladas.
  → `feat(core): bind and stop docker containers for temporary instances` · `test(tools): add docker checks script`

- [ ] **T1.10 — `ls`, `stop`, `extend`, `log`, `adopt` (propuesta).** *Deps:* T1.8. RF-05, RF-11, RF-15; CLI §4-7.
  Los tokens nunca salen en `ls`/`log`. `adopt` solo lectura. `adopt` debe generar `match` sin depender de `argv[0]` (ver MANIFIESTO §3, precisiones). **Retirar `devports-probe` (módulo `probe` y binario) y su entrada de CHANGELOG** (ADR-025, punto 8). *Pruebas:* I-08, I-15 (propuesta), U-08.
  → `feat(cli): add ls, stop, extend, log and adopt commands`

- [ ] **T1.11 — Salud (health checks) y estado «colgada».** *Deps:* T1.6. *Lee:* MANIFIESTO §5. (Sustituye al `--replace` eliminado.)
  TCP/HTTP <1 s, cada 5 s, 3 fallos ⇒ no sana; alimenta código 11 y `healthy` en `ls`. *Pruebas:* servidor que deja de responder; HTTP 200–399; reloj falso.
  → `feat(core): add tcp and http health checks with hung state`

- [ ] **T1.12 — Duplicados sin registrar, Olvidados, atribución e instancia vieja.** *Deps:* T1.5, T1.8. RF-10, RF-19, RF-20; ADR-004.
  Detección ≤3 s, `expired_alive` en Olvidados, cadena de ancestros con `known_agents` (lo desconocido, crudo; ninguna acción depende), rama/worktree leyendo `.git/HEAD` y `logs/HEAD` sin lanzar `git`. *Pruebas:* I-02, I-16, I-17.
  → `feat(core): detect unregistered duplicates and forgotten instances` · `feat(core): add launcher attribution and stale instance signals`

- [ ] **T1.13 — Suite completa y CI macOS.** *Deps:* T1.1–T1.12. Cierre de PRUEBAS.md: U-01..U-11, PT-01..PT-04, I-01..I-17, D simuladas; matriz requisito→prueba verificada (script que falla si un RF no tiene prueba); `quality.yml` en macOS.
  → `test: complete unit, property and integration suites` · `ci(quality): run macos integration suite`

- [ ] **G1 — PUERTA FASE 1 (HUMANA).** Todas las pruebas en verde en CI (Linux+macOS) + `docker-checks.sh` en la Mac + **3 días de uso real sin parada indebida** + E1–E7 del PRD en la Mac + **revisión del portafolio por escrito** (ROADMAP §6: ¿resolvió el dolor?, ¿qué proyecto se pausa?, ¿cabe la estimación?). Decidir también nombre/licencia si procede (ADR-020).

---

## Fase 2 — App de barra (puerta G2)

- [ ] **T2.0 — Cerrar los puntos abiertos de UI y extraer el sistema visual.** Decidir también las **esquinas redondeadas de 14 px** del panel: una ventana sin decoraciones es de esquinas rectas; redondearlas exige ventana transparente (API privada de macOS en Tauri) o aceptarlas rectas (T0.5). *Deps:* G1. *Lee:* UI-REFERENCIA §6-7 y capturas. 
  Proponer a David y registrar (ADR-026): persistencia de «Ignorar» (tabla `ignored` en SQLite, por `cwd+match`), «Crear proyecto nuevo», «Marcar como legítimo», estados vacíos/error (sin proyectos, Docker apagado, núcleo reconectando), orden de tabulación/foco/VoiceOver, filtro «Alertas», modo claro (diferido). Extraer tokens y estructura de `brag-output/*/composition/index.html` a `docs/ui-referencia/tokens.md`.
  *Hecho cuando:* David confirma cada punto; los PNG existentes cubren los estados diseñados.
  → `docs(ui): record adr-026 ui decisions and design tokens`

- [ ] **T2.1 — Shell Tauri que aloja el núcleo + tooling de `ui/`.** *Deps:* T2.0. RF-14.
  `crates/devports-app` arranca el núcleo y el socket; `ui/` con Vite+React 19+TS estricto (`strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, cero `any`), ESLint, Prettier, Vitest + Testing Library; bindings generados con `tauri-specta` (`ui/src/bindings.ts` no se edita; CI verifica deriva); texto UI centralizado; el núcleo empuja instantáneas por eventos Tauri, React sin lógica de negocio. Ícono con contador.
  → `feat(app): host core in tauri shell with tray counter` · `feat(ui): scaffold react panel with generated bindings`

- [ ] **T2.2 — Panel: Atención, Proyectos, Compartidos, Sin proyecto, Actividad.** *Deps:* T2.1. UI-REFERENCIA §3-4; capturas 01–18.
  Árbol proyecto→repo→servicio, tokens de color/tipografía, `prefers-reduced-motion`, accesibilidad (botones reales, foco visible). *Pruebas:* componentes con datos simulados (contador, cuentas regresivas, advertencia de dependientes).
  → varios `feat(ui): …` por pestaña

- [ ] **T2.3 — Acciones.** *Deps:* T2.2. RF-05, RF-12, RF-16.
  Detener Principal (confirmación; `data`/compartido muestra dependientes), Extender, Terminar, Conservar, Marcar como legítimo, Adoptar y **escribir manifiesto preservando comentarios** (I-15), Copiar comando, Abrir URL, Detener los N. Estas operaciones existen **solo** en comandos Tauri, nunca en el socket (ADR-009; test que lo verifica).
  → `feat(app): add stop, extend, adopt and legitimize commands`

- [ ] **T2.4 — Notificaciones.** *Deps:* T2.3, T0.6. Vencimiento T-60 s y duplicado agrupado (ventana 10 min por proyecto), solo esos dos casos (ADR-014); según V6, con botón o abriendo el panel enfocado. → `feat(app): add grouped system notifications`

- [ ] **T2.5 — Vista de Actividad.** *Deps:* T2.2. RF-15; filtros Todo/Rechazos/Temporales(+Alertas si T2.0 lo confirma). → `feat(ui): add activity view with filters`

- [ ] **T2.6 — Autoarranque y supervisión.** Instalar el agente con `.plist` propio (`RunAtLoad`, `KeepAlive{SuccessfulExit:false}`, `ThrottleInterval` 5; el plugin de Tauri no escribe `KeepAlive`) o con `SMAppService`; repetir `tools/spike-launchd.sh` contra el binario real. *Deps:* T2.1, T0.7. RF-18; ARQUITECTURA §11. *Pruebas:* M-15. → `feat(app): add login launch and crash restart`

- [ ] **T2.7 — Medición del presupuesto y palancas.** *Deps:* T2.1–T2.6. NF-01; M-14 (1 h en reposo). Ajustar intervalo/profundidad si no cumple; si el panel anclado es deficiente, decisión sobre SwiftUI **antes** de seguir. → `perf: tune observation within idle budget`

- [ ] **T2.8 — Release v0.1.0 (opcional, tras G2).** `release.yml` con bundle macOS sellado ad hoc (copiar de GitCanvas), `/release`. PR `dev → main` lo aprueba David.

- [ ] **G2 — PUERTA FASE 2 (HUMANA).** UI y M-13..M-15 en verde + presupuesto cumplido + 5 días de uso diario sin cuelgues.

---

## Fase 3 — Integración con el ecosistema (repo `ci4-kickstart`, sesión aparte)

> Trabajo en **otro proyecto**; PR enfocado en ese repo (AGENTS.md raíz). Depende de que `ci4-kickstart` y CI4 Ops Platform estén probados de punta a punta.

- [ ] **T3.1** `ci4-kickstart` genera `.devports.yml` por proyecto nuevo.
- [ ] **T3.2** `start-all.sh` con `devports run --reuse` (CLI §10).
- [ ] **T3.3** Inserta el bloque de `AGENTS.md`/`CLAUDE.md` (CLI §9).
- [ ] **T3.4** MySQL único en `shared.yml` + `uses: [mysql]`.
- [ ] **T3.5** Opcional: `import_ops_registry`.
- [ ] **G3 — PUERTA (HUMANA).** Un proyecto CI4 nuevo nace integrado y pasa E2 y E3 de punta a punta.

## Fase 4 — Un mes de uso y decisión de publicar (HUMANA)

- [ ] **F4.1** Un mes de uso real.
- [ ] **F4.2** David define **por escrito** las señales externas para un venture (antes de publicar).
- [ ] **F4.3** Decidir licencia, nombre definitivo (libre como comando, repo y dominio) y publicación (ADR-020).
- [ ] **G4 — PUERTA.** Decisión registrada en `docs/DECISIONES.md`.

---

## Decisiones y hallazgos vivos
- ADR-023: `--replace` fuera de v1 (código 15, U-12 e I-12 retirados, no se reutilizan). ADR-024: Principal de hecho y raíz `~/developer` confirmadas. ADR-025 (T0.1) redactado; ADR-026 (T2.0) pendiente.
- Abiertos: runtime de Docker/compose (T0.4); nombre y licencia (Fase 4).
- Hooks: tras clonar, ejecutar `bash scripts/install-git-hooks.sh` (copia `pre-commit`, `pre-push` y `commit-msg` a `.git/hooks`).
- `brag-output/` no se versiona; el marcado exacto de las vistas vive ahí hasta T2.0.
