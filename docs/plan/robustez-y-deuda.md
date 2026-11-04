# Plan — Robustez y deuda técnica (ROB)

> Fuente de diseño de las tareas ROB-1 a ROB-6 de [TASKS.md](../../TASKS.md#robustez-y-deuda). Origen: auditoría del 2026-11-04 pedida por David tras corregir el reconocimiento de servidores (`localhost` ≠ `127.0.0.1`). David aprobó **todos** los hallazgos. Si este plan y `docs/DECISIONES.md` se contradicen, manda `DECISIONES.md`.

## Objetivo
Una app sin deuda conocida: archivos del tamaño de una responsabilidad, lints acotados y con motivo, pruebas rápidas y con casos reales, controles cómodos de pulsar, y herramientas para diagnosticar sin leer código. **Sin cambiar comportamiento** salvo donde la tarea lo diga (marcado como `feat`/`fix`).

## Reglas para ejecutar sin gastar contexto (leer primero)
1. **Una tarea = una sesión.** Basta con «Ejecuta ROB-x.y de TASKS.md». No recuperar este chat: todo lo necesario está en este plan y en la ficha.
2. **No leer archivos enteros.** Cada ficha lista *puntos de entrada* con línea aproximada (pueden haberse movido: usa `grep -n "fn nombre"` y lee 60–120 líneas alrededor). Archivos grandes (`operations.rs`, `runtime.rs`, `App.tsx`, `App.test.tsx`, `engine/tests.rs`): nunca `Read` completo.
3. **Refactors = movimiento puro.** Antes: `cargo test --workspace` y `pnpm test` (anotar el número de pruebas). Después: mismo número, mismos nombres, sin editar aserciones. Un refactor que cambia una aserción no es un refactor: detenerse y registrar por qué.
4. **Una pasada de verificación al final**, no tras cada edición: `tools/check.sh full` y en `ui/`: `pnpm run typecheck && pnpm run lint && pnpm run format:check && pnpm test`. Durante el trabajo, solo la prueba concreta (`cargo test -p devports-core --lib nombre`, `pnpm vitest run archivo`).
5. **Sin subagentes salvo que la ficha lo pida.** Las tareas son pequeñas; buscar con `grep` basta.
6. Reglas fijas del repositorio (resumen; detalle en `CLAUDE.md`): rama `dev`, sin push, commits con `/commit-flow` (una línea, inglés, sin trailers), cero `unwrap/expect/panic/indexación`, el núcleo no depende de `tauri`, `bindings.ts` se regenera (`UPDATE_BINDINGS=1 cargo test -p devports-core --features bindings --test bindings`), texto nuevo en `es.ts` **y** `en.ts`, sin colores literales, sin `TODO` sueltos, cada prueba nueva con `// covers: ROB-x.y`.
7. **No tocar** `.devports.yml` de la raíz salvo ROB-1.3, ni los servicios reales de David (no detener, adoptar ni reiniciar nada propio: usar procesos de prueba).

## Hechos de la auditoría (no volver a medirlos)
| Hecho | Dato |
|---|---|
| Rust total | ~33.900 líneas; mayores: `engine/tests.rs` 2.544, `app/src/operations.rs` 1.227, `core/src/runtime.rs` 1.196, `engine/views.rs` 788, `store.rs` 770 |
| UI total | ~14.700 líneas; mayores: `App.test.tsx` 1.916, `bindings.ts` 1.117 (generado), `i18n/es.ts` 772, `en.ts` 752, `App.tsx` 737, `views/ProjectsView.tsx` 522, `AdoptSheet.tsx` 505, `hooks.ts` 501 |
| Archivo basura versionado | `crates/devports-core/src/engine/tests.rs-E` (residuo de `sed -i`; entró en `f5846a4`) |
| Lints silenciados a nivel módulo | `core/src/engine.rs:16` (`too_many_lines`, `needless_pass_by_value`, `unused_self`, `similar_names`, `assigning_clones`, `ref_option`); los demás `#[allow]` están acotados y justificados (unsafe, `print_*` del CLI/ejemplos, `disallowed_methods` de `Command::new`) |
| Pruebas UI | 17 entornos jsdom = 14 s de 25 s; vitest sugiere `isolate: false` o `pool: 'vmThreads'`; config en `ui/vite.config.ts` (bloque `test`) y `ui/vitest.setup.ts` |
| Controles pequeños (medidos en navegador a 800×600) | rueda de Configuración 26×26; pestañas de Actividad (Todo, Rechazos, Temporales, Alertas) 24 px de alto; resto ≥ 28 |
| `.devports.yml` raíz | sin versionar; `start` con ruta absoluta a `vite.js` de tu usuario |
| Comprobaciones al 2026-11-04 | `tools/check.sh full` y todo `pnpm` en verde; sin `TODO/FIXME` |
| Raíz de búsqueda por defecto | `~/developer`, profundidad 3 (`discovery.rs`: `default_roots`, `DEFAULT_MAX_DEPTH`) |

### La causa del bug de reconocimiento (ya corregido en `2be1d74`; contexto para ROB-5)
`fingerprint.rs` trataba `127.0.0.1:N` y `localhost:N` como tokens distintos, así que un servidor reiniciado con otra dirección local no coincidía con su servicio, quedaba «sin registrar», y «Adoptar» se bloqueaba con `PortTaken` porque el servicio declarado seguía dueño del puerto. Regla vigente (MANIFIESTO §3, regla 7): `localhost`, `127.0.0.1`, `0.0.0.0`, `::1`, `[::1]` son un solo token (`localhost`). Moraleja de diseño: **la identidad de un servicio no debe depender de cómo se escribió la dirección**, y **cualquier bloqueo debe poder explicarse**.

## Diseño por grupo

### ROB-1 — Limpieza y velocidad (sin riesgo)
Borrar el archivo basura y evitar que vuelva (`*-E` en `.gitignore`); acelerar vitest compartiendo jsdom entre archivos si las pruebas siguen aisladas por `cleanup` de Testing Library (verificar que no hay estado global entre archivos: orden aleatorio con `--sequence.shuffle`); decidir el destino de `.devports.yml` (recomendado: **ignorarlo** con una entrada comentada en `.gitignore`, porque describe el entorno del mantenedor y contiene rutas absolutas; alternativa descartada: hacerlo portable y versionarlo, porque `start` necesita la ruta de pnpm del usuario).

### ROB-2 — Lints acotados
Quitar el `#![allow(...)]` de `engine.rs` y reaparecer cada aviso. Criterio por aviso: *corregir* si es barato y mejora la lectura; si no, `#[allow(lint, reason = "…")]` **en la función** con motivo. `too_many_lines` es el más probable: preferir extraer pasos con nombre a silenciar. Meta: ningún `allow` a nivel de módulo en `core` salvo `unsafe` en `platform/macos.rs`.

### ROB-3 — Partir archivos por responsabilidad (refactor puro)
Regla de corte: un archivo por razón para cambiar; objetivo orientativo ≤ 500 líneas de producción por archivo (las pruebas pueden vivir al lado en `tests/` del módulo).
- `app/src/operations.rs` → carpeta `operations/`: `types.rs` (peticiones, vistas, estados), `store.rs` (`OperationStore`, retry, agregación), `validate.rs`, `failure.rs` (`OperationFailure` y sus `From`), `runner.rs` (`spawn`, `consumer_*`), `tests.rs`. API pública idéntica (`pub use`).
- `core/src/runtime.rs` → mover a `runtime/`: `actor.rs` (`Actor` e impl), `handle.rs` (`CoreHandle`), `scan.rs` (`scan_host`), `docker_supervisor.rs`, `catalog_watch.rs`, `health.rs`; `runtime.rs` queda con docs de cabecera, `Core`, `Snapshot`, `CoreOptions`, `RuntimeError`, `Msg`. Ya existen `runtime/{adoption,folders,preferences,probe,rules,uses}.rs`: seguir ese estilo.
- `ui/src/App.tsx` + `hooks.ts`: extraer lo que no es composición (barra de pestañas, sección de operaciones, cada hook a su archivo `hooks/use*.ts` con su prueba). `App.tsx` queda como composición.
- `ui/src/App.test.tsx` → un archivo por `describe` (panel, núcleo inalcanzable, inglés, actividad, apariencia, razones) más un `test-utils.tsx` con `fakeBackend` y ayudantes.
- `core/src/engine/tests.rs` (2.544) → `engine/tests/` por tema (run, binding, leases, views, adopción, ignorar, escenarios) con un `mod.rs` que comparte los ayudantes.

### ROB-4 — UI/UX
- Zonas de pulsación ≥ 32 px en la rueda y en las pestañas segmentadas (el área, no necesariamente el dibujo: `min-height`/`padding`, sin cambiar la densidad visual del resto). Ver `DESIGN.md` para los tokens; sin estilos en línea.
- «Detener los N» (grupo de duplicados) nombra qué se detiene: la confirmación lista las instancias (puerto y referencia) y dice que se conserva la principal. Reutilizar el diálogo de confirmación existente (`dialogs.tsx`).
- Motivo siempre visible junto a un botón deshabilitado en la hoja de adopción (`AdoptSheet.tsx`, `choice()`): hoy depende de `destination.reason`; asegurar que **nunca** hay un botón deshabilitado sin frase, que la frase explica qué hacer (p. ej. `PortTaken` → «ese puerto ya es del servicio X de este proyecto») y que cubre ambos idiomas. Revisar `adoptionMessage` (`ui/src/adoption.ts`) por razones sin texto.
- Revisión de modo claro y de contraste con las pantallas reales: lo ejecuta David (accesibilidad con VoiceOver incluida); la tarea deja la lista de comprobación y las pruebas automáticas.

### ROB-5 — Producto y diagnóstico
- **5.1 Regresiones con manifiestos reales (primero: barato y el mayor retorno).** Fixtures anonimizadas de los manifiestos de `teatromuseo` (dos servicios con el mismo `match`/`cwd` y distinto puerto, `127.0.0.1` vs `localhost`) y pruebas de: reconocimiento, rol Principal/duplicado, y adopción (`PortTaken` solo cuando el puerto es de otro servicio *sin coincidencia*).
- **5.2 `devports doctor`.** Diagnóstico de solo lectura: raíces de búsqueda y profundidad, proyectos descubiertos y manifiestos inválidos, y, dado un puerto o PID, **por qué** no coincide con ningún servicio (huella observada vs `match`, `cwd` fuera del servicio, proyecto fuera de alcance). **Requiere ADR-044 antes de implementar** (ADR-009 y la regla 6: el socket crece solo con ADR; operación nueva de solo lectura `doctor`, sin tokens ni efectos).
- **5.3 Proyecto fuera de alcance, visible.** Un proceso huérfano cuyo `cwd` tiene un ancestro con `.devports.yml` que el catálogo no conoce (fuera de raíces o más profundo que `max_depth`) lo dice en su fila/detalle («hay un proyecto en esta carpeta que DevPorts no busca: añade la carpeta en Ajustes»), con un diagnóstico tipado en el núcleo (patrón de `reasons.rs`/`diagnostic.rs`) y texto en es/en. La detección corre en el escáner del runtime (el engine no toca disco).
- **5.4 Aviso de cambio de dirección (opcional, [P]).** Cuando una instancia vinculada a un servicio se observa con otra dirección local que la vez anterior, anotarlo en Actividad («`api` ahora escucha en `127.0.0.1:8192`, antes `localhost:8192`»). Solo informativo; sin notificación del sistema. **Validar con David antes de construir** (si el ruido supera la utilidad, se cierra sin hacer).

### ROB-6 — Cierre y puertas humanas
Documentar (`MAPA-DEL-CODIGO.md`: nuevos módulos), actualizar `SNAPSHOT.md`, registrar en `ARCHIVES.md`. G1 sigue siendo humana (3 días de uso real, E1–E7, revisión del portafolio): este plan no la sustituye.

## Orden y dependencias
```
ROB-1.1 ─┬─ ROB-1.2
ROB-1.3 ─┘
ROB-5.1 ──► ROB-2.1 ──► ROB-3.1 ─┐
                       ROB-3.2 ─┤ (3.x entre sí independientes, salvo 3.2 antes de 5.2/5.3)
                       ROB-3.3 ─┤
                       ROB-3.4 ─┤
                       ROB-3.5 ─┘
ROB-4.1, 4.2, 4.3  (independientes; tras 3.3 si tocan App.tsx)
ROB-5.2 (ADR-044) ──► ROB-5.3 ──► ROB-5.4 (opcional)
ROB-6.1 cierra
```
Paralelizables en sesiones distintas sin tocar los mismos archivos: ROB-1.x ∥ ROB-5.1; ROB-3.1 ∥ ROB-3.3 ∥ ROB-3.5 (carpetas distintas); ROB-4.x tras 3.3.

## Riesgos
| Riesgo | Mitigación |
|---|---|
| Un refactor cambia comportamiento sin querer | Misma cuenta de pruebas y mismos nombres; sin editar aserciones; `git diff -M --stat` debe mostrar movimientos |
| `isolate: false` en vitest filtra estado entre archivos | Probar con orden aleatorio 3 veces; si falla, `pool: 'vmThreads'` o dejarlo como está y registrarlo |
| Quitar el `allow` de `engine.rs` obliga a reescribir mucho | Acotar por función con `reason`; no reescribir lógica del motor por un estilo |
| `doctor` filtra información sensible por el socket | Solo rutas y huellas ya visibles en `ls --all`; sin tokens (regla 7); ADR-044 lo fija |
| 5.4 genera ruido | Es opcional y se valida con David antes |
