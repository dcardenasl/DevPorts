# TASKS — DevPorts

> **Fuente de verdad de ejecución.** Contexto y reglas para sesiones nuevas: [`CLAUDE.md`](CLAUDE.md).
> Evidencia de lo completado: [`ARCHIVES.md`](ARCHIVES.md). Especificación: [`docs/`](docs/) (orden en `docs/LEEME.md`).
> Ante cualquier contradicción manda [`docs/DECISIONES.md`](docs/DECISIONES.md).

**Estado:** núcleo, CLI y panel construidos (fases 1 y 2); pendientes G1/G2 y los flujos identificados en la auditoría de UI/UX del 2026-10-17.
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
**Siguiente trabajo de corrección:** UXR-1 a UXR-6 completados; continuar UXR-7.1 (accesibilidad y densidad del panel), según el [plan de UI/UX](docs/audits/2026-10-17-ui-ux-plan.md). La [auditoría](docs/audits/2026-10-17-ui-ux.md) registra 14 hallazgos; la [entrevista](docs/audits/2026-10-17-ui-ux-decisiones.md) cierra D1–D14 y ADR-028–035. UXR-3.1 añade contratos de revisión, conflictos tipados y guardado idempotente; [entrega y límites de concurrencia](docs/audits/2026-10-17-uxr-3-1.md), ADR-036 derivada. UXR-0 sigue abierta por timeout de Computer Use al acceder al bundle; no bloquea las tareas de implementación habilitadas.

**Fases 1 y 2 construidas; correcciones de UI/UX pendientes antes del cierre.** Además del plan UXR, siguen pendientes las comprobaciones humanas:
1. **G1 (puerta de la Fase 1):** `tools/docker-checks.sh` en la Mac (ya pasa), **3 días de uso real** con la app instalada (`tools/install.sh`), E1 a E7 del PRD, y la **revisión del portafolio** por escrito (ROADMAP §6).
2. **T2.0 pendiente de confirmación:** las decisiones de interfaz de ADR-027 se tomaron con valores por defecto; confirmarlas o cambiarlas con el uso.
3. **M-15:** reiniciar el Mac una vez y comprobar que la app arranca sola (`tools/launchd-check.sh` ya verifica el reinicio tras caída y la salida limpia).
4. **G2 / T2.8:** tras 5 días de uso diario, `/release` (el flujo y `release.yml` están listos).
S0 (línea base) quedó sustituida por el uso real (ADR-026).

---

<a id="uxr-sesiones"></a>

## Correcciones de UI/UX — tareas para sesiones independientes

La cola ejecutable vive en **TASKS.md**. El [plan UXR](docs/audits/2026-10-17-ui-ux-plan.md) conserva el diseño y sus ocho grupos; aquí se desglosan en **18 tareas de sesión**, sin cambiar las decisiones. Una tarea termina con su resultado y verificaciones, sin obligar a ejecutar la siguiente en el mismo chat.

### Contexto obligatorio de cada sesión
1. Leer `CLAUDE.md`, este protocolo y la ficha de la tarea elegida. Leer el [plan completo](docs/audits/2026-10-17-ui-ux-plan.md) la primera vez; en sesiones siguientes, sus límites/propuesta técnica y el grupo enlazado. Consultar los hallazgos UX citados en la [auditoría](docs/audits/2026-10-17-ui-ux.md) y las decisiones D citadas en la [entrevista](docs/audits/2026-10-17-ui-ux-decisiones.md); ADR-028–035 de [DECISIONES.md](docs/DECISIONES.md) son autoridad; ADR-036 documenta la implementación derivada de revisiones y archivos.
2. Verificar `dev`, estado local y log. No incluir ni reescribir `.devports.yml` no versionado del usuario. No hay autorización de push, publicación o release por ejecutar estas tareas.
3. Verificar en `ARCHIVES.md` las entregas reales de las dependencias: archivos, contrato y pruebas, no asumir implementación por existir el plan. Los archivos citados son puntos de entrada actuales; si otra tarea los movió, seguir su registro de entrega.
4. Trabajar solo la tarea indicada y sus correcciones necesarias. Dependencias no completadas bloquean la tarea; UXR-0 solo bloquea el cierre nativo final, no el desarrollo de UXR-1.1. Si falla acceso nativo, dejar evidencia pendiente y tomar una tarea habilitada, sin afirmar que mock equivale a Tauri.
5. Ejecutar comprobaciones pertinentes y DoD del repositorio. Para cambios de contratos, regenerar bindings mediante su mecanismo de Rust y probar consumidores; no editar `ui/src/bindings.ts` a mano. Fuente de comandos actual: `tools/check.sh full` y scripts de `ui/package.json` con **pnpm**, no los comandos npm heredados de tareas históricas.
6. Cerrar con `/commit-flow`: actualizar CHANGELOG si cambia comportamiento, documentación y evidencia. Usar solo recursos de prueba propios; no detener, adoptar, reiniciar ni reconfigurar los servicios reales de David para verificar.

### Registro de entrega obligatorio
Al terminar cada tarea, añadir en `ARCHIVES.md` una entrada con **ID, fecha, commits verificables, resultado, archivos/contratos creados o cambiados, verificaciones con resultado, evidencia nativa vs simulada, pendientes y notas para las tareas consumidoras**. Para recursos propios, indicar limpieza realizada. Marcar `[x]` solo con sus criterios completos; si queda incompleta, conservar `[ ]` y registrar el paso exacto pendiente en `SNAPSHOT.md`. No mover ni borrar la ficha de TASKS: conservar sus enlaces y dependencias estables. Si se divide una tarea de nuevo, añadir sufijos y actualizar consumidores antes de cerrar sesión.

En las fichas de implementación, las verificaciones nativas indicadas «cuando esté disponible» pueden quedar explícitamente asignadas a UXR-7.2, con la implementación y pruebas automatizadas completas. Esto no permite omitir los criterios funcionales de la ficha ni completar UXR-0/UXR-7.2 con mock.

Los grupos UXR-1 a UXR-7 se consideran cerrados cuando todas sus subtareas están completas; no tienen una segunda casilla que pueda contradecirlas. G1/G2 son puertas humanas independientes.

<a id="uxr-0"></a>

- [ ] **UXR-0 — Reproducir la app nativa con casos controlados.**

  **Dependencias:** ninguna. **Plan:** [grupo UXR-0](docs/audits/2026-10-17-ui-ux-plan.md#uxr-0). **Contexto:** Límite de evidencia de la auditoría; D1; ADR-028.

  **Leer código:** `crates/devports-app/src/{main,panel,host}.rs`, `tools/{bundle,install}.sh`, `ui/src/api.ts`, `ui/src/mock.ts`; informe de auditoría §3 y §6.

  **Trabajo de esta sesión:** Identificar el bundle activo y su correspondencia con el checkout sin asumir que la app instalada contiene el último código. Recorrer las cinco pestañas con procesos desechables propios y manifiestos temporales; guardar capturas y resultados nativos. Registrar cómo arrancar y limpiar esos recursos por su identidad, sin tocar servicios existentes del usuario.

  **Hecho cuando:** Se distingue evidencia Tauri real, frontend con mock y CLI real; se documentan pasos reproducibles y se limpian los recursos propios. Si Computer Use no accede al panel, registrar el bloqueo exacto y dejar esta tarea abierta; continuar una tarea de implementación sin dependencia de UXR-0. No inventar equivalencia entre mock y app nativa.

  **Verificación:** Comprobación macOS del panel, foco y pestañas; consulta del CLI del mismo paquete; no hacer paradas de los MySQL/PHP/Java del usuario.

  **Entrega a la próxima sesión:** Dejar en la auditoría versión/ruta verificadas, evidencias, inventario de recursos propios y comando de limpieza. Este contexto se reutiliza en UXR-7.2.

<a id="uxr-1-1"></a>

- [x] **UXR-1.1 — Establecer diálogos accesibles dentro del panel.**

  **Entrega:** `cb58060`; [informe, API y evidencia](docs/audits/2026-10-17-uxr-1-1.md). UI 25/25 y controles completos verdes; comprobación de Tauri/VoiceOver asignada a UXR-7.2.

  **Dependencias:** ninguna para implementar; validación nativa conjunta en UXR-7.2. **Plan:** [grupo UXR-1](docs/audits/2026-10-17-ui-ux-plan.md#uxr-1). **Contexto:** UX-05 y UX-14; D1; ADR-028.

  **Leer código:** `ui/src/{dialogs,App}.tsx`, `ui/src/styles.css`, `ui/src/App.test.tsx`; `crates/devports-app/src/panel.rs` para cierre por foco.

  **Trabajo de esta sesión:** Crear/reutilizar una primitiva de diálogo con foco inicial, contención Tab/Shift+Tab, Escape, fondo no interactivo y retorno al invocador. Aplicarla a adopción y parada; añadir Volver en subpasos. No añadir ventana independiente ni evitar el cierre del panel por clic fuera.

  **Hecho cuando:** Teclado completa y cancela ambos diálogos sin operar el fondo; Volver conserva entradas. El panel mantiene tamaño y cierre esperados. La falta de acceso nativo se registra como verificación pendiente, no como éxito en WebKit.

  **Verificación:** Pruebas UI de foco inicial/restaurado, ciclo Tab, Escape, Volver y cancelación sin llamadas de mutación; comprobación WebKit reutilizable en UXR-7.2.

  **Entrega a la próxima sesión:** Documentar la API de la primitiva y un ejemplo de uso para los formularios posteriores; registrar tests y limitaciones nativas.

<a id="uxr-1-2"></a>

- [x] **UXR-1.2 — Conservar navegación y borradores fuera del webview.**

  **Entrega:** `d8af7f3`, `36f00b1`, `e5ec97d`; [contrato, evidencia y límites nativos](docs/audits/2026-10-17-uxr-1-2.md). Estado tipado en memoria de la shell, 28/28 UI, shell Rust y controles completos verdes. Tauri/WebKit nativo asignado a UXR-7.2.

  **Dependencias:** UXR-1.1. **Plan:** [grupo UXR-1](docs/audits/2026-10-17-ui-ux-plan.md#uxr-1). **Contexto:** UX-11 y parte de UX-08; D2, D11; ADR-028.

  **Leer código:** `crates/devports-app/src/{host,panel,commands}.rs`, `ui/src/{App,dialogs}.tsx`, `ui/src/{hooks,api,types}.ts`, `crates/devports-core/tests/bindings.rs`.

  **Trabajo de esta sesión:** Mantener estado mínimo tipado en la shell Rust: pestaña, proyecto expandido, filtro y borradores con versión. Hidratar antes de permitir que valores iniciales sobrescriban el estado recuperado. Integrar borradores existentes y definir extensión para adopción/compartidos/configuración futuros; invalidar o advertir referencias que ya no existen.

  **Hecho cuando:** Cerrar/reabrir recupera contexto y entradas mientras la app sigue viva, sin guardar archivos ni ejecutar acciones. Nueva alerta actualiza contador sin cambiar pestaña. Nunca restaurar una confirmación destructiva como autorización. No mantener WebKit vivo ni prometer persistencia tras salir.

  **Verificación:** Pruebas de serialización/validación de estado y UI de hidratación, cierre/recreación simulados, referencia desaparecida y ausencia de guardado implícito; prueba nativa de destrucción/reapertura cuando esté disponible.

  **Entrega a la próxima sesión:** Registrar contrato y ubicación de almacenamiento en memoria, ciclo de vida, versión y método para que UXR-3.2/4.2/6.1 añadan sus borradores.

<a id="uxr-1-3"></a>

- [x] **UXR-1.3 — Seguir operaciones confirmadas aunque se cierre el panel.**

  **Entrega:** `b1b1bae`; [contrato, verificación y límites nativos](docs/audits/2026-10-17-uxr-1-3.md). Operaciones de shell consultables e idempotentes, historial acotado, resultados por objetivo y reintento explícito. `tools/check.sh full` y UI 30/30 verdes. Recreación nativa WebKit diferida a UXR-7.2.

  **Dependencias:** UXR-1.2. **Plan:** [grupo UXR-1](docs/audits/2026-10-17-ui-ux-plan.md#uxr-1). **Contexto:** UX-08; D7, D12; ADR-030 y ADR-034.

  **Leer código:** `crates/devports-app/src/{commands,host}.rs`, `crates/devports-core/src/runtime.rs`, `ui/src/{api,hooks,actions}.ts`, `ui/src/App.tsx`; contratos y bindings vigentes.

  **Trabajo de esta sesión:** Introducir identificador, estado consultable y resultados por objetivo para operaciones humanas de duración no instantánea. Ejecutarlas fuera del ciclo del webview; integrar operaciones existentes y documentar cómo registrar las nuevas. Evitar doble envío y repetición ante respuesta perdida. Añadir progreso, error persistente y reintento explícito; no convertir el toast en registro de resultados.

  **Hecho cuando:** Guardar/Detener enviados continúan al cerrar; reabrir consulta resultado sin reenviar. Confirmación no enviada se pide otra vez. Doble clic y respuesta perdida no duplican efectos; fallos/éxitos parciales son visibles. El historial en memoria es acotado y no guarda tokens ni argumentos sensibles innecesarios.

  **Verificación:** Contrato/UI con operación lenta, fallo, respuesta perdida, doble envío y destrucción del consumidor de respuesta; verificar efectos una sola vez y recuperación de resultados. No ampliar operaciones del socket CLI.

  **Entrega a la próxima sesión:** Dejar contrato estable de inicio/consulta/reintento, ciclo de vida y límites del registro; UXR-2/3/4/5/6 lo reutilizan sin inventar sistemas separados.

<a id="uxr-2-1"></a>

- [x] **UXR-2.1 — Confirmar toda parada manual y detener sin adoptar.**

  **Dependencias:** UXR-1.3. **Plan:** [grupo UXR-2](docs/audits/2026-10-17-ui-ux-plan.md#uxr-2). **Contexto:** UX-01; D3, D4; ADR-029.

  **Leer código:** `ui/src/views/{OrphansView,SharedView,AttentionView,ProjectsView}.tsx`, `ui/src/{App,dialogs}.tsx`, `ui/src/actions.ts`, `crates/devports-core/src/engine/actions.rs`, `crates/devports-core/src/process/terminate.rs`.

  **Trabajo de esta sesión:** Exponer Detener junto a Adoptar/Declarar. Unificar todas las rutas manuales, incluidos duplicados y Temporales, bajo confirmación con identidad, puerto, origen y consumidores declarados si corresponde. Revalidar objetivo al ejecutar y antes de cada señal; mantener reglas automáticas y permisos CLI.

  **Hecho cuando:** Cancelar no tiene efectos; parar un servidor suelto no crea un manifiesto. No queda ningún botón manual que salte la confirmación. Cambios de identidad/puerto o target desaparecido no señalan otro proceso; los datos requieren acción humana independiente.

  **Verificación:** UI de cada ruta y cancelación; pruebas existentes de terminación/identidad más casos de stale target; integración de parada real solo sobre servidor propio de prueba.

  **Entrega a la próxima sesión:** UXR-2.2 reutiliza `stopExpectation` por objetivo y la operación `stop`, conservando resultados separados y sin detener compartidos al parar un proyecto.

  **Entrega:** [`docs/audits/2026-10-17-uxr-2-1.md`](docs/audits/2026-10-17-uxr-2-1.md); commits de implementación y registro en `ARCHIVES.md`.

<a id="uxr-2-2"></a>

- [x] **UXR-2.2 — Detener aplicaciones de un proyecto como acción puntual.**

  **Entrega:** `8f693b4`; [contrato, evidencia y límites](docs/audits/2026-10-17-uxr-2-2.md). Vista previa del núcleo, operación puntual por objetivos, conservación de datos/compartidos y resultados reales de terminación. Controles completos, UI 35/35 y caso host propio verdes; verificación Tauri asignada a UXR-7.2.

  **Dependencias:** UXR-2.1. **Plan:** [grupo UXR-2](docs/audits/2026-10-17-ui-ux-plan.md#uxr-2). **Contexto:** UX-01; D4, D5, D14; ADR-029.

  **Leer código:** `crates/devports-core/src/{engine/actions,runtime}.rs`, `ui/src/views/ProjectsView.tsx`, `ui/src/dialogs.tsx`, contratos de parada y operaciones de UXR-1.3/2.1.

  **Trabajo de esta sesión:** Crear vista previa de las aplicaciones y Temporales del proyecto y confirmar una lista concreta. Excluir datos y compartidos en el núcleo, no solo en React. Ejecutar por objetivos y presentar resultado por instancia; no recalcular al ejecutar un conjunto más amplio.

  **Hecho cuando:** MySQL y todo kind:data permanecen activos; una instancia que aparezca después no se incorpora. Cambio de pertenencia/identidad se rechaza por objetivo. Un fallo no oculta otros resultados. No elimina archivos, desasocia recursos ni bloquea lanzamientos futuros.

  **Verificación:** Motor: app, Temporal, dato local, compartido, instancia nueva, identidad cambiada y fallo parcial. UI: lista exacta, confirmar/cancelar y reintento seguro de pendientes. Caso real con recursos propios.

  **Entrega a la próxima sesión:** Dejar política de selección/revalidación y evidencia de cero efectos sobre datos/compartidos; explicar estados parciales para UXR-7.2.

<a id="uxr-2-3"></a>

- [x] **UXR-2.3 — Mostrar origen y estados reales de las instancias.**

  **Entrega:** `423321c`; [contratos, presentación, verificación y límites](docs/audits/2026-10-17-uxr-2-3.md). Detalle compartido, estados reales, puertos observados y argumentos de diagnóstico sin valores. `tools/check.sh full`, UI 41/41 y recorrido con teclado a 380×520 verdes; Tauri/VoiceOver pendiente en UXR-7.2.

  **Dependencias:** UXR-2.1. **Plan:** [grupo UXR-2](docs/audits/2026-10-17-ui-ux-plan.md#uxr-2). **Contexto:** UX-07 (detalle), UX-12; D3 y D10.

  **Leer código:** `ui/src/views/{shared-parts,OrphansView,AttentionView,ProjectsView,SharedView}.tsx`, `ui/src/{text,model,time}.ts`, `crates/devports-proto/src/views.rs`.

  **Trabajo de esta sesión:** Añadir detalle desplegable con origen/lanzador, carpeta, identidad y puertos disponibles. Eliminar sana fija, distinguir Detenido de fallo y presentar stale como indicio explicado. Reducir argv largo como título, manteniendo acceso seguro al detalle; no inventar datos ausentes.

  **Hecho cuando:** Panel y snapshot/CLI expresan estados consistentes; desconocido no aparece detenido. Datos breves legibles y rutas/comandos largos inspeccionables al tamaño nativo; argumentos sensibles no se vuelcan sin tratamiento en logs/notificaciones.

  **Verificación:** Casos UI/modelo de saludable, detenido, desconocido, stale, origen ausente y textos largos; verificar visualmente 380×520.

  **Entrega a la próxima sesión:** Dejar componente/formato reutilizable de detalle y reglas de presentación para UXR-5.2 y UXR-7.1.

<a id="uxr-3-1"></a>

- [x] **UXR-3.1 — Preparar adopción elegible y edición validada.**

  **Entrega:** `5e4425a`; [contratos, pruebas y límites](docs/audits/2026-10-17-uxr-3-1.md). Revisiones congeladas, destinos elegibles, campos editables, conflictos e idempotencia; FileSnapshot reutilizable y metadatos macOS verificados. `tools/check.sh full`, UI 41/41 y build verdes. Interfaz revisada pendiente de UXR-3.2; sin nuevos permisos CLI.

  **Dependencias:** UXR-1.3. **Plan:** [grupo UXR-3](docs/audits/2026-10-17-ui-ux-plan.md#uxr-3). **Contexto:** UX-03 y UX-04; D8; ADR-031.

  **Leer código:** `crates/devports-core/src/{adopt,runtime,manifest}.rs`, `crates/devports-core/src/adopt/apply.rs`, tests de adopción y runtime; `crates/devports-app/src/commands.rs`.

  **Trabajo de esta sesión:** Separar propuesta/eligibilidad de aplicación: raíz git real, destinos válidos y motivos de rechazo, nombre/comando editables y advertencias. Resolver explícitamente proyecto provisional sin manifiesto. Preparar edición que preserve comentarios, valide y detecte cambios externos; devolución tipada de conflictos. No sustituir lo confirmado por una propuesta recalculada sin avisar.

  **Hecho cuando:** No se ofrece un host fuera de raíz como destino válido. cwd/public muestra la raíz y archivo reales. Validación/cancelación no escriben ni reinician. Conflicto externo conserva el archivo ajeno y exige revisar; reintento de una misma operación no añade otro servicio con sufijo.

  **Verificación:** Temporales de archivos: comentarios, servicios existentes, YAML no editable, proyecto provisional, raíz git, path no elegible, permisos y cambio externo; idempotencia y operación con respuesta perdida.

  **Entrega a la próxima sesión:** Publicar contratos de preview/apply, política de concurrencia con límites reales y primitivas de edición reutilizables por UXR-4.1/6.1. Mantener el flujo actual compilable hasta integrar la UI.

<a id="uxr-3-2"></a>

- [x] **UXR-3.2 — Completar adopción revisada desde el panel.**

  **Entrega:** [flujo, contratos y límites](docs/audits/2026-10-17-uxr-3-2.md). Revisión obligatoria con borrador y resultado recuperables, alcance de exploración validado en el núcleo; UI 43/43 y controles completos verdes. Tauri nativo asignado a UXR-7.2.

  **Dependencias:** UXR-3.1 y UXR-1.2. **Plan:** [grupo UXR-3](docs/audits/2026-10-17-ui-ux-plan.md#uxr-3). **Contexto:** UX-03, UX-04 y UX-08; D2, D8, D12; ADR-028, ADR-031 y ADR-034.

  **Leer código:** `ui/src/{dialogs,App}.tsx`, `ui/src/{api,types,text}.ts`, `ui/src/{mock,sample}.ts`, `ui/src/App.test.tsx`; contratos de UXR-3.1.

  **Trabajo de esta sesión:** Elegir destino → revisar raíz/archivo/puerto y editar nombre/comando → Guardar. Motivos claros para destinos no elegibles, Volver/Cancelar, borrador recuperable, errores junto al formulario. Aplicar revisión también al caso compartido; UXR-4.2 añadirá consumidores. Éxito muestra destino devuelto y enlace a su tarjeta.

  **Hecho cuando:** Siempre hay revisión antes de escribir; fallo conserva datos y permite reintentar; cerrar recupera formulario o resultado según ya se haya enviado. Adoptar no reinicia el servicio. Mock actualiza catálogo/snapshot de forma coherente en vez de solo eliminar la fila.

  **Verificación:** UI de destino rechazado, edición, advertencias, volver, error recuperable, cierre antes/después de enviar; integración que lee YAML y snapshot resultantes.

  **Entrega a la próxima sesión:** Dejar flujo/base de formularios para declaración compartida; registrar E2E persistido y pendientes nativos.

<a id="uxr-4-1"></a>

- [x] **UXR-4.1 — Guardar consumidores compartidos por proyecto.**

  **Entrega:** [contrato, política y límites](docs/audits/2026-10-17-uxr-4-1.md). Prevalidación total, resultado por archivo, reintento idempotente; sin pantalla (UXR-4.2). Controles completos y UI 47/47 verdes.

  **Dependencias:** UXR-3.1. **Plan:** [grupo UXR-4](docs/audits/2026-10-17-ui-ux-plan.md#uxr-4). **Contexto:** UX-02; D6, D7; ADR-013 y ADR-030.

  **Leer código:** `crates/devports-core/src/{config,catalog,runtime}.rs`, `crates/devports-core/src/adopt/apply.rs`, `crates/devports-app/src/commands.rs`, contratos de operación/edición de UXR-1.3/3.1.

  **Trabajo de esta sesión:** Añadir operación humana de consulta/vista previa/guardado de uses. Validar todos los manifiestos antes de comenzar, preservar comentarios y otras referencias, aceptar cero consumidores y detectar cambios externos. Resultado por archivo y reintento solo de pendientes; no ampliar IPC CLI.

  **Hecho cuando:** Error conocido en prevalidación produce cero escrituras. Fallo durante guardado muestra éxito parcial real, sin revertir cambios ajenos ni declarar éxito global. Releer y revalidar al reintentar; no duplicar recurso ya declarado. Desasociar no detiene ni borra; consumidores son declaraciones, no conexiones detectadas.

  **Verificación:** Dos o más manifiestos: añadir/quitar, vacío, repetición, comentarios, readonly, conflicto externo y fallo inyectado después del primer éxito. Verificar archivos y catálogo, no solo llamadas.

  **Entrega a la próxima sesión:** Dejar contrato de elegibilidad/selección/resultados por proyecto y tratamiento del recurso declarado con asociaciones pendientes para UXR-4.2.

<a id="uxr-4-2"></a>

- [x] **UXR-4.2 — Gestionar recursos compartidos y completar sus acciones.**

  **Entrega:** [flujo, estados y límites](docs/audits/2026-10-17-uxr-4-2.md). Hoja de consumidores con revisión, parcial/reintento y borrador recuperable; comando/endpoint solo si están declarados. UI 54/54 y controles completos verdes; recorrido en navegador y Tauri pendientes (UXR-7.x).

  **Dependencias:** UXR-4.1, UXR-3.2 y UXR-2.2. **Plan:** [grupo UXR-4](docs/audits/2026-10-17-ui-ux-plan.md#uxr-4). **Contexto:** UX-02 y UX-09; D4, D6, D7; ADR-029 y ADR-030.

  **Leer código:** `ui/src/views/{SharedView,ProjectsView}.tsx`, `ui/src/{dialogs,App}.tsx`, `ui/src/{model,api,text}.ts`, `ui/src/{mock,sample}.ts`; comandos y contratos de UXR-4.1.

  **Trabajo de esta sesión:** Seleccionar consumidores al declarar y editar después mediante Gestionar proyectos, también desde el contexto del proyecto con la misma fuente uses. Búsqueda, guardar/cancelar, cero asociados y pendientes por proyecto. Reutilizar revisión obligatoria. Recurso detenido: copiar comando existente; abrir navegador solo para endpoint HTTP explícito y válido, nunca por asumirlo a partir del puerto.

  **Hecho cuando:** Asociar dos proyectos actualiza YAML, etiquetas, snapshot y aviso de parada; detener apps de uno conserva MySQL. Sin proyectos asociados no afirma que nadie lo usa. Recursos inexistentes/proyectos provisionales tienen explicación y siguiente acción; no se inventan comandos ni se redeclara en un reintento.

  **Verificación:** UI y persistencia de declaración sin consumidores, selección/edición, cancelación, fallo parcial, reabrir y reintentar; recorrido conjunto con UXR-2.2 sobre datos de prueba.

  **Entrega a la próxima sesión:** Registrar flujo completo, asociación declarativa y evidencia para UXR-7.2; actualizar manual de manifiestos y uso.

<a id="uxr-5-1"></a>

- [x] **UXR-5.1 — Hacer ignorados explícitos y restaurables.**

  **Entrega:** [regla exacta, compatibilidad y límites](docs/audits/2026-10-18-uxr-5-1.md), ADR-037. Ignorar propio con vista previa de la regla, Ver ignorados/Restaurar y reglas anteriores intactas; UI 57/57 y controles completos verdes.

  **Dependencias:** UXR-1.3. **Plan:** [grupo UXR-5](docs/audits/2026-10-17-ui-ux-plan.md#uxr-5). **Contexto:** UX-06; D9; ADR-032.

  **Leer código:** `crates/devports-core/src/engine.rs` (ignore_key), `engine/actions.rs`, `store.rs` (remove_ignored), `runtime.rs`; `ui/src/{App,dialogs}.tsx`, `ui/src/views/OrphansView.tsx`.

  **Trabajo de esta sesión:** Separar Ignorar de Adoptar, explicar criterio, listar reglas incluso sin instancia viva y restaurar por operación humana. Revisar clave persistente: argv crudo incluye puerto pese al comentario, concatenar argumentos puede colisionar, contenedor requiere contexto. Mantener reglas antiguas recuperables; documentar compatibilidad/migración sin ampliar coincidencias silenciosamente.

  **Hecho cuando:** Ignorar no detiene/adopta; reinicio del servicio conserva exclusión según criterio explícito. Restaurar vuelve a observar sin tocar SQLite a mano; reglas antiguas no se pierden. No ocultar otra carpeta/comando/contexto por accidente.

  **Verificación:** Motor/almacén con proceso recreado, cambio de puerto según regla elegida, argv con espacios, contextos y reglas antiguas; UI de lista vacía, regla sin proceso y restauración; persistencia real tras reapertura.

  **Entrega a la próxima sesión:** Dejar esquema de regla, compatibilidad, criterio visible y API de listado/restauración; registrar límites de normalización, no ampliar a patrones generales.

<a id="uxr-5-2"></a>

- [x] **UXR-5.2 — Agrupar auxiliares sin ocultarlos automáticamente.**

  **Entrega:** [señales, incertidumbre y correcciones](docs/audits/2026-10-18-uxr-5-2.md), ADR-038. Grupo plegado con origen, clasificación orientativa corregible con alcance exacto; sin ocultar, ignorar ni parar. UI 59/59 y controles completos verdes.

  **Dependencias:** UXR-5.1 y UXR-2.3. **Plan:** [grupo UXR-5](docs/audits/2026-10-17-ui-ux-plan.md#uxr-5). **Contexto:** UX-07; D10; ADR-033.

  **Leer código:** `ui/src/views/OrphansView.tsx`, `ui/src/model.ts`, `crates/devports-core/src/{engine/views,process,config}.rs`, contratos de vistas y reglas de UXR-5.1.

  **Trabajo de esta sesión:** Grupo Posibles herramientas auxiliares plegado inicialmente, origen y detalle reutilizables, elementos sin evidencia como sin clasificar. Implementar corrección de clasificación con alcance explícito separado de ignorar; conservar acceso y acciones seguras. No sustituir las prohibiciones de parada por una heurística.

  **Hecho cuando:** Homebrew/VS Code pueden reconocerse sin leer argv completo; una clasificación equivocada se corrige. Plegar/clasificar no excluye definitivamente, no adopta y no detiene; elementos siguen localizables y contadores comprensibles.

  **Verificación:** Modelo/UI con auxiliares conocidos, servidor de proyecto, evidencia insuficiente, corrección y reapertura; verificar comportamiento con fixture real anonimizado y sin parar auxiliares reales.

  **Entrega a la próxima sesión:** Documentar señales usadas, incertidumbre y almacenamiento/alcance de correcciones para diagnóstico y futuras regresiones.

<a id="uxr-6-1"></a>

- [x] **UXR-6.1 — Editar carpetas de exploración dentro del panel.**

  **Entrega:** [flujo, reglas y límites](docs/audits/2026-10-18-uxr-6-1.md). Engranaje de Configuración con carpetas validadas, efecto revisado, guardado con recarga inmediata y borrador recuperable; UI 64/64 y controles completos verdes.

  **Dependencias:** UXR-1.3 y UXR-3.1. **Plan:** [grupo UXR-6](docs/audits/2026-10-17-ui-ux-plan.md#uxr-6). **Contexto:** Parte de UX-13; D13; ADR-035.

  **Leer código:** `crates/devports-core/src/{config,catalog,discovery,runtime}.rs`, `ui/src/{App,dialogs}.tsx`, `ui/src/{api,text}.ts`; edición y borradores de UXR-3.1/1.2.

  **Trabajo de esta sesión:** Añadir acceso discreto a Configuración con lista de raíces, añadir/quitar y Guardar/Cancelar. Validar rutas y conflictos preservando claves avanzadas/comentarios existentes; comunicar el efecto sobre descubrimiento. Recargar configuración en el núcleo, no solo reflejarla en React.

  **Hecho cuando:** Una raíz nueva descubre un proyecto de prueba sin reiniciar; quitarla no borra archivos ni detiene procesos. Error conserva entradas/configuración previa y reintento. Vencimientos/límites avanzados no se editan visualmente en esta entrega.

  **Verificación:** Config con raíces válidas/inexistentes/duplicadas, YAML desconocido/no editable, conflicto y permisos; UI de cancelar/borrador y runtime con recarga real.

  **Entrega a la próxima sesión:** Dejar operación de configuración, resultados de validación y acceso de navegación para integrar diagnóstico en UXR-6.2.

<a id="uxr-6-2"></a>

- [x] **UXR-6.2 — Recuperar conexión y mostrar diagnóstico accionable.**

  **Entrega:** [máquina de conexión, límites y estados](docs/audits/2026-10-18-uxr-6-2.md). Reintentos acotados con Reintentar, datos y contexto conservados, Docker y avisos separados y accionables; UI 77/77 y controles completos verdes.

  **Dependencias:** UXR-6.1. **Plan:** [grupo UXR-6](docs/audits/2026-10-17-ui-ux-plan.md#uxr-6). **Contexto:** Resto de UX-13; D13; ADR-035.

  **Leer código:** `ui/src/hooks.ts`, `ui/src/App.tsx`, `ui/src/views/SharedView.tsx`, `crates/devports-app/src/{diagnostics,host,commands}.rs`, `crates/devports-core/src/runtime.rs`.

  **Trabajo de esta sesión:** Estados separados para conectar, núcleo inaccesible, Docker inaccesible y errores de manifiestos/configuración. Reintentos acotados con Reintentar manual; evitar suscripciones duplicadas/snapshots desordenados. Mantener borradores/contexto y explicar siguiente acción sin exponer detalles sensibles innecesarios.

  **Hecho cuando:** Un fallo transitorio se recupera sin reiniciar app ni reenviar operaciones confirmadas; Docker caído no convierte contenedores a detenidos. Diagnóstico no sustituye todos los flujos por un error sin salida.

  **Verificación:** Hooks/contratos con rechazo inicial, recuperación, reconexión repetida, desmontaje y snapshot viejo; UI de error y acción. Caso de Docker inaccesible simulado sin apagar Docker del usuario.

  **Entrega a la próxima sesión:** Documentar máquina de conexión, límite de reintentos y estados públicos para UXR-7.2; mantener intactos contratos de resultado de operaciones.

<a id="uxr-6-3"></a>

- [x] **UXR-6.3 — Hacer exacta y recuperable la actividad.**

  **Entrega:** [contrato, conteo y límites](docs/audits/2026-10-18-uxr-6-3.md). Errores de registro tipados, temas tipados, ventana de 24 h explícita y estados cargando/error/vacío distintos; UI 84/84 y controles completos verdes.

  **Dependencias:** UXR-1.3. **Plan:** [grupo UXR-6](docs/audits/2026-10-17-ui-ux-plan.md#uxr-6). **Contexto:** UX-10; contrato de errores y exactitud de la auditoría.

  **Leer código:** `ui/src/views/ActivityView.tsx`, `ui/src/{text,api,types}.ts`, `crates/devports-core/src/{engine/actions,runtime,store,messages}.rs`, `crates/devports-app/src/commands.rs`; consumidores CLI del contrato log si cambia.

  **Trabajo de esta sesión:** Separar carga/error/vacío; no convertir fallos de DB/canal en lista vacía. Ventana temporal explícita coherente con consulta, conteos sobre eventos visibles y filtro, tipos de evento/campos en vez de buscar Temporal en texto. Traducir etiquetas de eventos conocidas sin perder fallback comprensible; conservar compatibilidad del CLI cuando corresponda.

  **Hecho cuando:** Core_started oculto no suma a un total presentado como visible; filtro explica subtotal/total. Fallo de almacenamiento no dice Sin actividad, ofrece reintento. Cambiar sección conserva filtro según UXR-1.2 y no provoca peticiones o rechazos sin gestionar.

  **Verificación:** Motor/contrato con almacenamiento fallido y canal cerrado; UI con carga, error, vacío real, startup oculto, cada filtro y ventana temporal; regresión de CLI log si su contrato se toca.

  **Entrega a la próxima sesión:** Registrar semántica de conteo/ventana y errores; actualizar tipos generados y documentación del contrato afectado.

<a id="uxr-7-1"></a>

- [ ] **UXR-7.1 — Cerrar accesibilidad y densidad del panel.**

  **Dependencias:** UXR-2.3, UXR-3.2, UXR-4.2, UXR-5.2, UXR-6.2 y UXR-6.3. **Plan:** [grupo UXR-7](docs/audits/2026-10-17-ui-ux-plan.md#uxr-7). **Contexto:** UX-14; integración de todos los nuevos flujos.

  **Leer código:** `ui/src/{App,dialogs}.tsx`, `ui/src/styles.css`, todas las vistas, tokens existentes y `docs/UI-REFERENCIA.md`.

  **Trabajo de esta sesión:** Revisar tabs con flechas/roles/asociación al panel, foco en rutas nuevas, zoom, cadenas largas, scroll, mensajes de error y movimiento reducido. Mantener identidad visual compacta. Medir antes de atribuir costo a transition:width; no rediseñar por un aviso aislado del detector.

  **Hecho cuando:** Todos los recorridos operables con teclado a 380×520; errores y botones no quedan inaccesibles por overflow. Zoom/textos largos probados y limitaciones explicitadas. El panel sigue cerrando por clic fuera y no deja webview vivo.

  **Verificación:** UI semántica de tabs, foco y errores; revisión visual nativa/browser claramente etiquetada, contraste de pares usados, movimiento reducido y casos largos. Corregir aquí hallazgos de accesibilidad de flujos nuevos.

  **Entrega a la próxima sesión:** Dejar lista de pantallas/escenarios y evidencias para la validación nativa final, con cualquier limitación aún abierta.

<a id="uxr-7-2"></a>

- [ ] **UXR-7.2 — Validar recorridos reales y cerrar la auditoría técnica.**

  **Dependencias:** UXR-0 y todas las tareas UXR-1.1 a UXR-7.1. **Plan:** [grupo UXR-7](docs/audits/2026-10-17-ui-ux-plan.md#uxr-7). **Contexto:** UX-01 a UX-14; D1–D14; ADR-028–035.

  **Leer código:** Auditoría original y evidencia por tarea en `ARCHIVES.md`, `docs/PRUEBAS.md`, `tools/{check,docker-checks,launchd-check,measure}.sh`, `README.md`, `SNAPSHOT.md`.

  **Trabajo de esta sesión:** Ejecutar los recorridos nativos del plan: detener sin adoptar; adoptar/revisar/guardar; declarar recurso y asociar dos proyectos; parar apps de uno dejando datos/compartidos activos; pedir parada independiente, ver afectados y cancelar. Verificar YAML/DB/snapshot reales, no solo mocks. Medir reposo con núcleo y panel cerrado tras usarlo; limpiar recursos propios.

  **Hecho cuando:** Evidencia por los 14 hallazgos y suites pertinentes verdes; ningún dato/compartido se para en cascada; estado/operaciones se recuperan con panel efímero. Documentar pendientes humanos G1/G2/M-15 sin marcarlos aprobados. README/arquitectura/manuales reflejan cambios finales y fuentes de verdad no divergen.

  **Verificación:** `tools/check.sh full`; `pnpm --dir ui run typecheck`, `lint`, `format:check`, `test` y `build`; comprobaciones macOS/Docker pertinentes con recursos propios, memoria/CPU y regresión del ciclo del panel. Si Node 20 falla con pnpm 11, usar Node 22 disponible sin cambiar configuración global.

  **Entrega a la próxima sesión:** Publicar informe final enlazado desde auditoría/TASKS/SNAPSHOT con comandos, resultados y rutas de evidencia. Dejar a David los días de uso y decisiones de G1/G2; no hacer release/push sin alcance autorizado.

### Inicio de una sesión dedicada

Indicar «Ejecuta UXR-3.1 de TASKS.md, siguiendo sus dependencias, contexto y registro de entrega» basta para elegir una unidad. Sustituir el ID por la tarea deseada; no es necesario recuperar este chat. La sesión lee la ficha y sus fuentes antes de actuar. El orden sugerido es el de las fichas, saltando solo tareas bloqueadas por evidencia externa; no ejecutar una tarea sin sus dependencias técnicas.

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

## H0 — Endurecimiento de los hallazgos de la Fase 0 (hecho)

- [x] **H0 — Resolver los hallazgos con soluciones definitivas.** Comparación de programas por familia; política de dependencias solo macOS; reloj inyectable y guarda del panel probada; esquinas redondeadas y panel sin destello con API pública; almacén web no persistente (49 a ~41 MB); `UNUserNotificationCenter` en lugar del plugin obsoleto; política de procesos corregida. Evidencia en `ARCHIVES.md`. Lo que **no** se puede cerrar sin una persona (anclaje bajo el ícono, aceptar el permiso y pulsar el botón) quedó como comprobación manual en `docs/verificacion-fase0.md`.

## Semana 0 — Línea base (HUMANO, en paralelo desde hoy)

- [x] **S0 — Medir la línea base.** *Sustituida por ADR-026:* la puerta G0 se omitió y el criterio (duplicados a la mitad, ≥3 aperturas al día) se evalúa en G1/G2 con la app en uso real; `tools/baseline.sh` sigue disponible si se quiere la cifra de «antes» (la sonda temporal `devports-probe` se retiró en T1.10).

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

- [x] **G0 — PUERTA FASE 0 (HUMANA, ADR-022). OMITIDA por decisión de David el 2026-10-17 (ADR-026): se construye de inmediato; el criterio (b) se evalúa en G1/G2 con la app en uso.** (a) V1, V2, V3, V9 favorables o Plan B aceptado; (b) tras una semana usando `devports ls`: duplicados simultáneos ≤ ½ de la línea base **y** ≥3 aperturas/día laborable (`opens.log`); (c) presupuesto de reposo medido. **Si (b) falla: parar y replantear. No continuar sin decisión escrita de David.**

---

## Fase 1 — Núcleo y CLI sin interfaz (puerta G1)

- [x] **T1.1 — `devports-proto`, servidor IPC, cliente y verificación de uid.** *Deps:* T0.9 (G0 omitida, ADR-026). *Lee:* ARQUITECTURA §10, CLI §8, NF-04.
  Tipos serde versionados (`v:1`); socket 0600 en dir 0700; `LOCAL_PEERCRED`; instancia única (socket vivo ⇒ salir, huérfano ⇒ borrar); cotas (línea ≤1 MiB, timeouts, máx. conexiones); ruta ≤104 bytes con respaldo; `examples/headless.rs`. Operaciones: `ping` primero.
  *Pruebas:* I-11, longitud de ruta, línea excesiva, JSON inválido, versión desconocida.
  → `feat(proto): add ipc protocol types` · `feat(core): add unix socket server with peer credential check` · `feat(cli): add ping client`

- [x] **T1.2 — Parsers y validación de `.devports.yml`, `shared.yml`, `config.yml`.** *Deps:* T1.1. *Lee:* MANIFIESTO §1,2,6,9.
  Errores con número de línea, advertencias (clave `test` ⇒ mensaje ADR-010), valores por defecto de `config.yml`. T0.3 ya dejó `manifest::parse/load` (YAML con `serde-saphyr`, versión 1) y los tipos `Manifest`/`ServiceSpec`: T1.2 añade el validador encima (formatos, unicidad, uno de `cwd` o `docker`, advertencias, números de línea con `Spanned`, y **`cwd` que salga de la carpeta del proyecto = error**) y `shared.yml`/`config.yml`. **Decidir y fijar el método de escritura que preserva comentarios** (editor de YAML con CST) validado contra I-15.
  *Pruebas:* U-03 completa (cada error/advertencia), versión mayor rechazada.
  → `feat(core): parse and validate manifest, shared and config files`

- [x] **T1.3 — Descubrimiento con observador de archivos.** *Deps:* T1.2, T0.3. RF-01. Recarga sin reiniciar, respaldo de raíz git (ADR-001), `import_ops_registry`. *Pruebas:* I-14.
  → `feat(core): watch roots and reload manifests on change`

- [x] **T1.4 — SQLite: migraciones, instancias, eventos, retención.** *Deps:* T1.1. *Lee:* ARQUITECTURA §9 (sin `approvals`, ADR-023). WAL, un escritor, `schema_meta`, retención 30 días, tabla de «ignorados» **no** (se define en T2.0). 
  *Pruebas:* migración desde vacío, `CHECK` de estados, retención, reapertura tras corte.
  → `feat(core): add sqlite store with migrations and event retention`

- [x] **T1.5 — Observación, reconciliación, identidad y Principal de hecho.** *Deps:* T1.3, T1.4, T0.4. RF-02, RF-04, RF-21; ARQUITECTURA §4-5.
  Actor del núcleo, diferencias aparece/desaparece/cambia puerto, `(pid,start_time)`. *Pruebas:* U-05, PT-02, PT-03.
  → `feat(core): reconcile observed processes into identified instances`

- [x] **T1.6 — Registro, vínculo por puerto reservado y máquina de estados.** *Deps:* T1.5. RF-06; ARQUITECTURA §6-7.
  Reservas, vínculo contenedor/fuerte/medio/ninguno (la ascendencia ya existe: `process::ancestry`, T0.8), `released`, `port_stolen`. *Pruebas:* U-10, I-04, I-05, I-13.
  → `feat(core): add port reservation and instance binding rules`

- [x] **T1.7 — Política completa de `run`.** *Deps:* T1.6. RF-07, RF-17; CLI §3.
  Tabla de decisión (códigos 0,10,11,12,13,14,16,18,19; el 15 queda **sin asignar**), mensajes texto y `--json` con *golden files*, `--reuse`, modo interactivo `DEVPORTS_INTERACTIVE=1`, falla abierto + `open -g -b com.davidcardenas.devports` (módulo del CLI con `#[allow]` justificado para `Command`; el mismo módulo hace el `exec`), sustitución `{PORT}`/`${PORT}`, `sh -c 'exec …'`, variables de entorno. 
  *Pruebas:* U-04, U-11, I-01 (caos 7×), I-09, I-13.
  → `feat(core): decide run requests with deterministic conflict policy` · `feat(cli): add run with exec, reuse and fail-open` · `test: add golden files for run output`

- [x] **T1.8 — Temporales: puertos, tokens, tope, vencimientos persistentes, parada segura.** *Deps:* T1.7. RF-08, RF-09, NF-06; ARQUITECTURA §8.
  Asignador de puertos, token 128 bits (solo hash), tope 2, TTL/extend ≤4 h, aviso T-60 una vez, parada (SIGTERM→5 s→SIGKILL→2 s; nunca grupos ajenos; prohibiciones absolutas), `expired_alive`, ejecución inmediata de vencidos al arrancar. **Bloqueante de seguridad.**
  *Pruebas:* U-06, U-07, U-08, U-09, PT-03, PT-04, I-03, I-06, I-07, I-10.
  → `feat(core): add temporary instances with persistent leases` · `feat(core): add safe process tree termination`

- [x] **T1.9 — Docker en el núcleo: eventos, vínculo por contenedor, `stop`.** *Deps:* T1.8, T0.4. RF-03.
  Reintento con espera creciente («desconocido», no «caído»); **suscribirse a eventos antes de listar y tratar los eventos como pistas** (T0.4: pueden repetirse, ver ADR-025); añadir `stop` al trait `ContainerSource` (hoy solo `list` y `events`); `tools/docker-checks.sh` ejecuta `cargo test -p devports-core --test docker_engine -- --ignored` además de D-02/D-04; `docker stop` 10 s; nunca `rm`; `kind: data` jamás se detiene solo. Escribir `tools/docker-checks.sh` (D-01..D-05 en la Mac). *Pruebas:* D-01..D-04 simuladas.
  → `feat(core): bind and stop docker containers for temporary instances` · `test(tools): add docker checks script`

- [x] **T1.10 — `ls`, `stop`, `extend`, `log`, `adopt` (propuesta).** *Deps:* T1.8. RF-05, RF-11, RF-15; CLI §4-7.
  Los tokens nunca salen en `ls`/`log`. `adopt` solo lectura. `adopt` debe generar `match` sin depender de `argv[0]` (ver MANIFIESTO §3, precisiones). **Retirar `devports-probe` (módulo `probe` y binario) y su entrada de CHANGELOG** (ADR-025, punto 8). *Pruebas:* I-08, I-15 (propuesta), U-08.
  → `feat(cli): add ls, stop, extend, log and adopt commands`

- [x] **T1.11 — Salud (health checks) y estado «colgada».** *Deps:* T1.6. *Lee:* MANIFIESTO §5. (Sustituye al `--replace` eliminado.)
  TCP/HTTP <1 s, cada 5 s, 3 fallos ⇒ no sana; alimenta código 11 y `healthy` en `ls`. *Pruebas:* servidor que deja de responder; HTTP 200–399; reloj falso.
  → `feat(core): add tcp and http health checks with hung state`

- [x] **T1.12 — Duplicados sin registrar, Olvidados, atribución e instancia vieja.** *Deps:* T1.5, T1.8. RF-10, RF-19, RF-20; ADR-004.
  Detección ≤3 s, `expired_alive` en Olvidados, cadena de ancestros con `known_agents` (lo desconocido, crudo; ninguna acción depende), rama/worktree leyendo `.git/HEAD` y `logs/HEAD` sin lanzar `git`. *Pruebas:* I-02, I-16, I-17.
  → `feat(core): detect unregistered duplicates and forgotten instances` · `feat(core): add launcher attribution and stale instance signals`

- [x] **T1.13 — Suite completa y CI macOS.** *Deps:* T1.1–T1.12. Cierre de PRUEBAS.md: U-01..U-11, PT-01..PT-04, I-01..I-17, D simuladas; matriz requisito→prueba verificada (script que falla si un RF no tiene prueba); `quality.yml` en macOS.
  → `test: complete unit, property and integration suites` · `ci(quality): run macos integration suite`

- [ ] **G1 — PUERTA FASE 1 (HUMANA).** Todas las pruebas en verde en CI (Linux+macOS) + `docker-checks.sh` en la Mac + **3 días de uso real sin parada indebida** + E1–E7 del PRD en la Mac + **revisión del portafolio por escrito** (ROADMAP §6: ¿resolvió el dolor?, ¿qué proyecto se pausa?, ¿cabe la estimación?). Decidir también nombre/licencia si procede (ADR-020).

---

## Fase 2 — App de barra (puerta G2)

- [x] **T2.0 — Cerrar los puntos abiertos de UI y extraer el sistema visual.** Decidir también las **esquinas redondeadas de 14 px** del panel: una ventana sin decoraciones es de esquinas rectas; redondearlas exige ventana transparente (API privada de macOS en Tauri) o aceptarlas rectas (T0.5). *Deps:* G1. *Lee:* UI-REFERENCIA §6-7 y capturas. 
  Proponer a David y registrar (ADR-027): persistencia de «Ignorar» (tabla `ignored` en SQLite, por `cwd+match`), «Crear proyecto nuevo», «Marcar como legítimo», estados vacíos/error (sin proyectos, Docker apagado, núcleo reconectando), orden de tabulación/foco/VoiceOver, filtro «Alertas», modo claro (diferido). Extraer tokens y estructura de `brag-output/*/composition/index.html` a `docs/ui-referencia/tokens.md`.
  *Hecho cuando:* David confirma cada punto; los PNG existentes cubren los estados diseñados.
  → `docs(ui): record adr-026 ui decisions and design tokens`

- [x] **T2.1 — Shell Tauri que aloja el núcleo + tooling de `ui/`.** Eliminar el código de spike (`spike-autopilot`, `spike-notify`, `autopilot.rs`, `notify_spike.rs`, `attention.rs` de juguete y el `frontend/` estático); conservar y reutilizar `panel.rs` (guarda, alfa 0 hasta `panel_ready`, almacén no persistente), `window_style.rs`, `diagnostics.rs` y `clock`. *Deps:* T2.0. RF-14.
  `crates/devports-app` arranca el núcleo y el socket; `ui/` con Vite+React 19+TS estricto (`strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, cero `any`), ESLint, Prettier, Vitest + Testing Library; bindings generados con `tauri-specta` (`ui/src/bindings.ts` no se edita; CI verifica deriva); texto UI centralizado; el núcleo empuja instantáneas por eventos Tauri, React sin lógica de negocio. Ícono con contador.
  → `feat(app): host core in tauri shell with tray counter` · `feat(ui): scaffold react panel with generated bindings`

- [x] **T2.2 — Panel: Atención, Proyectos, Compartidos, Sin proyecto, Actividad.** *Deps:* T2.1. UI-REFERENCIA §3-4; capturas 01–18.
  Árbol proyecto→repo→servicio, tokens de color/tipografía, `prefers-reduced-motion`, accesibilidad (botones reales, foco visible). *Pruebas:* componentes con datos simulados (contador, cuentas regresivas, advertencia de dependientes).
  → varios `feat(ui): …` por pestaña

- [x] **T2.3 — Acciones.** *Deps:* T2.2. RF-05, RF-12, RF-16.
  Detener Principal (confirmación; `data`/compartido muestra dependientes), Extender, Terminar, Conservar, Marcar como legítimo, Adoptar y **escribir manifiesto preservando comentarios** (I-15), Copiar comando, Abrir URL, Detener los N. Estas operaciones existen **solo** en comandos Tauri, nunca en el socket (ADR-009; test que lo verifica).
  → `feat(app): add stop, extend, adopt and legitimize commands`

- [x] **T2.4 — Notificaciones.** Quitar la puerta `spike-notify` de `notifications.rs` (ya implementa `UNUserNotificationCenter`), añadir la deduplicación de 10 min por proyecto con el `Clock` inyectado y probarla con `ManualClock`. Mantener `NoBundle` como degradación explícita. *Deps:* T2.3, T0.6. Vencimiento T-60 s y duplicado agrupado (ventana 10 min por proyecto), solo esos dos casos (ADR-014); según V6, con botón o abriendo el panel enfocado. → `feat(app): add grouped system notifications`

- [x] **T2.5 — Vista de Actividad.** *Deps:* T2.2. RF-15; filtros Todo/Rechazos/Temporales(+Alertas si T2.0 lo confirma). → `feat(ui): add activity view with filters`

- [x] **T2.6 — Autoarranque y supervisión.** Instalar el agente con `.plist` propio (`RunAtLoad`, `KeepAlive{SuccessfulExit:false}`, `ThrottleInterval` 5; el plugin de Tauri no escribe `KeepAlive`) o con `SMAppService`; repetir `tools/spike-launchd.sh` contra el binario real. *Deps:* T2.1, T0.7. RF-18; ARQUITECTURA §11. *Pruebas:* M-15. → `feat(app): add login launch and crash restart`

- [x] **T2.7 — Medición del presupuesto y palancas.** *Deps:* T2.1–T2.6. NF-01; M-14 (1 h en reposo). Ajustar intervalo/profundidad si no cumple; si el panel anclado es deficiente, decisión sobre SwiftUI **antes** de seguir. → `perf: tune observation within idle budget`

- [ ] **T2.8 — Release v0.1.0 (opcional, tras G2).** *Preparado:* `release.yml`, `scripts/validate-release.py` y `tools/bundle.sh` (sella el paquete); falta decidir la versión y ejecutar `/release` tras G2. **Sellar** el paquete con firma ad hoc (`codesign --force --deep --sign -`) y verificarlo (`codesign --verify --deep --strict`) antes de publicar: sin eso las notificaciones no funcionan (ADR-025). `release.yml` con bundle macOS sellado ad hoc (copiar de GitCanvas), `/release`. PR `dev → main` lo aprueba David.

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
- ADR-023: `--replace` fuera de v1 (código 15, U-12 e I-12 retirados, no se reutilizan). ADR-024: Principal de hecho y raíz `~/developer` confirmadas. ADR-025 (T0.1) redactado; ADR-026 (G0 omitida) registrada; ADR-027 (T2.0, decisiones de UI) pendiente.
- Abiertos: runtime de Docker/compose (T0.4); nombre y licencia (Fase 4).
- Hooks: tras clonar, ejecutar `bash scripts/install-git-hooks.sh` (copia `pre-commit`, `pre-push` y `commit-msg` a `.git/hooks`).
- `brag-output/` no se versiona; el marcado exacto de las vistas vive ahí hasta T2.0.
