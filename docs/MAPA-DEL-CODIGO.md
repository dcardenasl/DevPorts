# DevPorts: Mapa del código

Cómo está armado el código y por qué los límites caen donde caen. Es la guía de navegación: dice **dónde está cada cosa y en qué orden leerla**. El *qué* y el *porqué* de producto viven en `PRD.md` y `DECISIONES.md`; el diseño detallado en `ARQUITECTURA.md`. Este documento es el puente entre esos textos y las carpetas.

> Si algo de aquí contradice a `DECISIONES.md`, manda `DECISIONES.md`.

## 1. La idea en un minuto

DevPorts no es una ventana con lógica detrás. Es **un proceso residente en la barra de menú** que vigila la máquina todo el tiempo (procesos que escuchan puertos, contenedores Docker), decide qué es "oficial" y qué es una prueba con vencimiento, y atiende a **dos clientes**:

- **El panel** (React), que se abre y se destruye cada vez.
- **El CLI `devports`**, que usan los agentes de IA y la terminal.

Por eso hay 4 crates en lugar de los 2 de GitCanvas: el segundo cliente (el CLI) obliga a separar los tipos del protocolo.

```
 terminal / agentes                 App de barra de menú (un proceso, Tauri)
┌────────────────┐  socket Unix    ┌──────────────────────────────────────────┐
│ devports-cli   │  0600, JSON     │ devports-app   (el shell)                │
│ (cliente fino) │ ◄─────────────► │   tray, panel, notificaciones            │
└───────┬────────┘  por línea      │        │ aloja                           │
        │ solo depende de          │        ▼                                 │
        ▼                          │ devports-core  (TODA la lógica)          │
┌────────────────┐                 │   engine = máquina de estados pura       │
│ devports-proto │ ◄── usa ─────── │   runtime = actor + tareas alrededor     │
│ (tipos del     │                 │   ipc = servidor del socket              │
│  protocolo)    │                 │   store = SQLite                         │
└────────────────┘                 └──────────────────────────────────────────┘
                                          ▲ comandos Tauri (en proceso)
                                   ┌──────┴───────┐
                                   │ ui/ (React)  │  panel efímero
                                   └──────────────┘
```

## 2. Las cuatro cajas y qué NO pueden hacer

| Crate | Es | Depende de | Tiene prohibido |
|---|---|---|---|
| `devports-proto` | Los tipos que viajan por el socket (serde), nada más | `serde`, `serde_json`, `thiserror` (y `specta`, opcional, para generar `bindings.ts`) | I/O, lógica, depender de otro crate del workspace |
| `devports-core` | **Todas las reglas del producto** | `devports-proto` | depender de `tauri`; crear procesos (`Command::new`) |
| `devports-cli` | El binario `devports`: pregunta, imprime y hace `exec` | **solo** `devports-proto` | depender del core o de Tauri; tener lógica de dominio |
| `devports-app` | El shell: ícono, panel, notificaciones; aloja el core | core + Tauri | decidir reglas (solo traduce) |

Estas reglas **no dependen de que alguien las recuerde**: las vigila `crates/devports-core/tests/architecture.rs`, que lee el grafo real de `cargo metadata` (incluidas dependencias transitivas) y falla si aparece una arista prohibida. El mismo test confina `unsafe` a los archivos permitidos (ver §9).

Consecuencia práctica: `cargo test -p devports-core` prueba **todo el producto sin abrir una ventana ni tener Tauri instalado**. También hay `examples/headless.rs`, que levanta el core con su socket sin interfaz.

## 3. Cómo leer el core (orden recomendado)

`devports-core/src/` tiene unos 30 módulos planos. Leídos alfabéticamente no dicen nada; por **capas** sí. Orden de lectura sugerido:

### Capa 1: vocabulario (pequeño, léelo primero)
| Módulo | Qué contiene |
|---|---|
| `ids.rs` | *Referencia* (nombre público corto, `a1b2`) y *token* (secreto para detener/extender; solo se guarda su SHA-256). |
| `clock.rs` | `Clock` inyectable: `now()` monotónico y `wall()` de calendario. Ningún test espera tiempo real. |
| `hash.rs` | Utilidades SHA-256. |
| `reasons.rs`, `diagnostic.rs` | Errores y avisos **como datos** (código + parámetros), no frases. Cada idioma los escribe después. |
| `messages.rs` | Todas las frases en español que el core escribe para personas (CLI, bitácora). Un solo lugar, con pruebas "golden". |

### Capa 2: qué es un proyecto (datos que se leen del disco)
| Módulo | Qué hace |
|---|---|
| `manifest.rs` | Lee y valida `.devports.yml`. Nunca ejecuta nada; los errores llevan número de línea. |
| `yaml_keys.rs` | Posición (línea) de cada clave YAML, para que los mensajes digan dónde corregir. |
| `config.rs` | `config.yml` y `shared.yml` en `~/.config/devports/`. Ambos opcionales. |
| `discovery.rs` | Encuentra proyectos: cualquier carpeta con `.devports.yml`, hasta cierta profundidad, sin seguir enlaces simbólicos. |
| `catalog.rs` | Proyectos y servicios conocidos, y la regla que asigna un proceso observado a uno de ellos. |
| `fingerprint.rs` | La "huella" de la línea de comandos. **Un servicio se reconoce por lo que ejecuta, nunca por su puerto** (ADR-003). |

### Capa 3: observar la máquina (detrás de traits)
| Módulo | Qué hace |
|---|---|
| `process.rs` | Trait `ProcessSource`: lista procesos que escuchan TCP. Un proceso es `(pid, start_time)`, nunca solo `pid`. |
| `process/ancestry.rs` | Quién lanzó a quién (para vincular una instancia a su puerto reservado). |
| `process/terminate.rs` | **El único código que termina procesos.** Reglas estrictas: objetivo + descendientes, jamás grupos. |
| `process/fake.rs` | `ProcessSource` en memoria para pruebas. |
| `platform/macos.rs` | Implementación real con `libproc` + `sysinfo`. **Único lugar del core con `unsafe`.** |
| `docker.rs` + `docker/` | Trait `ContainerSource`; `engine.rs` habla con la API de Docker (`bollard`); `context.rs` encuentra el daemon activo (Docker Desktop, OrbStack, Colima); `fake.rs` para pruebas. |
| `gitinfo.rs` | Rama, worktree y última actividad leyendo `.git` directo (el core nunca lanza `git`). Sirve para avisar "este servidor corre código viejo". |
| `ports.rs` | Elegir un puerto libre y no prometido a otra instancia. |

### Capa 4: el cerebro
| Módulo | Qué hace |
|---|---|
| **`engine.rs`** + `engine/` | **La máquina de estados con todas las reglas.** Recibe datos (observación del host, reloj) y devuelve estado nuevo + una lista de `Effect`s (`Terminate`, `Notify`). **Nunca** lee el reloj, escanea ni toca procesos por sí mismo. |
| `engine/run.rs` | `devports run`: decidir quién puede iniciar qué. Determinista y sin preguntas (ADR-008). |
| `engine/binding.rs` | Seguir las instancias registradas: puerto reservado → proceso, proceso que desaparece → fin de instancia, duplicado no registrado. |
| `engine/leases.rs` | Vencimientos: los Temporales avisan antes y se detienen al vencer. **Solo se detiene automáticamente lo que tiene vencimiento** (ADR-006). |
| `engine/views.rs` | Convierte estado + observación en las instancias que ve la gente (registradas vs observadas; Principal "de hecho" y duplicados). |
| `engine/actions.rs` | Listar, bitácora y las acciones que solo una persona puede tomar desde la app. |
| `classify.rs`, `ignore.rs` | Consejo de agrupación y reglas de "ignorar", con criterio exacto. Clasificar **no** decide nada. |

### Capa 5: persistencia
| Módulo | Qué hace |
|---|---|
| `store.rs` | SQLite en modo WAL, una sola conexión (un solo escritor), versionada con `PRAGMA user_version`. Guarda solo lo que debe sobrevivir a un reinicio: instancias registradas (con el hash del token) y el log de eventos. |
| `document.rs` | Ediciones de texto con control de conflicto optimista (para `config.yml`, manifiestos). |

### Capa 6: acciones humanas sobre archivos (solo desde la app, nunca por socket)
| Módulo | Qué hace |
|---|---|
| `adopt.rs`, `adopt/` | "Adoptar": proponer un manifiesto a partir de lo que corre (`review.rs` congela y valida la propuesta; `apply.rs` escribe). |
| `settings.rs` | Editar las carpetas de búsqueda de `config.yml` conservando comentarios. |
| `uses.rs` | Editar qué proyectos declaran usar un servicio compartido. |

### Capa 7: ponerlo a correr
| Módulo | Qué hace |
|---|---|
| **`runtime.rs`** + `runtime/` | **Un actor es dueño del `Engine`**; el resto habla con él por canales. Alrededor corren tareas: escáner del host, supervisor de Docker (reporta "desconocido", nunca "desaparecido", si pierde conexión), vigilante del catálogo, sondeo de salud cada 5 s y el servidor IPC. La UI lee un `Snapshot` (canal `watch`) y recibe avisos (`broadcast`). |
| `runtime/probe.rs` | Chequeos de salud (TCP o HTTP 200–399, tope de 1 s). |
| `runtime/{adoption,folders,preferences,rules,uses}.rs` | Operaciones humanas que deben sobrevivir aunque el webview se destruya. |
| `ipc.rs` | Servidor del socket Unix: carpeta `0700`, socket `0600`, verifica el uid del par, limita línea y conexiones, valida versión del protocolo. |

> Truco de lectura: abre `lib.rs` (lista de módulos), luego `engine.rs` (cabecera de 15 líneas) y `runtime.rs` (cabecera con las tareas). Con eso ya entiendes el 80 % de la forma.

## 4. El flujo estrella: `devports run`

El caso que justifica que exista el producto: un agente quiere levantar un servidor y no debe duplicar uno que ya existe.

```
1. devports run api --reuse          (devports-cli/src/main.rs → run.rs)
2. CLI calcula su cadena de ancestros (devports-cli/src/ancestry.rs)
   porque solo este proceso sabe quién lo lanzó.
3. Envía Op::RunRegister por el socket (devports-proto/src/run.rs)
4. ipc.rs valida peer uid, tamaño y versión → pasa al actor
5. engine/run.rs decide, sin preguntar:
      ya hay Principal sana  → RunOutcome::Reuse  (no inicia nada)
      se puede iniciar       → reserva puerto + lease + token
      no se puede            → rechazo que dice qué existe y qué hacer
6. El CLI imprime el mensaje y hace exec() del comando
   (mismo pid: el core puede vincular el proceso con certeza)
7. engine/binding.rs ve el puerto reservado ocupado por ese proceso o un
   descendiente → la instancia queda "viva"
8. Si es Temporal, engine/leases.rs avisa antes del vencimiento y, al vencer,
   emite Effect::Terminate → process/terminate.rs lo ejecuta
```

Idea clave: **el CLI nunca decide.** Solo pregunta, imprime y ejecuta. Toda la decisión es del engine.

## 5. Qué puede hacer el socket y qué no

El socket (`Op` en `devports-proto/src/lib.rs`) expone **exactamente** lo que el CLI puede hacer:

`ping` · `run.register` · `ls` · `stop` y `extend` (con token) · `log` · `adopt.propose`

Detener Principales o procesos no registrados, aprobar, escribir manifiestos y marcar algo como legítimo **no están en el socket**: existen solo como comandos Tauri dentro de la app (ADR-009). Así, aunque un agente se comporte mal, no tiene cómo llegar a ellos. Los tokens nunca salen en `ls` ni en `log`.

## 6. El shell: `devports-app`

Es delgado a propósito. `commands.rs` abre con "solo traducen: toda regla es del core".

| Archivo | Qué hace |
|---|---|
| `main.rs` | Arranque: política "Accessory" (sin ícono en el Dock), registra comandos, inicia el host. Si ya hay otra instancia, sale. |
| `host.rs` | Arranca el core **antes** de que exista ninguna ventana y lo conecta con la interfaz (snapshots, avisos). |
| `commands.rs` | Los comandos Tauri que llama el panel. |
| `panel.rs` | Crea el panel al abrir y lo destruye al perder foco (así un panel cerrado no cuesta memoria de webview). |
| `panel_state.rs` | Contexto volátil que sobrevive a la destrucción del webview (pestaña, tarjetas abiertas, borradores). |
| `operations.rs` | Operaciones pedidas por una persona que deben terminar aunque el panel se cierre (adopciones, ediciones). El archivo más grande del shell. |
| `tray.rs` | Ícono de la barra con el contador de atención. |
| `notifications.rs` | Notificaciones con `UNUserNotificationCenter` (botón "Extender 30 min"). Uno de los 3 archivos con `unsafe`. |
| `window_style.rs` | Esquinas redondeadas con AppKit público. Uno de los 3 archivos con `unsafe`. |
| `integration.rs` | Poner `devports` en el PATH e iniciar al abrir sesión. Siempre acciones explícitas del menú. |
| `i18n.rs` | Frases del tray y notificaciones; `match` exhaustivo: una frase sin traducción **no compila**. |
| `preferences.rs` | Copia de las preferencias (tema/idioma) para abrir el panel ya con el tema correcto. El dueño del valor es el core (SQLite). |
| `diagnostics.rs` | Salida de errores a stderr hasta que haya logger (T2.1). |

## 7. El panel: `ui/src`

React + TypeScript. Efímero: se monta al abrir, se destruye al cerrar. **No tiene reglas de negocio**: "qué necesita decisión" lo decide el core (`snapshot.attention`).

| Grupo | Archivos |
|---|---|
| **Entrada** | `main.tsx`, `boot.ts` (corre antes de React; aplica tema e idioma que pidió el shell para evitar parpadeo), `App.tsx` (pestañas y composición) |
| **Puente con el core** | `api.ts` (**único** archivo que sabe cómo hablar con el core; en navegador usa datos de muestra), `bindings.ts` (**generado** desde Rust, no se edita), `types.ts` (solo lo que el panel *envía*), `connection.ts` (máquina de estados conectando/listo/reconectando), `hooks.ts` |
| **Pestañas** (`views/`) | `ProjectsView`, `AttentionView`, `OrphansView`, `SharedView`, `ActivityView`, `InstanceDetails`, `shared-parts` |
| **Hojas/modales** | `AdoptSheet`, `ClassifySheet`, `ConsumersSheet`, `IgnoreSheets`, `SettingsSheet`, `PreferencesSection`, `Modal`, `dialogs`, `StatusStrip` |
| **Lógica de presentación (pura)** | `model.ts`, `instance.ts`, `events.ts`, `reasons.ts`, `failures.ts`, `consumers.ts`, `settings.ts`, `adoption.ts`, `projectFolds.ts`, `time.ts`, `theme.ts` |
| **Idioma** (`i18n/`) | `es.ts`, `en.ts`, `provider.tsx`, `locale.ts`, `format.ts` |
| **Datos falsos** | `mock*.ts`, `sample.ts`: para desarrollar el panel en un navegador sin la app. Nunca se envían como contenido real. |

Regla de oro del puente: **Rust es la fuente de verdad de los tipos.** `bindings.ts` se regenera con:

```bash
UPDATE_BINDINGS=1 cargo test -p devports-core --features bindings --test bindings
```

y CI falla si el archivo versionado difiere de lo que produce el código.

## 8. Dónde viven los datos

| Qué | Dónde | Quién es dueño |
|---|---|---|
| Instancias registradas + bitácora | SQLite (WAL) | `store.rs`, escritor único |
| Manifiesto del proyecto | `<proyecto>/.devports.yml` | la persona; el core lo lee, lo propone y (solo desde la app) lo edita conservando comentarios |
| Configuración y servicios compartidos | `~/.config/devports/config.yml`, `shared.yml` | la persona |
| Socket | ruta calculada en `devports-proto/src/paths.rs` | core y CLI comparten la regla |
| Lo meramente observado | memoria | el engine; se pierde al reiniciar a propósito |

## 9. Reglas que el compilador o un test hacen cumplir

| Regla | Mecanismo |
|---|---|
| El core no importa Tauri; el CLI solo ve `proto` | `tests/architecture.rs` sobre `cargo metadata` |
| `unsafe` solo en `platform/macos.rs`, `window_style.rs`, `notifications.rs` | lint `unsafe_code = "deny"` + `architecture.rs` |
| Sin `unwrap`/`expect`/`panic`/`todo`/indexación cruda/aritmética sin control | lints `deny` en `Cargo.toml` raíz (un panic tumba la barra de menú y el socket) |
| Falta de documentación en API pública | `missing_docs = "deny"` |
| Frase sin traducción en la app | `match` exhaustivo, no compila |
| `bindings.ts` desfasado de Rust | prueba `tests/bindings.rs` |
| Requisito sin prueba | `tools/check_matrix.py` (cada prueba lleva `// covers: ID`) |
| El core no crea procesos | `Command::new` prohibido por `clippy.toml`; solo se permite en el CLI (`run.rs`, `ancestry.rs`), la app (`commands.rs`, `integration.rs`) y pruebas |

Detalle completo de reglas no negociables: `CLAUDE.md`, sección "Reglas técnicas".

## 10. Dónde buscar según lo que quieras hacer

| Quiero… | Empieza en |
|---|---|
| Cambiar cuándo se rechaza un `run` | `core/src/engine/run.rs` + `messages.rs` |
| Cambiar cómo se reconoce un servicio | `core/src/fingerprint.rs`, `catalog.rs` |
| Cambiar el aviso/parada por vencimiento | `core/src/engine/leases.rs`, `process/terminate.rs` |
| Añadir un campo al manifiesto | `core/src/manifest.rs` (+ `yaml_keys.rs`), docs `MANIFIESTO.md` |
| Añadir una operación al CLI | `proto/src/lib.rs` (`Op`) → `core/src/ipc.rs`/`runtime.rs` → `cli/src/main.rs`. Antes, ADR: el socket es una superficie que se amplía con cuidado |
| Añadir un comando del panel | `app/src/commands.rs` → registrarlo en `main.rs` → nombre en `ui/src/api.ts` |
| Cambiar un texto | CLI/bitácora: `core/src/messages.rs`. Tray/notificaciones: `app/src/i18n.rs`. Panel: `ui/src/i18n/{es,en}.ts` |
| Cambiar cómo se ve el panel | `ui/src/views/*`, `styles.css`, `theme.css`; referencia en `UI-REFERENCIA.md` |
| Probar sin Docker ni procesos reales | `process/fake.rs`, `docker/fake.rs`, `Clock` manual; ver `PRUEBAS.md` |

## 11. Tamaño de las piezas (para saber dónde está el peso)

El peso real está en el core (`runtime.rs`, `store.rs`, `engine/*`, `reasons.rs`) y en `operations.rs` del shell. La UI es comparativamente pequeña: el producto es un motor con una cara chica, a diferencia de GitCanvas, donde la UI es la mayor parte del código.

Los archivos `tests.rs` junto a cada módulo (`engine/tests.rs` supera las 2.500 líneas) son pruebas unitarias del módulo; las de integración con procesos y sockets reales están en `crates/*/tests/`.

## 12. Cómo se relaciona con el resto de la documentación

```
DECISIONES.md  ← fuente de verdad (ADRs)
   │
   ├─ PRD.md             qué y para quién (requisitos RF/NF)
   ├─ ARQUITECTURA.md    diseño detallado (secciones numeradas, V1–V12)
   ├─ MANIFIESTO.md      formato de .devports.yml y config
   ├─ CLI.md             comandos y códigos de salida
   ├─ PRUEBAS.md         estrategia y matriz requisito→prueba
   ├─ UI-REFERENCIA.md   panel
   ├─ ROADMAP.md / TASKS.md / ARCHIVES.md   fases, cola, evidencia
   └─ MAPA-DEL-CODIGO.md (este)   dónde está todo en las carpetas
```
