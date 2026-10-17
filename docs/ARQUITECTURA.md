# DevPorts: Arquitectura

> Marcas: **[C]** confirmado por David. **[D]** derivado de decisiones confirmadas. **[P]** propuesta a validar en la Fase 0 en la Mac de David.
> Fuente de verdad ante contradicciones: `DECISIONES.md`. Especificaciones relacionadas: `MANIFIESTO.md`, `CLI.md`.

---

## 1. Visión general

Un solo proceso residente (la app de barra) aloja el **núcleo** en Rust. El CLI es un binario aparte, liviano, que habla con el núcleo por un socket Unix. La interfaz web (React) se crea solo al abrir el panel **[C]** (ADR-015, ADR-017).

```
 Agentes / terminal                         App de barra (un proceso, Tauri)
 ┌──────────────┐   socket Unix 0600    ┌────────────────────────────────────────┐
 │ devports CLI │ ────────────────────► │  Servidor IPC (solo operaciones del CLI)│
 │ (binario     │ ◄──────────────────── │                                        │
 │  liviano)    │                       │  NÚCLEO (crate devports-core)          │
 └──────────────┘                       │   - Descubrimiento de proyectos        │
                                        │   - Observación (procesos + Docker)    │
 ┌──────────────┐   comandos Tauri      │   - Identidad y política               │
 │ Panel React  │ ◄───────────────────► │   - Instancias, vencimientos           │
 │ (webview     │   (en proceso, sin    │   - Almacén SQLite + log de eventos    │
 │  efímero)    │    socket)            │   - Notificaciones                     │
 └──────────────┘                       └───────────┬────────────┬───────────────┘
                                                    │            │
                                          ProcessSource    ContainerSource
                                          (libproc, macOS) (API de Docker)
```

Principios que gobiernan el diseño (ADR-003, ADR-004, ADR-006): se decide por identidad y estado; la atribución solo informa; solo se detiene automáticamente lo registrado con vencimiento.

## 2. Estructura del repositorio **[D]**

```
devports/
  Cargo.toml                    # workspace
  crates/
    devports-proto/             # tipos del protocolo IPC (serde), compartidos por CLI y núcleo
    devports-core/              # biblioteca: toda la lógica. SIN dependencia de Tauri
    devports-cli/               # binario `devports`: cliente fino, arranque mínimo
    devports-app/               # Tauri v2: ícono, ventana, aloja el núcleo
  ui/                           # React + TypeScript (panel)
  docs/                         # esta documentación
  tools/baseline.sh             # medición de línea base
```
Reglas **[C]** (ADR-017):
1. `devports-core` no depende de Tauri; se puede ejecutar y probar sin interfaz (ejemplo `examples/headless.rs` que levanta el núcleo con socket).
2. `devports-cli` no depende de `devports-core` ni de Tauri; solo de `devports-proto`. Meta: arranque en menos de 50 ms hasta enviar la petición **[P]**.
3. El webview se crea al abrir el panel y se destruye al cerrarlo.

## 3. Abstracciones de plataforma **[D]** (ADR-016)

El núcleo no llama directamente al sistema operativo. Define interfaces; solo la implementación de macOS existe en v1.

```rust
trait ProcessSource {            // implementación v1: macOS (libproc)
    fn snapshot(&self) -> Result<Vec<ListeningProcess>>;   // solo procesos del usuario que escuchan TCP
    fn process_info(&self, pid: u32) -> Option<ProcessInfo>;
    fn signal(&self, pid: u32, sig: Signal) -> Result<()>;
}
trait ContainerSource {          // implementación v1: API de Docker (bollard o equivalente)
    fn list(&self) -> Result<Vec<Container>>;
    fn events(&self) -> Stream<ContainerEvent>;
    fn stop(&self, id: &str, timeout_s: u32) -> Result<()>;
}
trait Clock { fn now(&self) -> Instant; }   // reloj inyectable para pruebas de vencimiento
```
`ListeningProcess`: `pid`, `start_time`, `ppid`, `pgid`, `uid`, `argv`, `cwd` (opcional), `ports` (lista de puertos TCP en escucha).
**Un proceso se identifica por `(pid, start_time)`**, nunca solo por `pid`, para resistir la reutilización de PIDs **[D]**.

## 4. Observación

### Procesos del host (macOS) **[P]**
- Cada `host_interval` (1,5 s por defecto): listar PIDs del usuario (`proc_listpids`), y por cada uno sus descriptores (`proc_pidinfo` con `PROC_PIDLISTFDS`); para los de tipo socket, `proc_pidfdinfo` con `PROC_PIDFDSOCKETINFO` y quedarse con TCP en estado LISTEN. Directorio de trabajo: `PROC_PIDVNODEPATHINFO`. Argumentos: `sysctl KERN_PROCARGS2`. Padre, grupo, hora de inicio, uid: `PROC_PIDTBSDINFO`.
- Solo procesos del mismo usuario (no requiere privilegios).
- **Plan B** si el costo de escanear todos los procesos supera el presupuesto: `lsof -nP -iTCP -sTCP:LISTEN -Fpn` como respaldo, menos eficiente. Se decide con la medición de la Fase 0.
- Un proceso que vive menos de ~1,5 s no se observa; es irrelevante para servidores de desarrollo **[C]**.

### Contenedores **[C]** (ADR-012)
- Arranque: `docker context inspect` para obtener el endpoint activo (Docker Desktop, OrbStack, Colima). Lista inicial de contenedores y luego **stream de eventos** (`start`, `die`, `stop`, `destroy`). Reintento con espera creciente si el daemon no responde; mientras tanto, los contenedores se muestran como "desconocido", no como caídos.
- Proyecto de un contenedor: etiquetas de compose, o `docker:` del manifiesto.
- Acción única: `stop` (timeout 10 s) **[C]**.

### Reconciliación
Cada ciclo: tomar la foto, compararla con las instancias conocidas (clave `(pid, start_time)` o `container_id`) y generar diferencias: aparece, desaparece, cambia de puerto. Cada diferencia relevante produce un evento en el log.

## 5. Identidad y clasificación

Reglas de MANIFIESTO.md secciones 3 y 7. Para cada instancia observada:
1. Si pertenece a una reserva (instancia registrada) → se vincula (sección 7).
2. Si no, se intenta asociar a un servicio por directorio de trabajo y `match`.
3. Si no cae bajo ningún proyecto → "sin proyecto".

**Quién es la Principal de un servicio** **[D]** (hallazgo: David suele partir el día con un servidor levantado a mano o por script, sin pasar por devports; si no contara como Principal, el siguiente `run` crearía un duplicado, que es justo el problema):
1. La Principal **registrada** (vía `devports run`), si existe.
2. Si no, entre las instancias observadas que coinciden con el servicio: la que escucha en el `port` preferido; si no hay, la de `start_time` más antiguo. Se muestra como Principal con `registered: false`.
3. Las demás coincidencias del mismo servicio son **duplicados sin registrar** (ADR-014, caso 2).

Los Temporales registrados nunca son duplicados.

## 6. Instancias: estados **[D]**

```
 reserved ──(se vincula a un proceso o contenedor)──► running ──(vence / stop)──► stopping ──► stopped
    │                                                    │                           └──(no muere)──► expired_alive  ("Olvidados")
    ├──(nadie abre el puerto en 60 s, o el comando termina sin abrirlo)──► released
    └──(otro proceso toma el puerto)──► released + evento "puerto ajeno"

 running ──(el proceso desaparece)──► down      (la Principal queda "caída"; sigue en el manifiesto)
 running ⇄ salud: healthy | unhealthy  (campo aparte, no cambia el estado)
```
Temporales: 60 s antes de `expires_at` se emite el aviso una sola vez (`warned`).

## 7. Registro y vínculo por puerto reservado **[C]** (ADR-010) con reglas **[D]**

`devports run` envía la petición de registro antes del `exec`. El núcleo decide (CLI.md sección 3). Si acepta:
1. Reserva el puerto `P` (verificando que se puede abrir) y crea la instancia en `reserved`, anotando el PID del lanzador `L`. **`exec` conserva el PID**, así que el proceso del comando es `L` o un descendiente de `L`.
2. Espera un listener en `P` y lo vincula por reglas en orden:
   - **Contenedor:** si el `ContainerSource` informa un contenedor publicando `P` con creación posterior a la reserva → se vincula al contenedor (y nunca a un proceso del host que sea el proxy del runtime de Docker).
   - **Vínculo fuerte:** proceso del host con `P` en escucha que es `L` o descendiente de `L`, con `start_time` posterior a la reserva.
   - **Vínculo medio:** proceso del mismo usuario con `P` en escucha, `start_time` posterior a la reserva y directorio de trabajo dentro del proyecto (cubre procesos que se desprenden y pierden el ancestro).
   - **Ninguno cumple:** no se vincula. Evento "puerto tomado por un proceso ajeno", notificación, y la reserva pasa a `released`. Nunca se ata un vencimiento a un proceso que no cumple las reglas.
**Verificado en T0.8 (V9)** con procesos reales en macOS (`crates/devports-core/tests/macos_exec_ancestry.rs`): (a) con `sh -c 'exec <comando>'` el servidor **tiene el mismo pid** que el lanzador (`$$` coincide con el pid con que el padre lanzó el proceso, también con el Python de Homebrew, que se relanza a sí mismo); (b) un servidor que bifurca es descendiente directo del lanzador (`ppid = L`); (c) un servidor desprendido (el padre termina) queda con `ppid = 1` y la cadena de ascendencia **no** llega al lanzador: ahí solo rige el vínculo medio; (d) un servidor lanzado desde una shell no interactiva **hereda el grupo de procesos de quien lo lanzó** (`pgid` distinto de su pid), lo que confirma que parar un grupo mataría al lanzador. El módulo `process::ancestry` (`chain`, `is_self_or_descendant`) calcula la ascendencia comparando `(pid, start_time)`, corta ante un pid reutilizado (un padre que arrancó después que su hijo), ciclos y profundidad mayor de 64.

3. Si el lanzador `L` termina sin que haya contenedor ni listener y pasó la gracia, o si pasan 60 s, la reserva pasa a `released`.

## 8. Vencimientos y parada **[C]** con detalle **[D]**

Un temporizador de 1 s revisa los Temporales `running`:
- `expires_at - 60 s` y no avisado: evento y notificación "vence en 1 minuto" con Extender 30 min.
- `expires_at` alcanzado: procedimiento de parada.

**Parada de un proceso del host** (solo si el vínculo es fuerte o medio):
1. Verificar que `(pid, start_time)` sigue siendo el mismo proceso (protección contra reutilización de PID).
2. Calcular el conjunto `{pid} ∪ descendientes` recorriendo la tabla de padres.
3. Enviar `SIGTERM` a cada uno de ese conjunto. **Nunca a un grupo de procesos entero** salvo que `pgid == pid` y todos los miembros del grupo pertenezcan a ese conjunto (en una shell no interactiva el grupo se hereda del padre y señalarlo mataría la terminal o al agente).
4. Esperar 5 s **[P]**; a los que sigan vivos, `SIGKILL`; esperar 2 s.
5. Si algo sigue vivo: estado `expired_alive`, evento `stop_failed`, y aparece en "Olvidados". No se reintenta en bucle.

**Prohibiciones absolutas:** no señalar procesos de otro usuario, ni el proceso del propio núcleo, ni sus ancestros, ni el PID 1, ni procesos que no sean el vínculo de una instancia registrada.

**Contenedor:** `docker stop` con timeout 10 s. No `rm`, no volúmenes **[C]**.

**Persistencia:** los vencimientos están en SQLite. Si el núcleo se reinicia, al arrancar reconcilia y ejecuta de inmediato la parada de cualquier Temporal vencido que siga vivo.

**Extensión:** `extend` suma tiempo sin superar `max_total` (4 h) acumulado por instancia; `warned` se reinicia.

## 9. Datos: SQLite **[D]**

Ubicación: `~/Library/Application Support/devports/devports.db`. Modo WAL. Un solo escritor (el núcleo).

```sql
CREATE TABLE schema_meta (version INTEGER NOT NULL);

CREATE TABLE projects (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, root TEXT NOT NULL,
  manifest_hash TEXT, loaded_at INTEGER NOT NULL
);

CREATE TABLE instances (
  id INTEGER PRIMARY KEY,
  ref TEXT NOT NULL UNIQUE,                 -- referencia corta y pública
  project TEXT, service TEXT,               -- NULL si "sin proyecto"
  role TEXT NOT NULL CHECK (role IN ('principal','temporal','unregistered')),
  registered INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL CHECK (state IN
    ('reserved','running','stopping','stopped','released','expired_alive','down')),
  port INTEGER,
  pid INTEGER, pid_start INTEGER,           -- (pid, hora de inicio) identifican el proceso
  container_id TEXT,
  cwd TEXT, branch TEXT, worktree TEXT,
  launcher_pid INTEGER, launcher_chain TEXT, -- cadena de ancestros, JSON
  reason TEXT,
  token_hash TEXT,                          -- SHA-256 del token; solo Temporales
  reserved_at INTEGER, started_at INTEGER, expires_at INTEGER,
  leased_total_s INTEGER NOT NULL DEFAULT 0,
  warned INTEGER NOT NULL DEFAULT 0, healthy INTEGER,
  ended_at INTEGER, end_reason TEXT
);
CREATE INDEX idx_instances_active ON instances (state, project, service);

CREATE TABLE events (
  id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, kind TEXT NOT NULL,
  project TEXT, service TEXT, instance_ref TEXT, payload TEXT   -- JSON
);
CREATE INDEX idx_events_ts ON events (ts);

```
Tipos de evento (`events.kind`): `core_started`, `run_accepted`, `run_rejected`, `reuse`, `reservation_released`, `bound`, `port_stolen`, `lease_warned`, `lease_extended`, `lease_expired`, `stopped`, `stop_failed`, `duplicate_detected`, `unregistered_seen`, `process_gone`.
Retención de eventos: `events_retention_days` (30) **[P]**.

## 10. IPC **[D]**

- **Socket:** `~/Library/Application Support/devports/devports.sock`. Directorio con permisos `0700`, socket `0600`. Sin puerto TCP propio **[C]**. Al arrancar, si el socket existe y responde, otra instancia ya corre: salir; si no responde, borrarlo. **Longitud de ruta [D]:** macOS limita la ruta de un socket Unix a unos 104 bytes; si la ruta por defecto la supera (usuario de nombre largo), se usa un respaldo bajo el directorio temporal del usuario con los mismos permisos, y `DEVPORTS_SOCKET` lo sustituye siempre. Se prueba en T1.1.
- **Autenticación:** el núcleo verifica la credencial del par (`LOCAL_PEERCRED`): el uid debe ser el del usuario.
- **Protocolo:** líneas JSON (una petición, una respuesta). Petición: `{"v":1,"id":"<uuid>","op":"<operación>","args":{...}}`. Respuesta: `{"v":1,"id":"<uuid>","ok":true,"result":{...}}` o `{"ok":false,"error":{"code":"<código>","message":"...","data":{...}}}`.
- **Cotas del protocolo [D]:** línea máxima de 1 MiB, timeout de lectura y escritura por conexión, número máximo de conexiones simultáneas, y el campo `v` validado (versión desconocida: error `bad_version`). Una línea que excede la cota cierra la conexión con error. Valores exactos en `config.yml` o constantes del crate, fijados en T1.1.
- **Operaciones expuestas en el socket (solo lo que el CLI puede hacer, ADR-009):** `ping`, `run.register`, `ls`, `stop` (con token), `extend` (con token), `log`, `adopt.propose`.
- **Operaciones que NO existen en el socket** (solo accesibles desde la interfaz, que corre en el mismo proceso): detener una Principal o cualquier instancia no registrada, escribir un manifiesto, marcar como legítimo. Así el permiso se aplica por construcción y no por una bandera del CLI. **Límite honesto:** un agente con acceso al shell del usuario puede ejecutar `kill`; devports no puede impedirlo, solo no ofrece esa capacidad por su API **[D]**.
- **Token:** 128 bits aleatorios; el núcleo guarda solo su hash SHA-256. El token se entrega una vez al registrar el Temporal y no aparece en `ls` ni en `log` **[D]**.

## 11. Arranque, supervisión y recuperación **[C]** (ADR-015) con detalle **[P]**

- **Inicio de sesión:** agente de launchd en `~/Library/LaunchAgents/<BUNDLE_ID>.plist` con `RunAtLoad` y `KeepAlive` con `SuccessfulExit: false`. Si el proceso termina con error, launchd lo reinicia. "Salir" desde el menú termina con código 0 y no se reinicia. **Verificado en T0.7** con un trabajo real de launchd (`tools/spike-launchd.sh`): arranca con `RunAtLoad`; tras un `SIGKILL` vuelve a levantarse en unos 4 s con `ThrottleInterval` de 5 s (el valor por defecto, 10 s, lo alargaría); tras `exit(0)` no se reinicia (launchd informa `last exit code = 0`); `bootout` lo detiene. **El plugin de autoarranque de Tauri no sirve**: su crate `auto-launch` solo escribe `RunAtLoad`, sin `KeepAlive`. La instalación del agente queda para T2.6 (ver ADR-025, punto 7).
- **Instancia única:** garantizada por el socket (sección 10).
- **Recuperación:** al arrancar, cargar SQLite, reconciliar con la observación actual, marcar `down` lo que ya no existe, ejecutar la parada de Temporales vencidos aún vivos, reabrir el flujo de eventos de Docker.
- **Falla abierto del CLI:** CLI.md sección 3.

## 12. Interfaz (Tauri v2 + React) **[C]** estructura, **[P]** detalles de ventana

- **Ícono de barra:** imagen de plantilla; texto junto al ícono con el contador de atención (duplicados sin registrar + Temporales por vencer) si es mayor que 0. Política de activación "accessory" (sin ícono en el Dock).
- **Clic:** crea el panel (aprox. 380 x 520 px), anclado bajo el ícono (plugin de posicionamiento); al perder el foco o cerrarse, **se destruye** el webview (consumo mínimo en reposo) **[C]**.
- **Clic derecho:** menú nativo (Abrir panel, Salir).
- **Contenido del panel:**
  - *Atención:* Temporales por vencer (botones Extender 30 min y Terminar), duplicados sin registrar (botones Detener, Conservar, Marcar como legítimo).
  - *Proyectos:* por proyecto, sus servicios con estado (Principal sana, caída, colgada), URL (abrir en el navegador), Detener, Copiar comando de inicio; debajo, sus Temporales con cuenta regresiva, motivo y atribución.
  - *Compartidos:* servicios de `shared.yml` con los proyectos que los usan; al detener uno se muestran primero esos proyectos.
  - *Sin proyecto:* procesos y contenedores fuera de todo proyecto, con "Adoptar".
  - *Actividad:* registro de hoy (ADR-014).
- **Datos:** el núcleo empuja instantáneas a la interfaz por eventos de Tauri; React no contiene lógica de negocio.
- **Notificaciones:** `tauri-plugin-notification`. Solo los dos casos de ADR-014. **A verificar:** si macOS permite botones de acción ("Extender 30 min") desde el plugin; si no, la notificación abre el panel enfocado en ese elemento.
- **Detener una Principal** (acción de la persona): confirmación de un clic; si es `kind: data` o compartida, se muestra la advertencia con los proyectos afectados.

## 13. Seguridad

1. Socket local con permisos estrictos y verificación del uid; sin red **[C]**.
2. Sin telemetría ni llamadas de red propias **[D]**.
3. `.devports.yml` es datos: leerlo no ejecuta nada. `devports run` ejecuta `start` con los permisos del usuario, igual que `npm run`; un manifiesto de un repo ajeno clonado es tan confiable como su `package.json` **[D]**.
4. `adopt` desde el CLI es solo lectura para que un agente no pueda legitimar duplicados **[D]**.
5. Reglas de parada de la sección 8 (nunca señalar grupos ajenos ni procesos no vinculados).
6. El token de un Temporal vive en el entorno del proceso hijo: cualquier proceso que pueda leerlo (mismo usuario) puede detener esa prueba. Es aceptable: es la capacidad que se quiso dar **[D]**.

## 14. Rendimiento **[P]**

Presupuesto de la Fase 0 **[C]** el criterio, **[P]** los números: menos de 50 MB en reposo y menos de 1 % de CPU (panel cerrado). Contribuyentes a medir: escaneo cada 1,5 s, SQLite, stream de Docker, observador de archivos de las raíces. Palancas si no se cumple: subir el intervalo a 2 s, escanear solo procesos con `uid` propio y excluir los de sistema por nombre, limitar la profundidad del observador. Si aun así no se cumple, o el panel anclado resulta deficiente, evaluar Swift solo para la envoltura (el núcleo y el socket no cambian) **[C]**.

## 15. Puntos a verificar en la Mac de David (Fase 0)

| # | Verificación | Qué decide |
|---|---|---|
| V1 | `libproc` lista procesos en escucha del mismo usuario sin privilegios | Viabilidad de la observación sin `lsof` |
| V2 | `cwd` y `argv` disponibles para `node`, `php`, `python` | Reconocimiento de procesos no registrados |
| V3 | Costo de CPU del escaneo completo cada 1,5 s | Intervalo y Plan B |
| V4 | `docker context inspect` y stream de eventos en el runtime que usa David | Soporte de Docker |
| V5 | Tauri: contador junto al ícono, panel anclado, política accessory, destrucción del webview libera memoria | Si Tauri sirve para la envoltura |
| V6 | Botones de acción en notificaciones de macOS | Diseño de la notificación de vencimiento |
| V7 | launchd `KeepAlive` con la app de Tauri; "Salir" no reinicia | Supervisión |
| V8 | Latencia de `open -g -b` para el autoarranque del CLI | Falla abierto |
| V9 | `exec` conserva el PID y la ascendencia usada para el vínculo fuerte | Vínculo por puerto reservado |
| V10 | Memoria y CPU en reposo contra el presupuesto | Criterio de salida de la Fase 0 |
| V11 | Velocidad del descubrimiento bajo `~/developer` con 3 niveles | Valores por defecto de raíces |
| V12 | Qué runtime de Docker usa David y si usa compose o `docker run` | Pruebas de Docker |

## 16. Integración con el ecosistema de David (Fase 3)

- `ci4-kickstart` genera `.devports.yml` y un `start-all.sh` basado en `devports run --reuse` (CLI.md sección 10), e inserta el bloque de `AGENTS.md`/`CLAUDE.md` (CLI.md sección 9).
- CI4 Ops Platform: `PROJECT.md` sigue siendo la metadata del proyecto; `~/.ci4-ops/projects.yml` puede aportar raíces con `import_ops_registry`.
- El MySQL único de David se declara una vez en `shared.yml` y cada proyecto lo cita con `uses: [mysql]`.
