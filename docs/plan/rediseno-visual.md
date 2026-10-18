# Plan — Rediseño visual: el panel de sistema (UXR-10)

> Fuente de las tareas UXR-10.x de [TASKS.md](../../TASKS.md#rediseno-visual). Decisión: ADR-042. Sistema: [DESIGN.md](../../DESIGN.md). Contexto de producto: [PRODUCT.md](../../PRODUCT.md).

## Contexto
David no está conforme con el aspecto de la app («parece diseño de IA»). Diagnóstico sobre `styles.css` y las capturas: una tarjeta con borde y fondo teñido por cada elemento, color de estado en cinco sitios a la vez (texto, borde, fondo, punto y botón), etiquetas en mayúsculas monoespaciadas con tracking, un solo radio y una sola densidad, fundido de entrada en cada vista. Dirección confirmada: nativo de macOS, qué corre y qué pide decisión primero, color en un solo canal.

## Qué cambia y qué no
Cambia solo la presentación: tokens, CSS y la estructura de clases de las vistas. No cambia ningún texto, contrato, flujo ni comportamiento. La ventana sigue opaca (ADR-025), sin `unsafe` nuevo.

## Diseño (resumen; ver DESIGN.md)
- **Grupo + fila** en lugar de tarjeta: `.group` (radio 10, fondo `group`) con `.row` separadas por una línea que empieza en el texto. Encabezado de grupo en minúscula, 11/600 gris.
- **Un canal de color:** punto de estado; la palabra de estado coloreada solo cuando pide acción. Sin `wash-*`, `edge-*`, `tint-*`, `halo-*`.
- **Acción:** botón secundario neutro; un primario `accent-fill`; destructivo en texto rojo (relleno solo en la confirmación).
- **Tipografía:** título 20/700, titular 13/600, cuerpo 13, etiqueta 11/600, dato mono 12 solo para puertos, rutas, comandos y cuentas regresivas.
- **Movimiento:** sin fundidos de entrada; transiciones de 150 ms solo de estado (hover, plegar, cuenta regresiva).

## Tokens (ambos temas, mismo nombre)
`panel`, `group`, `sheet`, `toast`, `field`, `text`, `text-soft`, `text-dim`, `accent`, `accent-fill`, `on-accent`, `green`, `amber`, `red`, `line`, `line-strong`, `veil`, `hover`, `press`, `selected`, `dot-down`, `dot-loose`. Se retiran: `tabbar`, `text-strong`, `blue`, `coral` (pasa a `red`), `on-green/amber/coral/badge`, `wash-*`, `edge-*`, `tint-*`, `halo-*`, `ov-*`, `surface`, `pill-*`, `hover-filter`.

## Tareas
| ID | Contenido | Depende de |
|---|---|---|
| UXR-10.0 | Documentos: PRODUCT.md, DESIGN.md, ADR-042, este plan, fichas | — |
| UXR-10.1 | Sistema: `theme.css`, componentes base en `styles.css`, contraste, prueba de coherencia | 10.0 |
| UXR-10.2 | Vistas: Atención, Proyectos, Compartidos, Sin proyecto, Actividad y barra de estado | 10.1 |
| UXR-10.3 | Hojas y estados: modal, formularios, adopción, consumidores, ignorar, clasificar, ajustes, aviso, vacío, cargando | 10.1 |
| UXR-10.4 | Evidencia y cierre: capturas claro/oscuro × es/en, `UI-REFERENCIA.md`, M-17 nativo | 10.2, 10.3 |

## Uso de /commit-flow
Una línea, inglés, sin trailers, 1–3 commits por tarea: 10.0 `docs(design): record adr-042 and the system design`; 10.1 `refactor(ui): replace the color tokens with the system palette` → `refactor(ui): rebuild the base components as groups and rows` → `test(ui): check the system tokens and the visual coherence`; 10.2 `refactor(ui): lay out the views as grouped lists`; 10.3 `refactor(ui): restyle the sheets and states with the system`; 10.4 `docs: close uxr-10 with evidence`.

## Riesgos
- Regresión de pruebas por clases: las pruebas consultan por rol y texto; se ajustan solo las que dependían de clases.
- Contraste: la paleta está precalculada AA; el test decide.
- Alcance: sin nuevos comportamientos; cualquier mejora de UX se registra como tarea aparte.
