# DevPorts: Documento de requisitos del producto (PRD)

> Nombre de trabajo (ADR-020). Versión 1.0 del documento: 2026-10-16.
> Responsable: David Cardenas. Estado: especificado, sin código.
> Marcas: **[C]** confirmado por David. **[D]** derivado. **[P]** propuesta a validar.
> Fuente de verdad ante contradicciones: `DECISIONES.md`.

---

## 1. Resumen
DevPorts es una app de barra de menús para macOS que mantiene el orden de los servidores de desarrollo: muestra qué servidores, puertos y contenedores corren, a qué proyecto pertenecen, cuál es el oficial y cuáles son pruebas con fecha de vencimiento, y permite detenerlos. Incluye un CLI (`devports run`) que los agentes de IA usan para levantar servidores sin duplicarlos.

## 2. Problema
David trabaja con varios proyectos de código a la vez en una misma máquina: servidores en distintos puertos, varios repositorios por proyecto, Docker y una imagen de MySQL en contenedor. Dos fallas recurrentes **[C]**:
1. **Los agentes de IA levantan servidores nuevos sin saber que ya existe uno.** Dato de David: ocurre todos los días; parte con 1 repo montado y durante el día la IA llega a montar hasta 7 servidores simultáneos.
2. **Se prueba contra un servidor que no es el que se debería probar.**

Resultado: confusión constante sobre qué corre, de qué proyecto es y cuál es el correcto. Las herramientas existentes (Berth, Bosun) solo observan; ninguna sabe lo que *debería* haber ni distingue un duplicado por error de una prueba legítima.

## 3. Usuarios y actores
| Actor | Rol |
|---|---|
| **David** (usuario primario) | Ve el estado, aprueba reemplazos, detiene Principales, extiende o termina pruebas |
| **Agentes de código** (Claude Code, Cursor, Codex u otros; agnóstico, ADR-004) | Ejecutan `devports run`, `ls`, `stop`/`extend` de sus propios Temporales |
| Otros desarrolladores | No son objetivo de la v1; posible público si se publica (ADR-020) |

## 4. Objetivos
1. Que ningún agente cree un servidor duplicado sin que quede registrado, visible y con dueño de ciclo de vida.
2. Que las pruebas con datos dummy puedan existir y desaparezcan solas (vencimiento).
3. Que David vea en un vistazo qué corre, de qué proyecto y por qué.
4. Simple pero robusto y sólido: nada de funcionalidades que hoy no se necesitan **[C]**.

## 5. No objetivos (fuera de la v1) **[C]** (ADR-015)
Servidor MCP; adaptadores por agente; proxy de nombres estables; versión Pro o equipos; iniciar y gestionar procesos (tipo pm2); borrar contenedores o volúmenes; gestionar el contenido de bases de datos; Linux y Windows.

## 6. Escenarios clave

**E1. Inicio del día.** David ejecuta el `start-all.sh` del proyecto; cada línea es `devports run <servicio> --reuse`. Quedan registradas las Principales. La barra muestra el proyecto con sus servicios sanos.

**E2. Un agente intenta levantar un servidor que ya existe.** Ejecuta `devports run api`. Recibe un rechazo (código 10) con la URL de la instancia existente y las alternativas. Usa esa URL. No se crea ningún proceso. Queda una línea en la actividad.

**E3. Un agente necesita probar con datos dummy.** Ejecuta `devports run api --as test --reason "QA login" -- <su comando con {PORT}>`. Recibe un puerto libre, un token y un vencimiento de 30 min. Convive con la Principal. Un minuto antes de vencer, David recibe un aviso con "Extender 30 min". Al vencer, la prueba se detiene sola.

**E4. Alguien levanta a mano un duplicado.** Aparece un proceso sin registrar que duplica una Principal. La barra suma 1 al contador y envía una notificación (máximo una por proyecto cada 10 minutos). David elige Detener, Conservar o Marcar como legítimo.

**E5. David apaga el MySQL compartido.** La app muestra primero qué proyectos lo usan y pide un clic de confirmación. Nunca se detiene sola.

**E6. La app está cerrada.** `devports run` intenta abrirla; si no puede, ejecuta el comando y avisa "sin protección". Lo que se cuele aparece luego como "sin registrar".

**E7. El servidor ya estaba levantado a mano.** David levantó `php spark serve` sin devports. Un agente ejecuta `devports run api`. El servidor observado cuenta como Principal de hecho, así que el agente recibe el rechazo con su URL y no crea un duplicado.

## 7. Requisitos funcionales

| ID | Requisito | Criterio de aceptación | Fase |
|---|---|---|---|
| RF-01 | Descubrir proyectos en carpetas raíz | Dado `~/developer` con 3 manifiestos anidados hasta nivel 3, cuando arranca el núcleo, entonces los lista en menos de 5 s y reacciona a un manifiesto nuevo sin reiniciar (MANIFIESTO §7) | F1 |
| RF-02 | Observar procesos del host que escuchan TCP, con cwd, argv, inicio y padre | Un servidor nuevo aparece en `ls` en 3 s o menos **[P]** (ARQUITECTURA §4) | F0/F1 |
| RF-03 | Observar contenedores Docker por eventos | Un contenedor que arranca o se detiene se refleja en 3 s o menos **[P]**; el socket se obtiene del contexto activo | F0/F1 |
| RF-04 | Asociar instancias a proyecto y servicio por identidad (cwd + `match`) | Un proceso con otro puerto pero igual cwd y comando normalizado se asocia al mismo servicio (MANIFIESTO §3) | F1 |
| RF-05 | "Adoptar": proponer el manifiesto desde lo observado | `devports adopt` imprime una propuesta válida sin escribir; la app escribe al confirmar, preservando comentarios (MANIFIESTO §8) | F1 (propuesta), F2 (escritura) |
| RF-06 | `devports run` registra y ejecuta la Principal | Sin instancia previa: registra, exporta variables, hace `exec` y la instancia queda `running` (CLI §3) | F1 |
| RF-07 | Conflictos deterministas | Con Principal sana y sin `--as`: código 10, nada iniciado, mensaje con URL y alternativas; con `--reuse`: código 0 sin iniciar; nunca pregunta salvo `DEVPORTS_INTERACTIVE=1` en terminal | F1 |
| RF-08 | Temporales con propósito | `--as test --reason` convive con la Principal, recibe puerto libre, token y vencimiento; tope 2 por servicio (código 12 al excederlo); sin `--reason`, código 16 | F1 |
| RF-09 | Ciclo de vida de Temporales | Aviso a T-60 s una vez; `extend` hasta 4 h acumuladas; al vencer, parada segura; los vencimientos sobreviven a un reinicio del núcleo (ARQUITECTURA §8) | F1 (núcleo), F2 (avisos en UI) |
| RF-10 | Detectar duplicados sin registrar y "Olvidados" | Servicio duplicado sin registro: queda marcado en 3 s o menos; vencidos aún vivos aparecen en Olvidados | F1 (detección), F2 (UI) |
| RF-11 | Permisos del CLI | Con un token se detiene o extiende solo ese Temporal; ninguna operación del socket detiene una Principal o una instancia sin registrar; `ls` y `log` nunca muestran tokens | F1 |
| RF-12 | Detener desde la app | Un clic detiene una Principal; si es `kind: data` o compartida, muestra antes los proyectos afectados | F2 |
| RF-13 | Servicios compartidos | Un MySQL en `shared.yml` citado por 2 proyectos aparece en "Compartidos" con ambos (MANIFIESTO §2) | F1 (modelo), F2 (UI) |
| RF-14 | Contador y notificaciones | Contador = duplicados sin registrar + Temporales por vencer; notificación solo para vencimiento y duplicado, con ventana de 10 min por proyecto (ADR-014) | F2 |
| RF-15 | Registro de actividad | `devports log` y la pestaña muestran lanzamientos, rechazos con motivo, Temporales, y atribución disponible | F1 (log), F2 (vista) |
| RF-16 | Copiar comando de inicio | Para un servicio con `start`, un clic copia el comando con el puerto preferido sustituido | F2 |
| RF-17 | Falla abierto | Con el núcleo caído, `run` ejecuta el comando y avisa; no registra nada (ADR-018) | F1 |
| RF-18 | Arranque al iniciar sesión y reinicio automático | Tras reiniciar el Mac la app corre; si se cae por error, se reinicia; "Salir" no reinicia | F2 |
| RF-19 | Atribución informativa y agnóstica | Cada instancia muestra la cadena de ancestros; lo desconocido se muestra crudo; ninguna acción depende de ella (ADR-004) | F1 |
| RF-20 | Señales de instancia vieja | Cada instancia muestra rama, worktree, hora de inicio y si arrancó antes del último commit o checkout | F1 |
| RF-21 | Principal de hecho | Un servidor observado sin registro cuenta como Principal si no hay registrada (ARQUITECTURA §5) | F1 |

## 8. Requisitos no funcionales
| ID | Requisito | Meta |
|---|---|---|
| NF-01 | Consumo en reposo (panel cerrado) | Menos de 50 MB y menos de 1 % de CPU **[P]** |
| NF-02 | Latencia de detección | 3 s o menos **[P]** |
| NF-03 | Arranque del CLI hasta enviar la petición | Menos de 50 ms **[P]** |
| NF-04 | Seguridad de IPC | Socket 0600, verificación de uid, sin puerto TCP **[C]** |
| NF-05 | Privacidad | Sin telemetría ni red propia **[D]** |
| NF-06 | Seguridad al detener | Cumplir las prohibiciones de ARQUITECTURA §8; nunca señalar grupos ajenos **[D]** |
| NF-07 | Portabilidad | Plataforma tras interfaces; portar = escribir un módulo **[C]** |
| NF-08 | Recuperación | Tras reinicio del núcleo se reconcilia y se ejecutan vencimientos pendientes **[D]** |
| NF-09 | Datos | Todo local en SQLite; retención de eventos 30 días **[P]** |
| NF-10 | Salida | Texto en español para personas; `--json` estable para agentes **[D]** |

## 9. Métricas de éxito (Fase 0 y Fase 4) **[C]** (ADR-022)
| Métrica | Fuente | Meta |
|---|---|---|
| Duplicados simultáneos máximos por día y % de muestras con duplicados | `tools/baseline.sh report`, semana base contra semana con la herramienta | Reducción de al menos la mitad |
| Aperturas de la herramienta por día de trabajo | Registro de invocaciones de `devports ls` | 3 o más por día |
| Falsos positivos destructivos | Registro de actividad | Cero (nunca se detiene algo no registrado) |

Si no se cumplen las dos primeras en la Fase 0: parar y replantear.

## 10. Riesgos
| Riesgo | Mitigación |
|---|---|
| El escaneo de procesos consume demasiado | Plan B y palancas de ARQUITECTURA §14; medir en Fase 0 |
| Tauri no da un panel anclado pulido o supera el presupuesto | Envoltura reemplazable; evaluar SwiftUI sin tocar el núcleo (ADR-017) |
| Un agente ignora `devports run` y lanza el comando directo | La detección (RF-10) lo marca; no se pretende impedirlo (macOS no permite bloquear `bind` sin permisos especiales de Apple) |
| Vínculo por puerto reservado se rompe (proceso ajeno toma el puerto) | Reglas de vínculo y evento; nunca atar vencimiento sin verificar (ARQUITECTURA §7) |
| Competencia gratuita (Berth, Bosun) | El diferencial es el ciclo de vida Principal/Temporal; publicar solo con evidencia (ADR-020) |
| Dispersión del portafolio (Kosten parado, Proofy y FILMA sin sesión) | Puertas de revisión al final de cada fase; ROADMAP §6 |
| Estimación superior a la sugerida al registrar la idea | Puerta explícita al terminar la Fase 1 |

## 11. Preguntas abiertas
Ver `DECISIONES.md`, sección "Supuestos y puntos abiertos".
