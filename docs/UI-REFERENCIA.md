# DevPorts: referencia visual de la interfaz

> Origen: exploración visual hecha con David el 2026-10-16 para los videos de presentación. Las capturas salen de los videos renderizados (v4, v5 y v6), así que muestran exactamente lo que se aprobó.
> Marcas: **[Doc]** está respaldado por la documentación (`PRD.md`, `ARQUITECTURA.md` §12, `CLI.md`). **[Prop]** es una propuesta de esta exploración que la documentación no define: confirmar con David antes de implementarla.
> Fuente de verdad ante contradicciones: `DECISIONES.md`.
> Todos los datos (proyectos, repos, puertos, horas, "QA login") son ejemplos inventados. No son datos reales de David.

---

## 1. Cómo usar este documento
- Las capturas están en `ui-referencia/panel/` (solo el panel y la barra de menús) y `ui-referencia/contexto/` (pantalla completa).
- Cada vista tiene su sección abajo, con los elementos, los estados y las acciones.
- La sección 4 describe los flujos paso a paso; la sección 5, el sistema visual con valores exactos.
- Solo existe modo oscuro. El modo claro no está diseñado (ver sección 7).

---

## 2. Índice de capturas

### Panel (`ui-referencia/panel/`)
| Archivo | Vista | Estado | Origen |
|---|---|---|---|
| `01-atencion-duplicados.png` | Atención | Principal oficial + 6 duplicados sin registrar, contador 6 | v4 |
| `02-atencion-todo-en-orden.png` | Atención | Sin pendientes | v4 |
| `03-atencion-temporal.png` | Atención | Temporal por vencer con cuenta regresiva, contador 1 | v4 |
| `04-atencion-alerta-colgada.png` | Atención | Servicio colgado con botón "Ver en Proyectos" | v5 |
| `05-proyectos-tienda-demo.png` | Proyectos | Proyecto abierto: repos, servidores, Temporal anidado y servicios compartidos | v6 |
| `06-duplicado-en-repo.png` | Proyectos | Duplicado sin registrar dentro de su repo, con Detener y Conservar | v6 |
| `07-proyectos-ci4-kickstart.png` | Proyectos | Acordeón: otro proyecto abierto, el primero plegado | v6 |
| `08-sin-proyecto.png` | Sin proyecto | Proceso sin dueño con sugerencia y botón Adoptar | v6 |
| `09-adoptar-opciones.png` | Sin proyecto | Hoja con opciones: proyecto sugerido, proyecto nuevo, ignorar | v6 |
| `10-proyectos-repo-nuevo.png` | Proyectos | Repo recién adoptado en su proyecto, resumen actualizado | v6 |
| `11-actividad.png` | Actividad | Registro del día con la adopción arriba | v6 |
| `12-compartidos.png` | Compartidos | MySQL y Redis con los proyectos que los usan | v4 |
| `13-compartidos-detener-mysql.png` | Compartidos | Confirmación con los proyectos afectados | v4 |
| `14-proyectos-copiar-comando.png` | Proyectos | Lista plana con acciones por servicio y aviso "Comando copiado" | v5 |
| `15-proyectos-servicio-recuperado.png` | Proyectos | El servicio que estaba colgado vuelve a Principal sana | v5 |
| `16-actividad-todo.png` | Actividad | Filtro "Todo" | v5 |
| `17-actividad-filtro-rechazos.png` | Actividad | Filtro "Rechazos" | v5 |
| `18-actividad-filtro-temporales.png` | Actividad | Filtro "Temporales" | v5 |

Las capturas 14 y 15 vienen de una variante anterior de Proyectos (lista plana por servicio, con botones de abrir, copiar comando y detener). La versión vigente es el árbol proyecto, repo, servicio (05, 06, 07, 10). Usar 14 y 15 como referencia de las acciones por servicio, no del orden de la lista.

### Contexto (`ui-referencia/contexto/`)
| Archivo | Qué muestra |
|---|---|
| `escritorio-completo.png` | El Mac con la barra de menús, el ícono y un editor de fondo |
| `barra-icono.png` | Primer plano del ícono de DevPorts en la barra |
| `cli-rechazo-y-panel.png` | Un agente pide otro servidor del mismo repo y recibe el rechazo; el panel resalta el servicio |
| `cli-duplicado-y-panel.png` | El servidor se levanta sin pasar por DevPorts y aparece el duplicado en el repo |
| `cli-prevenir-y-panel.png` | Rechazo del CLI y la línea en Actividad (variante v4) |
| `cierre-logo.png` | Cierre con el logo (nombre de trabajo) |

---

## 3. Estructura general

### Ícono de la barra **[Doc]** (ARQUITECTURA §12)
- Anillo con un punto central, monocromo (imagen de plantilla: macOS lo tiñe).
- Muestra un contador solo si hay algo que atender (duplicados sin registrar + Temporales por vencer). En las capturas es una insignia coral con el número.
- Al abrir el panel el ícono queda resaltado.
- Clic abre el panel. Clic derecho abre un menú nativo con "Abrir panel" y "Salir" (no diseñado visualmente).

### Panel **[Doc]** tamaño y anclaje, **[Prop]** pestañas
- 380 x 520 px, esquinas de 14 px, anclado bajo el ícono. Se destruye al perder el foco.
- Encabezado (título 17 px semibold, subtítulo 12 px con el estado) arriba; barra de pestañas de 56 px abajo.
- Cinco pestañas con ícono y etiqueta: **Atención, Proyectos, Compartidos, Sin proyecto, Actividad**. La pestaña activa va en blanco; las demás en gris. La pestaña Atención lleva la misma insignia del ícono de la barra.
- El panel muestra una pestaña a la vez; la transición es un fundido corto (~0,3 s).

---

## 4. Vistas

### 4.1 Atención (`01` a `04`)
Qué hay que decidir ahora. **[Doc]** RF-14, ADR-014.
- **Subtítulo:** "Todo en orden", "1 cosa espera tu decisión", "6 cosas esperan tu decisión".
- **Principal oficial:** tarjeta verde con el servicio, el proyecto, `:puerto · rama · desde hora · sana` y la etiqueta OFICIAL. **[Prop]** como contexto fijo arriba de las alertas.
- **Temporal por vencer** (ámbar) **[Doc]**: etiqueta, cuenta regresiva a la derecha (monoespaciada), servicio y motivo, `:puerto · pedido por · duración`, barra de progreso y botones **Extender 30 min** (primario) y **Terminar**. Al extender, la cuenta pasa a verde (30:48) y la barra se llena.
- **Duplicado sin registrar** (coral) **[Doc]**: lista de procesos con puerto, comando y antigüedad. Botones **Detener** (primario coral) y **Conservar**; "Marcar como legítimo" como texto subrayado (está en la doc; en la captura 01 aparece agrupado como "Detener los 6"). **[Prop]** acción en grupo "Detener los 6".
- **Servicio colgado** **[Prop]**: tarjeta coral "COLGADA · NO RESPONDE" con el comando, el tiempo sin respuesta y el botón **Ver en Proyectos** que lleva a esa pestaña, más **Ignorar**. La doc define el estado "colgada" (3 chequeos fallidos) pero no esta tarjeta.
- **Todo en orden:** tarjeta verde tenue.
- No existe la solicitud de reemplazo con Aprobar y Rechazar: `--replace` quedó fuera de la v1 (ADR-023).

### 4.2 Proyectos (`05`, `06`, `07`, `10`, `14`, `15`)
Cómo está organizado el trabajo. **[Doc]** ADR-001 (proyecto = carpeta con `.devports.yml`; los repos son grupos dentro del proyecto), RF-12, RF-16. **[Prop]** la jerarquía visual proyecto, repo, servicio como acordeón.
- **Cabecera de proyecto:** chevron (gira al abrir), nombre, ruta corta en monoespaciada y un resumen a la derecha ("3 repos · 3 servicios").
- **Repo:** fila con ícono de carpeta, nombre en monoespaciada y la rama a la derecha ("main", "develop").
- **Servicio:** punto de estado, nombre, rol (Principal), puerto a la derecha y botón para abrir la URL. Las capturas 14 y 15 muestran además copiar comando de inicio y detener (cuadrado rojo).
- **Temporal anidado:** fila ámbar bajo su servicio, con el motivo y la cuenta regresiva.
- **Servicios compartidos** **[Doc]** ADR-013: línea "Usa" con etiquetas azules (`mysql`, `redis`) al final del proyecto.
- **Duplicado sin registrar en el repo** (`06`): fila coral dentro del repo con "api · sin registrar · :8083", **Detener** y **Conservar**. Suma al contador.
- **Acordeón:** un proyecto abierto a la vez; el otro queda como cabecera. Un proyecto sin servicios activos se muestra atenuado con borde punteado.
- **Detener un servicio** de datos o compartido pide confirmación con los proyectos afectados (ver 4.3) **[Doc]** RF-12.
- **Copiar comando** **[Doc]** RF-16: aviso "Comando copiado" flotante (píldora con marca verde, ~2 s).
- **Recuperación** (`15`): el punto pasa de coral a verde, la etiqueta de "Colgada · no responde" a "Principal", el fondo coral se apaga y la insignia desaparece.

### 4.3 Compartidos (`12`, `13`)
Servicios usados por varios proyectos (MySQL). **[Doc]** ADR-013, RF-13.
- Tarjeta por servicio: punto, nombre, etiqueta "datos" (azul), puerto, `docker · contenedor · activo desde`, y "Lo usan" con una etiqueta por proyecto. Botón de detener a la derecha.
- Al detener, **diálogo de confirmación** dentro del panel: ícono de advertencia, "¿Detener mysql?", la lista de proyectos que pierden la base de datos (con los servicios afectados) y los botones **Cancelar** y **Detener mysql** (coral). Nunca se detiene sola.

### 4.4 Sin proyecto (`08`, `09`)
Procesos y contenedores fuera de todo proyecto. **[Doc]** RF-05, ADR-002, MANIFIESTO §7 y §8.
- **Tarjeta:** comando, puerto, ruta de la carpeta, tiempo en ejecución. Punto con borde punteado.
- **Sugerencia** **[Prop]**: etiqueta verde "Parece de tienda-demo" cuando la carpeta del proceso cae dentro de un proyecto conocido.
- **Adoptar** (botón verde sólido en la sugerida, con borde en las demás).
- **Hoja de opciones** **[Prop]**, tres filas grandes y una sola decisión:
  1. El proyecto sugerido, con el detalle ("Como repo nuevo: tienda-checkout") y la etiqueta SUGERIDO.
  2. Crear un proyecto nuevo (con esta carpeta como raíz).
  3. Ignorar este proceso (no volver a avisar).
- Al elegir, la tarjeta pasa a "Agregado a tienda-demo" en verde y el contador de la pestaña baja. En el producto real, escribir el `.devports.yml` sin perder comentarios (MANIFIESTO §8) ocurre por detrás; no se muestra código ni archivo al usuario.

### 4.5 Actividad (`11`, `16`, `17`, `18`)
Registro del día. **[Doc]** RF-15.
- Filtros en píldoras: **Todo, Rechazos, Temporales** (el filtro activo va en blanco con texto oscuro). **[Prop]** el diseño original también incluía "Alertas".
- Filas: hora en monoespaciada, etiqueta de tipo con color (RECHAZO 10, TEMPORAL, VENCIÓ, EXTENDIDO, PRINCIPAL, ADOPTADO, SIN REGISTRAR) y una línea de texto. La fila nueva se resalta un momento.

---

## 5. Flujos

**A. Entrar.** Clic en el ícono, el panel se abre en Atención. **[Prop]** abre siempre en Atención y se navega con clics (la doc no dice si recuerda la última pestaña).

**B. Un agente pide otro servidor del mismo repo (CLI)** **[Doc]** CLI §3, RF-07. Desde la carpeta del repo:
```
tienda-api $ devports run api
✕ Rechazado (código 10)
api ya corre en http://localhost:8080
tienda-demo · repo tienda-api · desde 09:12
usa --reuse, o --as test --reason "..."
```
No se crea ningún proceso; el panel resalta el servicio y la línea queda en Actividad (`cli-rechazo-y-panel.png`).

**C. Lo levanta sin pasar por DevPorts** **[Doc]** RF-10. El proceso nuevo aparece como duplicado dentro de su repo (`06`), el ícono suma 1. Detener lo apaga, Conservar lo deja, "Marcar como legítimo" lo registra.

**D. Prueba con vencimiento** **[Doc]** RF-08, RF-09. `devports run api --as test --reason "QA login"` crea un Temporal (`✓ Temporal en :52814`). Aparece la tarjeta ámbar con cuenta regresiva; Extender 30 min suma tiempo (máx. 4 h); al vencer se detiene sola.

**E. Limpiar duplicados.** En Atención, **Detener los 6** (**[Prop]** acción en grupo): las filas salen una a una, el contador baja a 0 y aparece "Todo en orden".

**F. Servicio colgado.** Alerta en Atención, **Ver en Proyectos**, la fila resaltada en coral, **copiar comando de inicio**, aviso "Comando copiado", se pega en la terminal, el servidor arranca y la fila vuelve a verde.

**G. Adoptar.** Pestaña Sin proyecto, **Adoptar**, hoja de opciones, un clic en el proyecto sugerido. Al volver a Proyectos el repo y su servicio ya están, con el resumen del proyecto actualizado ("4 repos · 4 servicios").

**H. Detener un servicio compartido.** Botón de detener, diálogo con los proyectos afectados, Cancelar o Detener.

**I. Cambiar de proyecto.** Clic en la cabecera de otro proyecto: el abierto se pliega y el otro se despliega con sus repos y servicios.

---

## 6. Sistema visual

### Color (modo oscuro)
| Token | Valor | Uso |
|---|---|---|
| Fondo del panel | `#171B1F` | Superficie del panel |
| Barra de pestañas | `#1A1E22` | Fondo de las pestañas |
| Texto | `#E8ECEF` | Texto principal |
| Texto suave | `#8C979F` | Texto secundario |
| Texto tenue | `#838E96` | Rutas, horas, pestañas inactivas (cumple AA sobre el panel) |
| Principal / sano | `#3DD6A0` | Servicio oficial y sano, éxito, botón de adoptar |
| Temporal | `#F5B544` | Pruebas con vencimiento, cuenta regresiva |
| Atención | `#FF6A55` | Duplicados, colgados, detener, insignia |
| Dato / info | `#6AA9FF` | Servicios de datos, etiqueta "datos", rechazos |
| Tarjetas de estado | color al 7–10 % de relleno y 30–45 % de borde | Verde, ámbar, coral |
| Superficie neutra | `rgba(255,255,255,0.035)` con borde `rgba(255,255,255,0.07–0.08)` | Proyectos, compartidos, sin proyecto |
| Sombra del panel | `0 24px 60px rgba(0,0,0,0.55)` y borde `rgba(255,255,255,0.12)` | Elevación |

Regla: el color significa estado, nunca decoración. Verde = oficial y sano; ámbar = prueba con reloj; coral = requiere atención; azul = datos.

### Tipografía
- Texto: sans del sistema (en el diseño original, IBM Plex Sans). Títulos 17 px semibold; nombres 13,5–15 px; texto de apoyo 11–12 px.
- Puertos, comandos, rutas y cuentas regresivas: monoespaciada (IBM Plex Mono o la del sistema), 10–13 px, con cifras tabulares.
- Etiquetas en mayúsculas (TEMPORAL POR VENCER, SIN REGISTRAR…): monoespaciada 10 px con espaciado de 0,1 em.

### Componentes
- **Punto de estado:** 8 px. Principal sana: verde con halo de 3 px. Colgada o sin registrar: coral con halo. Temporal: anillo ámbar de 2 px. Sin dueño: anillo punteado gris. Caída: anillo gris.
- **Botones** (altura 30 px, radio 8): primario relleno (ámbar para extender, coral para detener, verde para adoptar); secundario con borde `rgba(255,255,255,0.14)`; terciario solo texto subrayado.
- **Tarjetas:** radio 12; padding 12–14 px. **Etiquetas:** radio 4, fondo del color al 12 %. **Píldoras de filtro:** 24 px de alto.
- **Íconos:** trazo de 1,7 px, 18 px en pestañas, 12–13 px en filas; sin emojis.
- **Hoja y diálogo:** radio 14, fondo `#20262B`, sobre un velo `rgba(8,10,12,0.62–0.66)`.

### Movimiento (para la implementación, valores sugeridos)
- Entrada de tarjetas y filas: desplazamiento de 16–30 px y fundido, 0,35–0,5 s, escalonado 0,25–0,35 s.
- Cambio de pestaña: fundido de 0,3 s. Acordeón: altura en 0,7 s con curva suave.
- Cuenta regresiva: se actualiza cada segundo. Resaltado de una fila nueva: parpadeo verde de ~0,4 s.
- Respetar "reducir movimiento" del sistema.

---

## 7. Qué falta o queda abierto
- **Modo claro:** no diseñado. Los tokens están pensados para invertirse.
- **Notificaciones del sistema** (vencimiento en 1 min con "Extender 30 min", y duplicado): diseñadas en el lienzo "Dirección visual", no en estas capturas.
- **Menú contextual del ícono**, preferencias, arranque al iniciar sesión: sin diseño.
- **"Marcar como legítimo", "Crear un proyecto nuevo" e "Ignorar":** solo están como opciones; falta el detalle de cada una (nombre del proyecto, persistencia de lo ignorado).
- **Estados vacíos y de error** (sin proyectos, Docker apagado, núcleo caído): no diseñados.
- **Teclado y accesibilidad:** foco visible, orden de tabulación y VoiceOver sin definir. Los textos de los videos pasaron contraste AA; los controles reales deben ser botones accesibles.
- **Solicitud de reemplazo** (`--replace`): descartada en la v1 (ADR-023); no hay que diseñarla.
- **Hoja de adopción con diff de manifiesto:** se probó una versión que mostraba el `.devports.yml` línea por línea y se descartó. No implementarla en la UI.

---

## 8. Fuentes de las capturas
- Videos y proyectos: `brag-output/v4-historia-completa/`, `v5-pestanas/`, `v6-proyectos/`. Cada carpeta tiene su `composition/index.html` con el marcado y los estilos exactos de las vistas.
- Lienzo de diseño inicial (cinco tableros: Atención, Proyectos, Compartidos, Actividad y dirección visual con notificaciones): https://claude.ai/artifact/HimC1MiWC24QsxeYTC84Wz (privado).
- Nombre definitivo pendiente (Faro, Portero o Muelle). "DevPorts" es de trabajo (ADR-020).
