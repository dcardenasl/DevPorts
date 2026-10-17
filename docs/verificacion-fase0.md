# DevPorts: informe de verificación de la Fase 0

> Fecha: 2026-10-17. Máquina: Mac con Apple Silicon, macOS 26.3, Rust 1.98.1, Docker Desktop.
> Cubre las tareas T0.2 a T0.8. Evidencia detallada (comandos, cifras, commits) en `ARCHIVES.md`.
> Lo que este informe **no** puede cerrar solo (S0, comprobaciones manuales y la puerta G0) está en la sección 3 y requiere a David.

## 1. Resultado de V1 a V12

Leyenda: **Favorable** = cumple y está probado con procesos, contenedores o launchd reales. **Parcial** = lo automatizable está probado y queda una comprobación humana. **Resuelto** = deja de ser una pregunta abierta.

| # | Verificación | Resultado | Evidencia | Decisión |
|---|---|---|---|---|
| V1 | `libproc` lista procesos en escucha del mismo usuario sin privilegios | **Favorable** | `MacProcessSource` lista los sockets TCP en `LISTEN` de los procesos del usuario sin `sudo`; 5 pruebas con servidores reales (`tests/macos_process_source.rs`) | Se usa `libproc`. El plan B `lsof` no hace falta |
| V2 | `cwd` y `argv` disponibles para `node`, `php`, `python` | **Favorable** | `php -S`, `node -e` y `python3 -m http.server` mostraron puerto, pid, carpeta y comando correctos. Dos hallazgos: `libproc::pidcwd` **no está implementado en macOS** y `libproc` no lee `argv`, así que ambos salen de `sysinfo`; y el Python de Homebrew se relanza como `Python`, así que un `match` no debe apoyarse en `argv[0]` | `sysinfo` para `argv` y `cwd` (ADR-025). Regla escrita en MANIFIESTO §3 |
| V3 | Costo de CPU del escaneo completo cada 1,5 s | **Favorable** | Con 24 procesos escuchando, un escaneo cuesta unos 4 a 5 ms de CPU y 7 MB (build release, 40 ejecuciones). El presupuesto es 15 ms por ciclo (1 %) | Intervalo de 1,5 s se mantiene. Se vuelve a medir con el núcleo residente en T2.7 |
| V4 | `docker context inspect` y eventos en el runtime de David | **Favorable** | El contexto se resuelve leyendo el almacén de contextos, igual que la CLI (probado con `default` y `desktop-linux`); `bollard` lista y recibe `start`, `stop`/`die` y `destroy` en menos de 3 s contra el demonio real (`tests/docker_engine.rs`, `--ignored`) | Se lee el almacén de contextos en vez de ejecutar `docker context inspect`: el núcleo no crea procesos (ADR-025). Los eventos se piden con `since` y se tratan como pistas |
| V5 | Tauri: contador junto al ícono, panel anclado, política accessory, destrucción del webview | **Parcial** | Ícono de plantilla visible en la barra; sin ícono en el Dock; panel de 380 x 520 con IPC funcionando; el webview se destruye al cerrar. La CLI de Tauri funciona sin `src-tauri/`. **No verificado:** anclaje bajo el ícono tras un clic real, cierre por pérdida real de foco y la guarda contra reapertura. Esquinas rectas (las de 14 px exigen ventana transparente) | Tauri sirve como envoltura. Comprobación manual en la sección 3. Esquinas: decisión en T2.0 |
| V6 | Botones de acción en notificaciones de macOS | **Parcial** | `tauri-plugin-notification` 2.7 admite acciones en escritorio (`on_action`). macOS pidió el permiso de notificaciones y no entregó nada hasta aceptarlo, así que el botón y su clic **no se verificaron**. El backend usa NSUserNotification, obsoleta desde macOS 11 | Diseño provisional: botón «Extender 30 min» si funciona; si no, la notificación abre el panel enfocado (ARQUITECTURA §12). Plan B: `UNUserNotificationCenter` desde `platform::macos` |
| V7 | launchd `KeepAlive` con la app; «Salir» no reinicia | **Favorable** | Con un trabajo de launchd real: arranca con `RunAtLoad`; tras `SIGKILL` se reinicia en unos 4 s (`ThrottleInterval` 5); tras `exit(0)` no se reinicia; `bootout` la detiene (`tools/spike-launchd.sh`) | Agente propio con `KeepAlive{SuccessfulExit:false}` y `ThrottleInterval` 5 (T2.6). El plugin de autoarranque de Tauri **no sirve**: no escribe `KeepAlive`. A evaluar: `SMAppService` |
| V8 | Latencia de `open -g` para el autoarranque del CLI | **Favorable** | 42 a 129 ms en volver, proceso visible a los 59 a 162 ms; funciona por ruta, por nombre y por identificador | El CLI usa `open -g -b com.davidcardenas.devports`. El plazo de 2 s del falla abierto sobra |
| V9 | `exec` conserva el PID y la ascendencia usada para el vínculo fuerte | **Favorable** | `exec` conserva el pid; un servidor que bifurca es descendiente del lanzador; uno desprendido queda con `ppid = 1` y pierde la ascendencia; un servidor desde una shell no interactiva hereda el grupo de procesos del lanzador (`tests/macos_exec_ancestry.rs`) | El vínculo fuerte y el medio quedan como están. Confirmada la regla de no señalar grupos |
| V10 | Memoria y CPU en reposo contra el presupuesto | **Favorable, sin margen** | Memoria física (`footprint`): 21,0 MB con el panel sin abrir; 66,7 MB con el panel abierto; **49,1 MB** con el panel cerrado tras usarlo (app 32,0 + 2 procesos compartidos de WebKit 17,1 que no se liberan). CPU: 0,00 a 0,05 % en reposo. RSS (78 a 100 MB) engaña y no se usa | El presupuesto se cumple por 0,9 MB y **sin el núcleo dentro**. Se mide la memoria física, no el RSS (ver sección 2). La decisión sobre SwiftUI queda para T2.7/G2 |
| V11 | Velocidad del descubrimiento bajo `~/developer` con 3 niveles | **Favorable** | 2 ms en caliente y 62 ms en frío sobre `~/Developer` real (aún sin manifiestos) | Valores por defecto se mantienen (raíz `~/developer`, profundidad 3) |
| V12 | Runtime de Docker y uso de compose | **Resuelto** | **Docker Desktop**, contexto `default` (`/var/run/docker.sock` enlaza a `~/.docker/run/docker.sock`). Usa **compose y `docker run`**: el MySQL compartido es el servicio `mysql` del proyecto compose `docker-services` en `~/docker-services`, fuera de `~/developer`, junto a `phpmyadmin`; hay contenedores sueltos con puertos dinámicos (`suite-*`, `ci4-preflight-mysql-*`) | El MySQL se declara en `shared.yml` con `docker: mysql`; los sueltos aparecen «sin proyecto». Cierra el punto abierto 2 de DECISIONES |

Resumen: 9 favorables o resueltas, 2 parciales (V5 y V6, pendientes de una comprobación humana) y 1 favorable sin margen (V10).

## 2. Ajustes a valores **[P]** de la especificación

Registrados en ADR-025 (`DECISIONES.md`), con el cambio ya aplicado en los documentos:

1. **Definición del presupuesto de memoria (NF-01, ARQUITECTURA §14).** «Menos de 50 MB en reposo» pasa a significar **memoria física (`footprint`) de la app más sus procesos auxiliares de WebKit con el panel cerrado**, que es la cifra del Monitor de Actividad. El RSS cuenta frameworks compartidos del sistema y no es comparable.
2. **Observación de procesos (ARQUITECTURA §4).** `libproc` para sockets, padre, grupo e inicio; `sysinfo` para `argv` y directorio de trabajo. El plan B `lsof` queda como contingencia no necesaria.
3. **Contexto de Docker (ARQUITECTURA §4).** Se lee el almacén de contextos; no se ejecuta `docker context inspect`.
4. **Falla abierto del CLI (CLI §3).** `open -g -b <identificador>` en lugar de `open -g -a "<nombre>"`.
5. **Supervisión (ARQUITECTURA §11).** Agente propio; el plugin de autoarranque queda descartado.
6. **`unsafe` (ADR-025).** `unsafe_code` pasó de `forbid` a `deny` con una sola excepción vigilada por un test; el núcleo no crea procesos.

## 3. Lo que falta para cerrar la Fase 0 (acciones de David)

### 3.1 Línea base (S0), todavía sin iniciar
`~/.devports-baseline/` no existe: `tools/baseline.sh` aún no se ha ejecutado en la Mac.

```bash
./tools/baseline.sh once      # prueba una muestra y revisa ~/.devports-baseline/samples.tsv
./tools/baseline.sh start     # muestreo cada 30 s en segundo plano
```

Trabajar normal al menos 5 días laborales; después `./tools/baseline.sh report > docs/linea-base-AAAA-MM-DD.txt`.
El script no se había probado en macOS: si `once` falla, se corrige como tarea `fix(tools):` antes de seguir.

### 3.2 Semana de uso de la sonda (criterio b de la puerta G0)
```bash
cargo install --path crates/devports-core --bin devports-probe --locked
devports-probe ls        # servidores que escuchan, con su proyecto y servicio si hay manifiesto
devports-probe docker    # contenedores del contexto activo
devports-probe opens     # aperturas de `ls` por día
```
Usarla durante una semana de trabajo. El criterio de la puerta es: duplicados simultáneos a la mitad o menos que en la línea base **y** 3 o más aperturas por día laborable.

### 3.3 Comprobaciones manuales de lo que el entorno de desarrollo no pudo hacer

| # | Comprobación | Resultado esperado | Anotar |
|---|---|---|---|
| 1 | `cargo run -p devports-app` y clic izquierdo en el ícono | El panel aparece **bajo el ícono** | ¿Bien anclado? ¿Parpadea? |
| 2 | Con el panel abierto, clic fuera de él | Se cierra | |
| 3 | Con el panel abierto, clic en el ícono | Se cierra y **no** se reabre | |
| 4 | Clic derecho en el ícono | Menú «Abrir panel» y «Salir» | |
| 5 | «Salir» | El proceso termina | |
| 6 | `cargo run -p devports-app --features spike-notify`; aceptar el permiso de notificaciones que pide macOS | Notificación «api vence en 1 minuto» con botón «Extender 30 min» | ¿Botón directo o en «Opciones»? Pulsarlo escribe `notify: user performed extend` en la terminal; pulsar el cuerpo escribe `tap` |
| 7 | Abrir `target/debug/bundle/macos/DevPorts.app` (tras `npx @tauri-apps/cli@2 build --debug --bundles app` en `crates/devports-app`) | Sin ícono en el Dock ni rebote | |

La comprobación 6 deja una entrada «devports-app» en Configuración del Sistema > Notificaciones; se puede eliminar después.

### 3.4 Puerta G0 (decisión humana, ADR-022)
| Criterio | Estado |
|---|---|
| (a) V1, V2, V3 y V9 favorables o plan B aceptado | **Cumplido** |
| (b) Una semana con la sonda: duplicados a la mitad o menos y 3 o más aperturas por día | **Pendiente** (3.1 y 3.2) |
| (c) Presupuesto de reposo medido | **Cumplido** con la salvedad de V10: 49,1 MB tras usar el panel, sin el núcleo dentro |

Si (b) falla, la regla de ADR-022 es parar y replantear. **No se empieza la Fase 1 sin una decisión escrita de David.**
