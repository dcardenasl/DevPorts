# DevPorts: Hoja de ruta

> Orden **[C]** (ADR-021): núcleo y CLI sin interfaz antes que la app de barra. Estimaciones **[P]**, sin medir.
> Cada fase termina con una **puerta**: si no se cumple, no se avanza.
> Fuente de verdad ante contradicciones: `DECISIONES.md`.

## Resumen
| Fase | Qué | Estimación **[P]** | Puerta |
|---|---|---|---|
| Semana 0 | Línea base con `tools/baseline.sh` | Pasiva, ya iniciable | Datos de una semana |
| 0 | Sondeo en la Mac: `devports ls` de solo lectura y verificaciones V1 a V12 | 3 a 4 días de construcción + 1 semana de uso | Criterio de éxito (ADR-022) |
| 1 | Núcleo + CLI sin interfaz | 2 a 2,5 semanas | Pruebas en verde + revisión del portafolio |
| 2 | App de barra | 1 a 1,5 semanas | Presupuesto cumplido + 5 días de uso |
| 3 | Integración con `ci4-kickstart` | 2 a 3 días | Proyecto CI4 nuevo nace integrado |
| 4 | Un mes de uso real y decisión de publicar | 4 semanas calendario | Señales definidas por David |

Construcción total estimada: 4 a 5 semanas de trabajo, por encima de las 2 semanas máximas sugeridas al registrar la idea. Por eso la puerta de la Fase 1 incluye decidir conscientemente si se continúa (ver §6).

---

## Semana 0: Línea base
- Ejecutar `./tools/baseline.sh start` en la Mac y trabajar normalmente.
- Al final: `./tools/baseline.sh report` y guardar la salida en `docs/linea-base-AAAA-MM-DD.txt`.
- **Hecho cuando:** hay al menos 5 días de trabajo registrados.

## Fase 0: Sondeo
Objetivo: comprobar en la Mac de David lo que no se pudo verificar desde el entorno de diseño y medir si la visibilidad sola ya ayuda.

| Tarea | Detalle | Verifica |
|---|---|---|
| T0.1 | Crear el workspace de Cargo (`devports-proto`, `devports-core`, `devports-cli`, `devports-app`) y CI mínimo | n/a |
| T0.2 | `ProcessSource` de macOS (libproc) y `devports ls` de solo lectura, **sin núcleo**, que registra cada invocación en `~/.devports-baseline/opens.log` | V1, V2, V3 |
| T0.3 | Descubrimiento de manifiestos bajo `~/developer` y mapeo proceso a proyecto | V11 |
| T0.4 | Docker: `docker context inspect`, lista y eventos; asignación por etiquetas de compose | V4, V12 |
| T0.5 | Spike de Tauri: ícono con contador, panel anclado y efímero, política accessory, medición en reposo | V5, V10 |
| T0.6 | Spike de notificación con botón de acción | V6 |
| T0.7 | Spike de supervisión: launchd `KeepAlive` con la app de Tauri; autoarranque del CLI con `open -g -a` | V7, V8 |
| T0.8 | Spike de `exec`: confirmar que conserva el PID y la ascendencia | V9 |
| T0.9 | Informe `docs/verificacion-fase0.md` con la tabla V1 a V12 (resultado y decisión) | todas |

**Puerta (ADR-022):** (a) V1, V2, V3 y V9 resueltas favorablemente o con plan B aceptado; (b) tras una semana usando `devports ls`, los duplicados simultáneos bajan al menos a la mitad frente a la línea base **y** se abre 3 o más veces por día; (c) presupuesto de reposo medido. Si (b) falla: parar y replantear.

## Fase 1: Núcleo y CLI sin interfaz
Todo el comportamiento, probado sin pantalla (`examples/headless.rs`).

| Tarea | Detalle | Requisitos |
|---|---|---|
| T1.1 | `devports-proto` + servidor IPC + cliente CLI + verificación de uid | NF-04 |
| T1.2 | Parser y validación de `.devports.yml`, `shared.yml`, `config.yml` | MANIFIESTO |
| T1.3 | Descubrimiento de proyectos con observador de archivos | RF-01 |
| T1.4 | SQLite: migraciones, instancias, eventos, retención | NF-09 |
| T1.5 | Observación, reconciliación, identidad y `match`, Principal de hecho | RF-02, RF-04, RF-21 |
| T1.6 | Registro, vínculo por puerto reservado y máquina de estados | RF-06 |
| T1.7 | Política completa de `run`: tabla, códigos, mensajes texto y JSON, `--reuse`, modo interactivo, falla abierto | RF-07, RF-17 |
| T1.8 | Temporales: asignación de puertos, tokens, tope, vencimientos persistentes, parada segura | RF-08, RF-09, NF-06 |
| T1.9 | Docker: eventos, vínculo por contenedor, `stop` | RF-03 |
| T1.10 | `ls`, `stop`, `extend`, `log`, `adopt` (propuesta) | RF-05, RF-11, RF-15 |
| T1.11 | Salud (TCP/HTTP, estado "colgada"); sustituye al `--replace` eliminado por ADR-023 | MANIFIESTO §5 |
| T1.12 | Detección de duplicados sin registrar, Olvidados, atribución, señales de instancia vieja | RF-10, RF-19, RF-20 |
| T1.13 | Suite de pruebas de PRUEBAS.md, incluida la de caos | todos |

**Puerta:** todas las pruebas de integración en verde (I-01 a I-17, sin I-12, más las de Docker simuladas); 3 días de uso real en la Mac de David sin ninguna parada indebida; **revisión del portafolio** (§6).

## Fase 2: App de barra
| Tarea | Detalle | Requisitos |
|---|---|---|
| T2.1 | Shell de Tauri que aloja el núcleo; ícono con contador | RF-14 |
| T2.2 | Panel React: Atención, Proyectos, Compartidos, Sin proyecto, Actividad | ADR-014 |
| T2.3 | Acciones: Detener (con advertencia de dependientes), Extender, Terminar, Conservar, Adoptar y escribir manifiesto, Marcar como legítimo, Copiar comando, Abrir URL | RF-05, RF-12, RF-16 |
| T2.4 | Notificaciones (vencimiento y duplicado agrupado) | RF-14 |
| T2.5 | Vista de actividad | RF-15 |
| T2.6 | Autoarranque y supervisión (`KeepAlive`) | RF-18 |
| T2.7 | Medición del presupuesto y ajuste de palancas | NF-01 |

**Puerta:** presupuesto cumplido; 5 días de uso diario sin cuelgues; si el panel anclado resulta deficiente, decidir sobre SwiftUI antes de seguir.

## Fase 3: Integración con el ecosistema
| Tarea | Detalle |
|---|---|
| T3.1 | `ci4-kickstart` genera `.devports.yml` en cada proyecto nuevo |
| T3.2 | `start-all.sh` generado con `devports run --reuse` (CLI §10) |
| T3.3 | Inserción del bloque de `AGENTS.md` y `CLAUDE.md` (CLI §9) |
| T3.4 | Alta del MySQL único en `shared.yml` y `uses` en los proyectos |
| T3.5 | Opcional: `import_ops_registry` con `~/.ci4-ops/projects.yml` |

**Puerta:** un proyecto CI4 nuevo nace integrado y pasa un escenario E2 y E3 de punta a punta.

## Fase 4: Un mes de uso y decisión de publicar **[C]** (ADR-020)
- Usar la herramienta un mes en el trabajo real.
- Antes de publicar, David **define por escrito las señales externas** que activarían un venture (por ejemplo: equipos que piden manifiestos compartidos, personas que piden pagar). No inventarlas después.
- Decidir: licencia, nombre definitivo (verificar que esté libre como comando, repo y dominio) y publicación como GitCanvas (gratis, apoyo voluntario).

---

## 6. Puerta de revisión del portafolio (al terminar la Fase 1)
El INDICE de VentureOS muestra proyectos parados (Kosten sin commits desde 2026-09-22, Proofy y FILMA sin sesión desde mayo). Antes de entrar en la Fase 2, responder por escrito:
1. ¿La Fase 0 y la Fase 1 ya resolvieron el dolor diario (medido contra la línea base)? Si sí, la Fase 2 puede esperar.
2. ¿Qué proyecto activo se pausa para dar tiempo a la Fase 2?
3. ¿La estimación restante sigue cabiendo?

## 7. Dependencias y riesgos de calendario
- Todo lo que no se pudo probar en el entorno de diseño (V1 a V12) está en la Fase 0; un resultado adverso puede mover decisiones (Plan B de observación, Swift para la envoltura).
- La decisión sobre `--replace` quedó cerrada por ADR-023 (eliminado de v1).
- La Fase 3 depende de que `ci4-kickstart` y CI4 Ops Platform (ver `assets/ci4-ops-platform/`) estén en un estado probado; hoy su Fase 1 está implementada pero pendiente de prueba de punta a punta.
