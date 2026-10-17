# DevPorts: Estrategia de pruebas

> Decisión base **[C]** (ADR-021): fuente de procesos falsa para unitarias; procesos reales para integración; Docker real solo en la Mac de David, porque el CI de macOS de GitHub no tiene Docker.
> Fuente de verdad ante contradicciones: `DECISIONES.md`.

## 1. Principios
1. **El núcleo se prueba sin interfaz.** `devports-core` no depende de Tauri; las pruebas usan `ProcessSource`, `ContainerSource` y `Clock` falsos (ARQUITECTURA §3).
2. **Lo destructivo se prueba más.** La parada de procesos tiene pruebas de propiedades y de integración dedicadas (NF-06).
3. **Reloj inyectable.** Ningún test espera tiempo real para vencimientos.
4. **Golden files** para mensajes de rechazo y JSON (CLI §3).
5. Toda prueba de integración que lanza procesos limpia lo que crea, incluso si falla.

## 2. Niveles

### Unitarias (U) con fuentes falsas
| ID | Qué verifica | Requisitos |
|---|---|---|
| U-01 | Normalización de comandos: todas las filas de la tabla de MANIFIESTO §3 | RF-04 |
| U-02 | Matching proceso a servicio: cwd dentro, `match` contiguo, resolución de ambigüedad (más largo, luego puerto, luego "ambiguo") | RF-04 |
| U-03 | Validación de manifiesto: cada error y advertencia de MANIFIESTO §1; la clave `test` genera advertencia | MANIFIESTO |
| U-04 | Tabla de decisión de `run`: cada fila de CLI §3 (incluido tope de Temporales, sin `--reason`, puerto ajeno) | RF-06, RF-07, RF-08 |
| U-05 | Principal de hecho: registrada manda; si no, la del puerto preferido; si no, la más antigua; el resto son duplicados | RF-21, RF-10 |
| U-06 | Vencimientos con reloj falso: aviso a T-60 una sola vez, vencimiento, `extend` con tope de 4 h acumuladas, reinicio de `warned` | RF-09 |
| U-07 | Asignador de puertos: rango, ocupados, reserva, liberación tras gracia | RF-08 |
| U-08 | Tokens: se guarda solo el hash; `ls` y `log` nunca lo incluyen | RF-11 |
| U-09 | Reglas de parada: conjunto = proceso + descendientes; prohibiciones (PID 1, propio núcleo y ancestros, otro uid); grupo solo si `pgid == pid` y todos los miembros pertenecen al conjunto | NF-06 |
| U-10 | Vínculo: fuerte, medio, ninguno; reutilización de PID no confunde procesos | RF-06 |
| U-11 | Códigos de salida y JSON de rechazo contra golden files | RF-07 |
| U-12 | (retirada por ADR-023: `--replace` no existe en v1; el ID no se reutiliza) | n/a |

### Propiedades (PT)
| ID | Propiedad |
|---|---|
| PT-01 | La normalización es idempotente |
| PT-02 | La identidad de un servicio no cambia si solo cambia el puerto del proceso |
| PT-03 | Con PIDs reutilizados al azar, `(pid, start_time)` nunca asocia un proceso nuevo a una instancia vieja |
| PT-04 | Para cualquier árbol de procesos aleatorio, el conjunto de señales de la parada es subconjunto del proceso vinculado y sus descendientes |

### Integración (I) con procesos reales (macOS en CI y en la Mac de David)
| ID | Escenario | Resultado esperado |
|---|---|---|
| I-01 | **Caos:** 7 `devports run api` simultáneos sin `--as` | Exactamente 1 Principal; 6 rechazos con código 10; 0 procesos extra |
| I-02 | **Caos:** 7 servidores lanzados a mano sin registrar para el mismo servicio | 1 Principal de hecho + 6 duplicados detectados en 3 s o menos |
| I-03 | Temporal con TTL corto de prueba | Al vencer, el proceso muere y el puerto queda libre |
| I-04 | La app abre el puerto 10 s después de la reserva | Se vincula correctamente |
| I-05 | Otro proceso toma el puerto reservado | No se ata vencimiento; evento y notificación; reserva liberada |
| I-06 | El comando bifurca 3 hijos | Todos mueren al vencer; el shell padre sobrevive |
| I-07 | Servidor lanzado desde un shell no interactivo que comparte grupo de procesos | La parada no mata al shell ni al lanzador |
| I-08 | Permisos: el CLI no puede detener una Principal; un token ajeno no sirve; `ls` no muestra tokens | Operación inexistente o rechazada |
| I-09 | Núcleo caído | `run` ejecuta el comando y avisa; con `DEVPORTS_NO_AUTOSTART=1` no intenta abrir la app |
| I-10 | Se mata el núcleo con un Temporal vencido pendiente | Al volver, lo detiene de inmediato |
| I-11 | Segundo núcleo; socket huérfano; par con uid ajeno (simulado) | No arranca el segundo; el socket se limpia; se rechaza al par |
| I-12 | (retirada por ADR-023; el ID no se reutiliza). Sustituida por verificar que `--replace` es un argumento inválido (código 16) | `devports run api --replace` falla sin iniciar nada |
| I-13 | `{PORT}` y `${PORT}` en un shell real | `$PORT` llega vacío al CLI; `{PORT}` se sustituye |
| I-14 | Descubrimiento: manifiestos anidados, `node_modules` ignorado, profundidad 3 | Proyectos esperados |
| I-15 | `adopt` con servidores de ejemplo (`php -S`, `node`, `python -m http.server`); escritura sobre un manifiesto con comentarios | Propuesta correcta; comentarios y orden preservados |
| I-16 | Atribución: lanzar desde un árbol de procesos con y sin ancestro conocido | Cadena mostrada; lo desconocido aparece crudo; ninguna acción cambia |
| I-17 | Señales de instancia vieja: cambiar de rama o hacer commit tras el inicio | Se marca "arrancó antes del último commit" |

### Docker (D)
En CI con `ContainerSource` simulado: D-01 a D-04. En la Mac de David con Docker real, mediante `tools/docker-checks.sh` (a escribir en la Fase 1), antes de cada versión:
| ID | Escenario |
|---|---|
| D-01 | Contenedor de compose se asigna al proyecto por etiquetas |
| D-02 | `docker run -d -p {PORT}:3306 ...` como Temporal: vínculo por contenedor; al vencer, `docker stop`; ningún `rm` |
| D-03 | Eventos `start` y `stop` reflejados en 3 s o menos; reconexión al reiniciar Docker |
| D-04 | MySQL `kind: data`: jamás se detiene automáticamente |
| D-05 | El runtime que use David (Docker Desktop, OrbStack o Colima) |

### Interfaz (UI)
- Componentes React con datos simulados (Vitest y Testing Library): contador, listas, cuentas regresivas, advertencia de dependientes.
- E2E mínima de Tauri **[P]**: abrir panel, verificar que el webview se destruye al cerrar.

### Manuales en macOS (M)
- M-01 a M-12: la tabla V1 a V12 de ARQUITECTURA §15 (informe de la Fase 0).
- M-13: notificaciones del sistema, con y sin botón de acción.
- M-14: una hora en reposo con panel cerrado; registrar memoria y CPU (`top -l`, Monitor de Actividad) contra el presupuesto.
- M-15: reinicio del Mac: la app arranca sola; matar el proceso: se reinicia; "Salir": no se reinicia.

## 3. Integración continua
| Trabajo | Dónde | Qué ejecuta |
|---|---|---|
| Rápido | Linux | `fmt`, `clippy`, unitarias y de propiedades del núcleo con fuentes falsas, pruebas del CLI y del protocolo |
| macOS | macOS de GitHub | Compilación completa, `libproc` real, pruebas de integración con procesos reales (I-01 a I-17 salvo las que requieren Docker o permisos especiales) |
| UI | Linux | Pruebas de componentes |
| Docker | Manual en la Mac de David | `tools/docker-checks.sh` antes de cada versión |

El CI no puede ejecutar Docker en macOS; esa brecha se cubre con el cliente simulado y el script manual.

## 4. Matriz de requisitos a pruebas
| Requisito | Pruebas |
|---|---|
| RF-01 | I-14 |
| RF-02, RF-03 | M-01 a M-04, D-03, I-02 |
| RF-04 | U-01, U-02, PT-01, PT-02 |
| RF-05 | I-15 |
| RF-06, RF-07 | U-04, U-10, U-11, I-01, I-13 |
| RF-08 | U-04, U-07, I-03, I-04, I-05 |
| RF-09 | U-06, I-03, I-06, I-07, I-10, PT-04 |
| RF-10 | U-05, I-02 |
| RF-11 | U-08, I-08 |
| RF-12 a RF-16 | UI, M-13 |
| RF-17 | I-09 |
| RF-18 | M-15 |
| RF-19, RF-20 | I-16, I-17 |
| RF-21 | U-05 |
| NF-01 | M-14 |
| NF-04 | I-11 |
| NF-06 | U-09, PT-03, PT-04, I-06, I-07 |
| NF-08 | I-10 |

## 5. Medición de la Fase 0 (ADR-022)
1. **Semana base:** `tools/baseline.sh start` y trabajo normal; al final `report` y guardar la salida.
2. **Semana con la herramienta:** `baseline.sh` sigue corriendo y se usa `devports ls`; cada invocación se registra en `~/.devports-baseline/opens.log`.
3. **Comparar:** `max_duplic` y `pct_con_dup` de ambas semanas, y aperturas por día.
4. **Criterio:** reducción de al menos la mitad en duplicados **y** 3 o más aperturas por día laboral. Si no, parar.
Limitación: la herramienta del baseline es aproximada (misma carpeta y mismo comando) y solo se probó con servidores simulados en Linux.

## 6. Definición de hecho por fase
- **Fase 0:** informe V1 a V12 firmado + criterio de la sección 5.
- **Fase 1:** U, PT e I en verde en CI (Linux y macOS) + D con simulados + `docker-checks.sh` en verde en la Mac de David + 3 días de uso real sin paradas indebidas.
- **Fase 2:** UI y M-13 a M-15 + presupuesto cumplido.
- **Fase 3:** E2 y E3 de PRD.md ejecutados de punta a punta en un proyecto CI4 nuevo.
