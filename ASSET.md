# DevPorts (nombre de trabajo)

> App de barra de menús para macOS que mantiene el orden de los servidores de desarrollo: qué corre, de qué proyecto es, cuál es el oficial y cuáles son pruebas con vencimiento. Incluye un CLI (`devports run`) para que los agentes de IA no dupliquen servidores.

---

## ⚡ Retomar en 30 segundos

| Campo | Valor |
|---|---|
| **Versión actual** | v0.0.0 local; v0.1 en pruebas, sin release |
| **Estado** | Núcleo, CLI y app construidos; correcciones UXR en curso, G1/G2 pendientes |
| **Última sesión** | 2026-10-17 |
| **¿Dónde quedé?** | UXR-1 y UXR-2 implementados; detalles/estados reales y parada puntual por proyecto verificados. Estado y evidencia vigentes en [SNAPSHOT.md](SNAPSHOT.md) y [ARCHIVES.md](ARCHIVES.md). |
| **Próximo paso concreto** | UXR-3.1 (adopción elegible y edición validada). UXR-0 sigue abierta por timeout de acceso nativo; cierre nativo pendiente en UXR-7.2. |
| **Bloqueante activo** | Ninguno |

---

## Ficha del asset

| Campo | Valor |
|---|---|
| **Slug** | devports |
| **Tipo** | Herramienta de escritorio (app de barra) + CLI |
| **Visibilidad** | Repositorio local privado, sin remoto |
| **Repositorio** | Checkout local `devports`, rama `dev`; remoto pendiente de autorización |
| **Licencia** | {{PENDIENTE}} (se decide al terminar la Fase 1, ADR-020) |
| **Nombre definitivo** | {{PENDIENTE}} (candidatos: Faro, Portero, Muelle; DevPorts es de trabajo) |
| **Inicio** | 2026-10-16 |
| **Stack** | Rust (núcleo y CLI), Tauri v2 (envoltura), React + TypeScript (panel), SQLite |
| **Plataforma** | Solo macOS en v1 |

---

## Qué hace / Qué resuelve

**Problema que resuelve:**
David trabaja con varios proyectos a la vez en una misma máquina. Los agentes de IA levantan servidores nuevos sin saber que ya existe uno (hasta 7 simultáneos en un día, desde 1) y se termina probando contra el servidor equivocado. Berth y Bosun solo observan; ninguno distingue un duplicado por error de una prueba legítima.

**Cómo funciona (en 5 pasos):**
1. Un manifiesto `.devports.yml` por proyecto declara sus servicios; nace de lo observado ("adoptar").
2. El núcleo observa procesos y contenedores y los asocia a proyecto y servicio por identidad, no por puerto.
3. `devports run <servicio>` registra y ejecuta el servidor; si ya existe uno oficial, rechaza con su URL.
4. Para pruebas, `--as test --reason "..."` crea un Temporal con puerto libre y vencimiento; se detiene solo.
5. La barra muestra estado pasivo, un contador de lo que requiere atención y un registro de actividad.

**Casos de uso actuales:**
- Uso propio de David en sus proyectos CI4 y de clientes.

---

## Proyectos que lo usan

| Proyecto | Tipo | Versión usada | Notas |
|---|---|---|---|
| (ninguno todavía) | n/a | n/a | La integración con `ci4-kickstart` es la Fase 3 |

---

## Estado del build

| Componente | Estado | Descripción |
|---|---|---|
| Documentación de implementación | ✅ | `docs/`: PRD, ARQUITECTURA, MANIFIESTO, CLI, ROADMAP, PRUEBAS, DECISIONES |
| Script de línea base | Histórico | Sustituido por uso real (ADR-026); no es requisito vigente |
| Núcleo (`devports-core`) | ✅ | Construido y probado; G1 humana pendiente |
| CLI (`devports-cli`) | ✅ | Construido y probado con permisos acotados al protocolo |
| App de barra (`devports-app`, `ui/`) | En pruebas | Construida; correcciones UXR y G2 pendientes |
| Integración `ci4-kickstart` | ⏳ | Fase 3 |

---

## Roadmap

### Próximas mejoras (corto plazo)
La cola vigente es [TASKS.md](TASKS.md): UXR-3.1 es la siguiente tarea de implementación. UXR-0/UXR-7.2 conservan la verificación nativa pendiente; G1/G2 requieren uso real y decisión humana. `--replace` permanece fuera de v1 (ADR-023).

### Ideas para el futuro (largo plazo)
- Servidor MCP, adaptadores por agente, proxy de nombres estables `*.localhost`, Linux, equipos con manifiestos compartidos (todo fuera de la v1, ADR-015)

Detalle completo en [docs/ROADMAP.md](docs/ROADMAP.md).

---

## Historial de versiones

| Versión | Fecha | Cambios principales |
|---|---|---|
| v0.0 | 2026-10-16 | Especificación completa, sin código |

---

## Documentación

| Recurso | Estado | Ruta |
|---|---|---|
| Decisiones (fuente de verdad) | ✅ | [docs/DECISIONES.md](docs/DECISIONES.md) |
| PRD | ✅ | [docs/PRD.md](docs/PRD.md) |
| Arquitectura | ✅ | [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md) |
| Manifiesto y configuración | ✅ | [docs/MANIFIESTO.md](docs/MANIFIESTO.md) |
| CLI | ✅ | [docs/CLI.md](docs/CLI.md) |
| Hoja de ruta | ✅ | [docs/ROADMAP.md](docs/ROADMAP.md) |
| Pruebas | ✅ | [docs/PRUEBAS.md](docs/PRUEBAS.md) |
| Referencia visual de la interfaz (capturas, vistas, flujos, tokens) | ✅ | [docs/UI-REFERENCIA.md](docs/UI-REFERENCIA.md) |
| Ficha de la idea original (histórico) | ✅ | [../../ideas/2026-10-16-devports-orden-de-servidores.md](../../ideas/2026-10-16-devports-orden-de-servidores.md) |

Orden de lectura para quien implemente: DECISIONES, PRD, ARQUITECTURA, MANIFIESTO, CLI, PRUEBAS, ROADMAP.

---

## Adopción y visibilidad

| Métrica | Valor | Fecha |
|---|---|---|
| GitHub stars | n/a (privado) | 2026-10-16 |
| Proyectos propios que lo usan | 0 | 2026-10-16 |
| Proyectos externos que lo usan | 0 | 2026-10-16 |

---

## Potencial de monetización

| Modelo | Viabilidad | Notas |
|---|---|---|
| Setup como servicio | Baja | Herramienta personal; sin demanda observada |
| SaaS construido sobre el asset | Baja | Es una herramienta local; el valor está en el ciclo de vida para agentes |
| Documentación / curso premium | Baja | n/a |
| Templates pagos / extensiones | Baja | n/a |
| GitHub Sponsors / Open source | Media | Mismo modelo que GitCanvas, tras Fase 4 |

**Lectura estratégica:** la app de barra sola compite con Berth (gratis y open source). El diferencial vendible es el ciclo de vida Principal/Temporal para agentes y, más adelante, manifiestos compartidos para equipos. No hay evidencia de disposición a pagar.
**Decisión actual:** No monetizar. Evaluar en la Fase 4 con señales externas que David define por escrito antes de publicar (ADR-020). Si aparecen, convertir en venture con `nuevo venture`.

---

## Log de avances

- 2026-10-17: UXR-2.3 completa (`423321c`): detalle compartido, puertos observados y estados reales; UI 41/41, comprobación completa Rust y teclado de navegador a 380×520 verdes. Validación nativa pendiente.

- 2026-10-17: Núcleo, CLI y app construidos; UXR-1.1–1.3 y UXR-2.1/2.2 completas. Parada puntual de proyecto, recuperación de operaciones y confirmaciones verificadas; [evidencia](ARCHIVES.md). G1/G2 siguen pendientes.

- 2026-10-16: Exploración visual de la interfaz con videos de presentación (`brag-output/`). Capturas del panel y documento de referencia en `docs/UI-REFERENCIA.md`; incluye propuestas nuevas (árbol proyecto/repo/servicio, adopción por opciones) pendientes de confirmar con David.
- 2026-10-16: Idea registrada en el pipeline y definida en sesión con David (22 decisiones). Promovida a Asset. Documentación de implementación escrita en `docs/`. Script de línea base en `tools/baseline.sh`.

---

*VentureOS · Asset creado: 2026-10-16 · Última actualización: 2026-10-17*
