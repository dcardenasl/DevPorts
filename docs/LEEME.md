# DevPorts: empieza aquí

> Este paquete es la base para crear el proyecto en una sesión nueva. Nada de esto es código: es la especificación completa, definida con David el 2026-10-16.
> Nombre de trabajo "DevPorts"; el nombre definitivo se decide al terminar la Fase 1 (ADR-020).

## Qué es
App de barra de menús para macOS (Rust + Tauri v2 + React) con un CLI (`devports run`) que mantiene el orden de los servidores de desarrollo: qué corre, de qué proyecto es, cuál es el oficial (Principal) y cuáles son pruebas con vencimiento (Temporales). Resuelve que los agentes de IA levanten servidores duplicados sin saber que ya existe uno.

## Orden de lectura
1. `DECISIONES.md`: 22 decisiones confirmadas por David y puntos abiertos. **Fuente de verdad ante cualquier contradicción.**
2. `PRD.md`: requisitos RF-01 a RF-21 con criterios de aceptación.
3. `ARQUITECTURA.md`: diseño técnico, datos, algoritmos y verificaciones V1 a V12.
4. `MANIFIESTO.md`: esquemas YAML y algoritmos de identidad.
5. `CLI.md`: comandos, códigos de salida, mensajes.
6. `PRUEBAS.md`: estrategia y catálogo de pruebas.
7. `ROADMAP.md`: fases, tareas y puertas.

## Reglas para quien implemente
- Cada afirmación está marcada **[C]** (confirmada por David), **[D]** (derivada de lo confirmado) o **[P]** (propuesta a validar con mediciones). No cambies una **[C]** sin preguntar a David. Las **[D]** y **[P]** se pueden ajustar si lo justificas y lo dejas escrito en `DECISIONES.md`.
- Respeta lo que queda **fuera de la v1** (ADR-015): servidor MCP, adaptadores por agente, proxy de nombres estables, versión Pro, iniciar procesos gestionados, borrar contenedores o volúmenes, Linux y Windows.
- Principio de producto de David: simple pero robusto y sólido; no agregar funcionalidades que hoy no se necesitan.
- Lo específico de macOS no se pudo verificar al diseñar: se comprueba en la Fase 0 (tabla V1 a V12 de `ARQUITECTURA.md`) y los resultados van a `docs/verificacion-fase0.md`.
- Los puntos abiertos 1, 3 y 4 de `DECISIONES.md` están cerrados (ADR-023 y ADR-024). Queda el runtime de Docker, que se mide en la tarea T0.4. La cola de trabajo está en `TASKS.md` y las reglas del proyecto en `CLAUDE.md`.

## Primera sesión de construcción
Alcance: **Fase 0** de `ROADMAP.md`, empezando por la tarea T0.1 (workspace de Cargo con `devports-proto`, `devports-core`, `devports-cli`, `devports-app` y CI mínimo). No avances a la Fase 1 sin la puerta de la Fase 0.

## Línea base (antes de construir)
`tools/baseline.sh start` mide cuántos servidores simultáneos y duplicados hay cada día. Debe correr al menos 5 días de trabajo antes de evaluar la Fase 0 (`PRUEBAS.md` sección 5).

## Contexto de David
- Los proyectos viven bajo `/Users/davidcardenas/developer/`, organizados por tecnología (`PHP/`, `Node/`, `Tauri/`, etc.). Este proyecto va en `~/developer/Tauri/devports/` (la carpeta `Tauri/` ya contiene `GitCanvas` y `docs`). Raíz por defecto propuesta para el descubrimiento: `~/developer`.
- Usa un solo MySQL en Docker para todos sus desarrollos (ADR-013).
- Ya tiene un proyecto con el mismo stack Rust + Tauri + React: GitCanvas (carpeta `~/developer/Tauri/GitCanvas`). Es la referencia para estructura y CI.
- La fuente original de esta documentación es su VentureOS: `Documents/Work Workflow/VentureOS/assets/devports/`. Si cambian las decisiones, actualizar allí también.
