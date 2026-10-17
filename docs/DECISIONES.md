# DevPorts: Registro de decisiones

> Nombre de trabajo. El nombre definitivo se decide al terminar la Fase 1 (ver ADR-020).
> Origen: sesión de definición con David, 2026-10-16. Todas las decisiones marcadas **Confirmada** fueron aprobadas explícitamente por David en esa sesión.
> Este documento es la fuente de verdad ante cualquier contradicción con otros documentos de `docs/`. Si algo no coincide, manda este archivo.

Convenciones: **Confirmada** = aprobada por David. **Derivada** = consecuencia de implementación que sale de decisiones confirmadas, pero que David no aprobó por separado; confirmar antes de implementar si hay duda. **Propuesta** = valor sugerido a validar con mediciones.

---

## ADR-001 Un proyecto es la carpeta que contiene un `.devports.yml`
**Estado:** Confirmada.
**Contexto:** `ci4-kickstart` crea varios repos (hub, admin, domains) dentro de una carpeta contenedora que no es repo. La regla "raíz del repo git = proyecto" partiría un mismo proyecto en varios.
**Decisión:** el proyecto es la carpeta que contiene `.devports.yml` (o el `PROJECT.md` que genera CI4 Ops Platform). Los repos son grupos dentro del proyecto. Sin manifiesto, respaldo: raíz del repo git, y la app sugiere crear el manifiesto.
**Consecuencias:** el servicio "admin del proyecto X" tiene identidad aunque hoy esté caído.

## ADR-002 El manifiesto nace de la observación
**Estado:** Confirmada.
**Decisión:** nunca se escribe desde cero. Vías: (1) "adoptar" propone el manifiesto desde lo que corre en una carpeta y David confirma; (2) CI4 Ops Platform lo genera en proyectos nuevos del kit; (3) "marcar como legítimo" agrega un proceso observado al manifiesto en un clic.
**Motivo:** un manifiesto escrito a mano se desactualiza y produce falsos "intrusos".

## ADR-003 Identidad por (proyecto, servicio), no por puerto
**Estado:** Confirmada.
**Contexto:** los frameworks saltan de 3000 a 3001 cuando el puerto está ocupado; los 7 servidores diarios que sufre David son eso.
**Decisión:** un servicio se identifica por (proyecto, nombre de servicio) con una huella: patrón del comando normalizado (`match`) más directorio de trabajo. El puerto del manifiesto es "preferido", no identidad. Duplicado = dos procesos vivos con la misma identidad.
**Descarta:** la regla "el puerto declarado es el oficial".

## ADR-004 La atribución solo informa; no decide acciones
**Estado:** Confirmada.
**Contexto:** el árbol de procesos se rompe (reasignación de padre, daemons, contenedores).
**Decisión:** se muestra la cadena de procesos ancestros (app o terminal de origen). Una tabla extensible de nombres conocidos solo agrega etiquetas; lo desconocido se muestra como proceso crudo. Ninguna acción destructiva depende de la atribución.
**Requisito de David:** funcionar con cualquier agente o editor, sin lista fija.

## ADR-005 Dos tipos de instancia: Principal y Temporal
**Estado:** Confirmada.
**Contexto:** una IA levanta un servidor por desconocimiento (duplicado por error) o para pruebas/QA con datos dummy (legítimo). Importa el propósito y el ciclo de vida.
**Decisión:**
- **Principal:** instancia oficial de un servicio. Máximo una. Sin vencimiento.
- **Temporal:** declarada con `--as test --reason "..."`. Convive con la Principal. Siempre con vencimiento. Máximo 2 por servicio.
**Vencimiento:** 30 min por defecto, renovable, máximo 4 h acumulado por instancia. Aviso 1 min antes con "Extender 30 min". Al vencer se detiene.

## ADR-006 Solo se detiene automáticamente lo registrado con vencimiento
**Estado:** Confirmada.
**Decisión:** devports solo detiene automáticamente Temporales con vencimiento. Nunca mata solo lo que David levantó, ni procesos sin registrar, ni servicios `kind: data`. Mentir declarando "test" es inofensivo: crea algo que expira solo, con tope de 2.

## ADR-007 `devports run` es un envoltorio en primer plano, no un gestor
**Estado:** Confirmada (la semántica de `--reuse` es Derivada, ver ADR-008).
**Decisión:** `devports run` consulta al núcleo, se registra y se reemplaza por el comando real (`exec`) en el terminal del que llama. El proceso queda del usuario. El núcleo no posee procesos, no maneja logs ni reinicios.
**Descarta:** convertir devports en un gestor tipo pm2 (decisión de alcance v1).

## ADR-008 Comportamiento determinista, sin preguntas
**Estado:** Confirmada. **Derivada:** el comportamiento exacto de `--reuse`.
**Contexto:** detectar "terminal interactiva" es frágil: editores como Cursor ejecutan comandos del agente en una terminal real y una pregunta interactiva colgaría al agente.
**Decisión:**
- `devports run` nunca pregunta por defecto, llame quien llame.
- Ante conflicto se niega (código de salida distinto de 0) con un mensaje que dice qué ya corre (URL, rama, desde cuándo) y el comando para cada alternativa: reutilizar y `--as test --reason "..."`. Reemplazar la Principal se hace desde la app (ADR-023).
- La pregunta interactiva (reutilizar / temporal / cancelar) existe solo si David exporta `DEVPORTS_INTERACTIVE=1` en su shell. Apagada por defecto.
**Nota:** la reconciliación derivada de `--replace` (solicitud aprobada en la app, código 15) fue reemplazada por ADR-023: `--replace` no existe en v1.
**Derivada (reconciliación):** "reutilizar" es la bandera explícita `--reuse`: no inicia nada, imprime la instancia existente y sale con código 0. Los scripts (`start-all.sh`) usan `--reuse` para ser idempotentes. Sin la bandera, un conflicto es rechazo.

## ADR-009 Permisos del CLI
**Estado:** Confirmada. **Derivada:** el mecanismo del token (ver CLI.md).
**Decisión:** quien llama al CLI puede detener y extender solo los Temporales que él mismo registró. Principales y todo lo demás se detienen únicamente desde la app de barra con un clic de David. El CLI no ofrece esa operación, ni con bandera.
**Peor caso de un agente que abusa o se equivoca:** gestionar sus propias pruebas con vencimiento.

## ADR-010 La configuración de pruebas la resuelve la IA
**Estado:** Confirmada.
**Decisión:** el manifiesto no tiene bloque `test:`. Un Temporal recibe de devports solo: un puerto libre (en `${PORT}` y en la variable `DEVPORTS_PORT`), un registro con propósito y un vencimiento. El comando y el entorno los trae la IA: `devports run api --as test --reason "..." -- <su comando>`.
**El vencimiento se ata al puerto reservado, no a un proceso.** Al vencer se detiene lo que escuche en ese puerto: el proceso del host con sus descendientes, o `docker stop` si es un contenedor que publica ese puerto. Funciona aunque la IA use `docker run -d`.
**Derivada (hallazgo de diseño):** el shell del que llama expande `$PORT` y `${PORT}` antes de que devports vea los argumentos, y queda vacío. En el comando pasado después de `--`, devports sustituye el token literal `{PORT}` (sin `$`). En `start` del manifiesto sirve `${PORT}`. Ver CLI.md y MANIFIESTO.md sección 4.
**Riesgo a probar en Fase 0:** entre reservar el puerto y que la app lo abra, otro proceso podría tomarlo. Mitigación: verificar que quien lo abre nació después de la reserva y desde la misma carpeta; si no, avisar y no atar el vencimiento a ese proceso.

## ADR-011 Frontera: procesos, contenedores y puertos; no contenido de bases de datos
**Estado:** Confirmada.
**Decisión:** devports no gestiona el contenido de las bases de datos (por ejemplo una base `app_test` dentro de un MySQL). Limpiar datos dummy es responsabilidad de la IA. devports solo puede avisar si un Temporal de tipo data venció.

## ADR-012 Docker
**Estado:** Confirmada.
**Decisión:** (1) contenedores de compose se asignan al proyecto por las etiquetas `com.docker.compose.project` y `com.docker.compose.project.working_dir`; (2) contenedores sueltos se declaran en el manifiesto (`docker: nombre`) o aparecen "sin proyecto" para asignarlos con un clic; (3) la única acción es `stop`: sin `rm` ni borrado de volúmenes en v1; (4) detección por stream de eventos de la API de Docker; (5) el socket se obtiene del contexto activo (`docker context inspect`), que cubre Docker Desktop, OrbStack y Colima sin rutas a mano.
**Pendiente de verificar en Fase 0:** qué runtime usa David y si usa compose o `docker run`. No bloquea el diseño.

## ADR-013 Servicios compartidos
**Estado:** Confirmada.
**Contexto:** David usa una sola imagen de MySQL en Docker para todos sus desarrollos. Otros usuarios pueden tener una por repositorio.
**Decisión:** un servicio puede pertenecer a un proyecto o ser compartido. Los compartidos se declaran en `~/.config/devports/shared.yml` y cada proyecto los referencia con `uses: [mysql]`. Detener un compartido muestra primero qué proyectos lo usan. Un MySQL es `kind: data`.

## ADR-014 Notificaciones: estado pasivo primero
**Estado:** Confirmada.
**Decisión:** el ícono de la barra muestra un contador de cosas que requieren atención (duplicados sin registrar + Temporales por vencer). Notificación del sistema solo en dos casos: (1) un Temporal vence en 1 minuto, con "Extender 30 min"; (2) aparece un proceso sin registrar que duplica una Principal, agrupado: máximo una notificación por proyecto cada 10 minutos. Registro de actividad en v1, derivado del log de eventos.

## ADR-015 Alcance v1
**Estado:** Confirmada.
**Dentro:** proyectos, servicios, instancias con propósito y vencimiento, `devports run`, lista en la barra, detener, copiar comando de inicio, detección de duplicados sin registrar, notificaciones agrupadas, registro de actividad.
**Fuera de v1:** servidor MCP (el agente usa el CLI + instrucción en `AGENTS.md`), adaptadores por agente (hooks), proxy de nombres estables (`*.localhost`), versión Pro y equipos, iniciar procesos gestionados (pm2), borrar contenedores o volúmenes.
**Un solo proceso:** app de barra con núcleo Rust adentro; el CLI es un binario aparte que habla con ella por socket Unix.

## ADR-016 Plataforma
**Estado:** Confirmada.
**Decisión:** solo macOS en v1. La detección de procesos va en un módulo con interfaz clara ("fuente de procesos") para portar a Linux escribiendo un módulo. Windows fuera por ahora.

## ADR-017 Tecnología: Rust + Tauri v2 + React
**Estado:** Confirmada. **Propuesta:** el presupuesto numérico.
**Alternativas evaluadas:** Swift/SwiftUI (mejor integración y consumo, pero lenguaje nuevo y atado a macOS), Electron (peor consumo), Rust con menú nativo (UI muy limitada).
**Reglas de estructura (Confirmadas):**
1. El núcleo es un crate aparte sin dependencia de Tauri; se prueba sin interfaz. La envoltura es reemplazable.
2. El CLI es su propio binario liviano, no el de Tauri.
3. El webview se crea al abrir el panel y se destruye al cerrarlo.
**Presupuesto de la Fase 0 (Propuesta):** menos de 50 MB en reposo y menos de 1 % de CPU. Si no se cumple, o el popover anclado al ícono resulta deficiente, evaluar Swift para la envoltura.

## ADR-018 Si el núcleo no está corriendo, `devports run` falla abierto
**Estado:** Confirmada.
**Decisión:** intenta arrancar la app automáticamente; si no puede, ejecuta el comando igual y avisa "devports no está activo, sin protección". Lo que se cuele mientras estuvo caído aparece luego como "sin registrar" por la detección.

## ADR-019 Descubrimiento de proyectos
**Estado:** Confirmada (esquema). **Propuesta:** raíz por defecto.
**Decisión:** lista de carpetas raíz en la configuración; búsqueda de `.devports.yml` hasta 3 niveles de profundidad, con nueva revisión ante cambios. Un proceso fuera de todo proyecto aparece "sin proyecto" con botón "adoptar".
**Dato verificado:** los proyectos de David viven bajo `/Users/davidcardenas/developer/php/<proyecto>` (según `ci4-ops-platform/FASE1-DISENO.md`). Raíz por defecto propuesta: `~/developer`. David no confirmó la ubicación; verificar en Fase 0.
**Integración opcional:** importar `~/.ci4-ops/projects.yml` (campo `ruta_local`) como fuente adicional de raíces.

## ADR-020 Publicación, licencia, nombre y camino a producto
**Estado:** Confirmada.
**Decisión:** repo privado ahora. Licencia, nombre definitivo (candidatos: Faro, Portero, Muelle; verificar libre como comando, repo y dominio) y publicación se deciden al terminar la Fase 1.
**Camino:** (1) uso propio, v1 privada; (2) tras pasar el criterio de la Fase 0 y un mes de uso, publicar como GitCanvas: gratis con apoyo voluntario; (3) pasar a venture solo ante señales externas reales (equipos que piden manifiestos compartidos, gente que pide pagar), definidas por David antes de publicar.
**Diferencial frente a Berth y Bosun:** el ciclo de vida Principal/Temporal para agentes.

## ADR-021 Orden de construcción
**Estado:** Confirmada.
**Decisión:** núcleo y CLI sin interfaz antes que la app de barra (ver ROADMAP.md). Pruebas: fuente de procesos falsa para unitarias, procesos reales para integración, Docker real solo en la Mac de David (el CI de macOS de GitHub no tiene Docker).

## ADR-022 Criterio de éxito de la Fase 0
**Estado:** Confirmada.
**Decisión:** línea base medida antes de usar la herramienta (`tools/baseline.sh`). Avanzar si los incidentes bajan al menos a la mitad y la herramienta se abre 3 o más veces por día de trabajo. Si no, parar.
**Dato de David (estimación a ojo, sin medir):** el problema ocurre todos los días; parte con 1 repo montado y durante el día la IA llega a montar hasta 7 servidores simultáneos porque no entiende que ya tenía el servicio levantado.

## ADR-023 `--replace` queda fuera de la v1
**Estado:** Confirmada (David, 2026-10-16, al preparar el plan de implementación).
**Contexto:** `--replace` exigía un mecanismo extra (solicitud, permiso de un solo uso de 5 minutos, tabla `approvals`, código de salida 15, tarjeta Aprobar/Rechazar) solo para reemplazar una Principal desde el CLI, algo que el CLI no puede hacer por ADR-009.
**Decisión:** no existe `--replace` ni su flujo. Ante una Principal sana o colgada, el rechazo (códigos 10 y 11) indica "detén la instancia desde la app y vuelve a ejecutar". Se retiran: el código de salida 15 (queda **sin asignar**, no se reutiliza), el error `replace_pending`, la tabla `approvals`, los eventos `replace_*`, las pruebas U-12 e I-12 y la solicitud de reemplazo en el panel.
**Motivo:** principio de David, simple pero robusto y sin funciones que hoy no se necesitan. Reevaluable en una versión posterior con evidencia de uso.

## ADR-024 Principal de hecho y raíz de descubrimiento
**Estado:** Confirmada (David, 2026-10-16).
**Decisión:** (1) Sin Principal registrada, cuenta como Principal la instancia observada que escucha en el `port` preferido o, si no hay, la de `start_time` más antiguo (`registered: false`); las demás coincidencias son duplicados sin registrar (ARQUITECTURA §5). (2) La raíz de descubrimiento por defecto es `~/developer` (ADR-019). La ruta real en la Mac es `/Users/davidcardenas/Developer`; macOS no distingue mayúsculas, así que se expande `~` y se compara canonicalizada.

## ADR-025 Decisiones técnicas de implementación
**Estado:** Derivada (tarea T0.1, 2026-10-17). Sale de ADR-015, ADR-016 y ADR-017 y de las convenciones de GitCanvas; ajustable con una nota aquí si una medición de la Fase 0 lo exige.
**Decisiones:**
1. **Workspace y lints.** Cuatro crates (`devports-proto`, `devports-core`, `devports-cli`, `devports-app`), edición 2024, `resolver = "3"`, toolchain fijada en `rust-toolchain.toml` (1.98.1). Los lints viven en el workspace y se heredan: `unsafe_code = deny`, `missing_docs = deny`, `clippy::pedantic` como aviso y `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `todo`, `unimplemented`, `dbg_macro`, `print_stdout`, `print_stderr`, `string_slice`, `arithmetic_side_effects` y `disallowed_methods` en `deny`. CI usa `-D warnings`. `clippy.toml` permite `unwrap`, `expect`, `panic`, indexación y `print` dentro de pruebas.
2. **Reglas de dependencia verificadas.** `crates/devports-core/tests/architecture.rs` resuelve el grafo con `cargo metadata` y falla si `devports-core` o `devports-proto` alcanzan `tauri`, o si `devports-cli` alcanza algo distinto de `devports-proto` (cierre transitivo).
3. **`unsafe` y procesos.** `unsafe_code` es `deny` en el workspace, **no** `forbid`: `libproc` entrega la información de protocolo de un socket como una unión de C y leer un campo de unión exige `unsafe`; `forbid` no admite excepción local. Las excepciones son **tres archivos**, cada uno con su justificación: `devports-core/src/platform/macos.rs` (una lectura protegida por el discriminante `soi_kind` que informa el kernel), `devports-app/src/window_style.rs` (el `NSWindow` que Tauri entrega como puntero crudo, para redondear esquinas y fijar la opacidad con API pública de AppKit) y `devports-app/src/notifications.rs` (clase delegada de Objective-C y su envoltorio `Send`/`Sync`). `crates/devports-core/tests/architecture.rs` falla si `unsafe` o `allow(unsafe_code)` aparecen en cualquier otro archivo de `src/`, o si el workspace deja de usar `deny`. `std::process::Command::new` y `std::thread::sleep` están en `disallowed-methods` y **el núcleo no crea procesos**: Docker se alcanza por su socket y el contexto de Docker se lee del disco (punto 5). Por eso no existe un módulo `process_spawn` en el núcleo. Quien sí necesita `Command` lo hace en un módulo propio con un `#[allow(clippy::disallowed_methods, reason = ...)]` justificado: el **CLI** (`exec` del comando de `run` y `open -g -b` del falla abierto, T1.7) y, si hiciera falta, el **crate de la app** (`launchctl`, T2.6). Las pruebas que lanzan procesos relajan el lint en su cabecera.
4. **Concurrencia por actor.** El núcleo tiene un único actor que posee el estado del dominio y recibe mensajes por un canal `mpsc` (IPC, observadores, temporizador y UI). No hay `Mutex` compartido sobre el dominio: las transiciones de estado son deterministas y se prueban con reloj falso (PRUEBAS §1).
5. **Almacén y fuentes.** SQLite con `rusqlite` (modo bundled) y un único hilo escritor. Docker con `bollard` (solo transporte de socket Unix, característica `pipe`) sobre `tokio`; los traits de fuentes de contenedores usan `async fn` nativo en traits, sin `async-trait`, y se usan por genéricos. **Contexto de Docker:** `ARQUITECTURA.md` nombra `docker context inspect`; se lee directamente el almacén de contextos (`config.json` y `contexts/meta/<sha256 del nombre>/meta.json`), con el mismo orden de decisión que la CLI (`DOCKER_HOST`, `DOCKER_CONTEXT`, `currentContext`, `default`). Sin proceso hijo ni dependencia del binario `docker`. Se contrastó con `docker context inspect` en la Mac. Endpoints que no son `unix://` quedan como no soportados en v1. Procesos con `libproc` (sockets, padre, grupo, inicio) y `sysinfo` (`argv` y directorio de trabajo, porque `libproc::pidcwd` no está implementado en macOS) detrás del trait `ProcessSource`; fechas con `jiff`; YAML (lectura) con `serde-saphyr`, que informa línea y columna en los errores. La escritura que conserva comentarios se decide en T1.2. Tiempo detrás del trait `Clock`. Los eventos de Docker se piden con `since` = instante de la llamada porque el motor solo abre la suscripción en el primer sondeo del flujo; puede repetirse el segundo anterior, así que los consumidores tratan los eventos como pistas y concilian con `list`. **Presupuesto de memoria (T0.9):** NF-01 se mide como memoria física (`footprint`) de la app más sus procesos de WebKit, no como RSS (ARQUITECTURA §14). **Panel (H0):** esquinas redondeadas con API pública (capa del `contentView` con `cornerRadius`, ventana no opaca con fondo transparente), sin la API privada de transparencia de WebKit; la ventana se pone en pantalla con opacidad 0 y sin eventos de ratón, porque WebKit no pinta una ventana oculta (`requestAnimationFrame` no corre), y se hace opaca cuando la página avisa (`panel_ready`; respaldo de 1,5 s); el panel usa un almacén web no persistente (menos memoria y nada en disco). **Reloj:** `Clock` inyectable en el núcleo (`now` monótono y `wall` de calendario, `SystemClock` y `ManualClock`), usado ya por la guarda de reapertura del panel. **Comparación de programas:** por familia (`program_family`) solo en la posición del programa. Estas dependencias se incorporan en la tarea que las usa (T0.2, T0.4, T1.4), no antes.
6. **Texto para personas.** Los mensajes del CLI y del núcleo (español) viven en un módulo de mensajes central y se verifican con *golden files*; `--json` es el contrato estable y versionado. No hay literales de interfaz dispersos.
7. **Envoltura Tauri (T0.5).** `crates/devports-app` funciona con la CLI de Tauri 2 tal cual, **sin carpeta `src-tauri/`** (`tauri.conf.json` junto al `Cargo.toml` del crate; verificado con `@tauri-apps/cli` 2.12 `info` y `build --debug --no-bundle`). Sin ventana al arrancar, política *accessory* en tiempo de ejecución (sin ícono en el Dock), ícono de plantilla con contador en el título del ícono, panel creado al abrir y destruido al perder el foco, anclado con `tauri-plugin-positioner`. El crate solo compila en macOS en CI (Linux necesitaría WebKitGTK; ADR-016). **Notificaciones (T0.6, endurecido en H0):** el plugin de Tauri se **descartó**: envuelve `NSUserNotification` (obsoleta desde macOS 11, Apple puede retirarla). Se usa `UNUserNotificationCenter` directo (`objc2-user-notifications`, módulo `notifications` de la app): categoría con la acción «Extender 30 min», delegado para el clic, `willPresent` para que siempre salga como banner. Condiciones que descubrió la prueba: exige un paquete `.app` (sin él el sistema lanza una excepción, así que `Notifier::new` lo comprueba y devuelve `NoBundle`) y **un paquete sellado con firma ad hoc que ate el `Info.plist`** (`codesign --force --deep --sign -`); con la firma de enlazado de Tauri el sistema rechaza la solicitud sin preguntar (`granted: false`, `UNErrorDomain` 1). El empaquetado de la entrega (T2.8) debe sellar y verificar el paquete. **Supervisión y autoarranque (T0.7, V7 y V8):** comprobados con launchd real (`tools/spike-launchd.sh`); detalle en ARQUITECTURA §11. El agente se instalará en T2.6 con un `.plist` propio (`RunAtLoad`, `KeepAlive{SuccessfulExit:false}`, `ThrottleInterval` 5); el plugin de autoarranque de Tauri queda descartado (no escribe `KeepAlive`). A evaluar en T2.6: `SMAppService` (macOS 13+), que registra el agente desde el propio paquete y lo muestra en Ajustes > Elementos de inicio, con la excepción de `unsafe` ya existente en `platform::macos`; si no se usa, instalar el `.plist` y cargarlo con `launchctl bootstrap`. El paquete se construye con `LSUIElement` (`crates/devports-app/Info.plist`) y el CLI lanza la app por identificador (`open -g -b com.davidcardenas.devports`). Pendiente para T2.1/T2.8: `MACOSX_DEPLOYMENT_TARGET`, `LSUIElement` en el `Info.plist` del paquete y la firma.
8. **Sonda de la Fase 0.** Como el CLI no puede depender del núcleo, el `ls` sin núcleo de la Fase 0 es el binario temporal `devports-probe` del crate `devports-core` (`ls` y `opens`). Se elimina en T1.10, cuando `devports ls` hable con el núcleo por el socket.
9. **CI.** `dev-check` (ubuntu, `fmt`, `clippy -D warnings`, `test`) en cada push a `dev`. `quality` (ubuntu + macOS, más `cargo deny` y `cargo audit`) en el PR a `main`. Sin Windows (ADR-016).

## ADR-026 La puerta G0 se omite: se construye de inmediato
**Estado:** Confirmada (David, 2026-10-17).
**Contexto:** ADR-022 pedía una semana de línea base y una semana de uso de `devports ls` antes de construir el núcleo, y paraba el proyecto si los incidentes no bajaban a la mitad. David decidió construir ya y probar la app con su interfaz.
**Decisión:** la Fase 1 y la Fase 2 empiezan sin esperar a G0. La medición **no se cancela**: `tools/baseline.sh` y `devports-probe opens` siguen disponibles y la comparación se hace con la herramienta ya construida. El criterio de ADR-022 (incidentes a la mitad y 3 o más aperturas por día) pasa a evaluarse en la puerta G1/G2 con la app en uso real, y la regla «si no se cumple, parar y replantear» se mantiene.
**Riesgo aceptado:** invertir las 4 a 5 semanas de construcción antes de saber si la visibilidad sola ya resuelve el dolor.

---

## Decisiones reemplazadas (no implementar)
| Propuesta antigua | Reemplazada por |
|---|---|
| "El puerto declarado es el oficial" | ADR-003 |
| Rechazar siempre cualquier segundo servidor | ADR-005 y ADR-008 |
| Solo avisar, nunca actuar | ADR-006 (se actúa sobre Temporales vencidos) |
| Pregunta interactiva según haya terminal | ADR-008 |
| Servidor MCP como parte de la v1 | ADR-015 |
| Daemon separado de la interfaz | ADR-015 y ADR-017 |
| Regla "raíz del repo git = proyecto" | ADR-001 |
| Hooks por agente (Claude Code y similares) | ADR-015 |
| `devports run --replace` con aprobación en la app | ADR-023 |

## Supuestos y puntos abiertos
1. ~~Ubicación de los proyectos de David~~ Cerrado por ADR-024 (raíz `~/developer`).
2. ~~Runtime de Docker y uso de compose~~ Cerrado en T0.4: Docker Desktop, con compose y con `docker run` (ver `ARCHIVES.md`).
3. ~~`--replace`~~ Cerrado por ADR-023 (eliminado de v1).
4. ~~Principal de hecho~~ Cerrado por ADR-024 (regla confirmada).
5. Valores propuestos sin medir: presupuesto de memoria y CPU, intervalo de muestreo, códigos de salida, retención de eventos.
6. Plazos: ver ROADMAP.md. La estimación total supera las 2 semanas máximas sugeridas al registrar la idea; hay una puerta de revisión al terminar la Fase 1.

## ADR-027 Construcción rápida de las Fases 1 y 2: decisiones de la interfaz y del tiempo de ejecución
Fecha: 2026-10-17. Estado: vigente. Marcas **[D]** derivadas / **[P]** propuestas, por revisar con el uso real.
- **[P] «Conservar», «Marcar como legítimo» e «Ignorar»** son la misma acción: el proceso o contenedor se recuerda en la tabla `ignored` de SQLite (migración 2) por carpeta + comando (o nombre de contenedor), sobrevive a reinicios y deja de contar como atención y de avisar. No lo registra como Principal.
- **[P] «Temporal por vencer»** (Atención) = le quedan 5 minutos o menos; el aviso del sistema sigue siendo a 1 minuto (ADR-014).
- **[D] Adoptar** escribe el manifiesto solo desde la app (nunca por el socket): edición de texto que conserva comentarios y sangría, validada con el mismo analizador antes de escribir, escritura atómica; si el resultado no valida o el archivo usa la forma de flujo para `services`, no se toca nada. «Crear proyecto nuevo» usa la carpeta del proceso como raíz y no sobrescribe un manifiesto existente.
- **[D] Procesos del sistema** (`/System`, `/usr/libexec`, `/usr/sbin`, `/Library`, `/Applications`) no se listan en «Sin proyecto»: no son servidores de desarrollo.
- **[D] Descubrimiento de manifiestos** por repaso cada 5 s (no `notify`): más simple, sin dependencias y suficiente; se revisa si el costo en reposo (T2.7) lo exige.
- **[D] Contrato núcleo/interfaz:** los tipos de TypeScript se generan desde los de Rust con `specta` (`ui/src/bindings.ts`, prueba `crates/devports-core/tests/bindings.rs` que falla si el archivo difiere). Se descartó `tauri-specta` (tipa también los comandos) porque está en versión candidata y los comandos son pocos y estables; sus nombres viven en `ui/src/api.ts`. Los números exportados son de 32 bits (specta prohíbe `u64` hacia TypeScript).
- **[D] Empaquetado** con `tools/bundle.sh` (sin `tauri-cli`): construye la interfaz, la app y el CLI, genera el `.app` y lo sella ad hoc.
- **[D]** El CSP de la app permite estilos en atributos (`style-src-attr 'unsafe-inline'`) porque React los usa; no hay contenido remoto.
