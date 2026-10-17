# DevPorts: Especificación del CLI

> Binario: `devports` (nombre de trabajo; ver ADR-020). Binario aparte y liviano, cliente fino del núcleo que corre dentro de la app de barra (ADR-015, ADR-017).
> Marcas: **[C]** confirmado. **[D]** derivado. **[P]** propuesta.
> Fuente de verdad ante contradicciones: `DECISIONES.md`.

---

## 1. Principios

1. **Determinista.** Ningún comando pregunta por defecto, llame quien llame (ADR-008) **[C]**.
2. **Falla abierto.** Si el núcleo no responde, `devports run` ejecuta el comando igual y avisa (ADR-018) **[C]**.
3. **Mínimo privilegio.** Quien llama solo puede detener o extender los Temporales que él registró (ADR-009) **[C]**.
4. **Legible para humanos y máquinas.** Salida de texto por defecto; `--json` para agentes y scripts **[D]**.
5. **Sin efectos fuera de `run`.** Solo `run` ejecuta comandos; `ls`, `log` y `adopt` son de solo lectura **[D]**.

## 2. Resolución de proyecto y servicio
- Proyecto: `--project <nombre>` o, si falta, el `.devports.yml` más cercano subiendo desde el directorio actual. Si no hay ninguno: código 14.
- Servicio: el nombre dado debe existir en `services` del manifiesto o ser un compartido citado en `uses`. Si no: código 14.

---

## 3. `devports run`

```
devports run <servicio> [--project <p>] [--reuse]
                        [--as test --reason "<motivo>" [--ttl <duración>]]
                        [--json] [-- <comando> [args...]]
```

### Qué comando se ejecuta
1. Si hay `--`, se usa ese comando (obligatorio para un servicio sin `start`).
2. Si no, se usa `start` del manifiesto.
3. Si no hay ninguno: código 13.

Sustitución de puerto: `${PORT}` en `start`; el token literal `{PORT}` en lo que sigue a `--` (el shell del que llama expandiría `$PORT` antes de que devports lo vea, ver MANIFIESTO.md sección 4) **[D]**. Los comandos se ejecutan con `sh -c 'exec <comando>'` para que el proceso final reemplace al shell; `start` debe ser un solo comando (para secuencias, usar un script) **[D]**.

### Decisión (tabla de situaciones, ADR-005, ADR-008)

| Situación | Sin `--as` | Con `--as test --reason "..."` |
|---|---|---|
| No hay instancia del servicio | Registra como **Principal** y ejecuta (`exec`) | Registra como **Temporal** y ejecuta |
| Hay Principal sana | **Rechazo, código 10**. Con `--reuse`: no inicia nada, imprime la instancia existente, **código 0** | Registra como **Temporal** y ejecuta (tope 2) |
| Hay Principal no sana (colgada) | **Rechazo, código 11** (informa; detener la instancia es una acción de la app, ADR-023) | Registra como **Temporal** y ejecuta |
| Temporales del servicio ya en el tope | n/a | **Rechazo, código 12** |
| Puerto preferido ocupado por un proceso ajeno al servicio | **Rechazo, código 18**, con quién lo ocupa | Se asigna otro puerto libre; no aplica |

- `--reuse` es la forma idempotente para scripts (`start-all.sh`) **[D]**.
- `--as test` exige `--reason`; sin él: código 16 **[D]**.
- `--ttl` acepta `30m`, `2h`, etc. Por defecto 30 min; máximo acumulado 4 h; si lo supera: código 16 **[C]** (valores).
- No existe `--replace` en la v1 (ADR-023): reemplazar una Principal es una acción de la persona desde la app; el rechazo lo indica.

### Puertos
- **Principal:** `port` del manifiesto si está definido y libre; si no está definido, un puerto libre del rango de Temporales.
- **Temporal:** puerto libre del rango `temporal_port_range` (por defecto 49152 a 65535), verificado antes de reservar. La reserva se libera si nadie lo abre en `reservation_grace` (60 s por defecto) **[P]**.

### Qué ocurre antes del `exec`
1. Conecta al socket del núcleo (timeout 300 ms **[P]**).
2. Envía: proyecto, servicio, rol (principal/temporal), motivo, vencimiento solicitado, directorio actual, comando, y su **cadena de procesos ancestros** (se calcula en el CLI porque aún está vivo) **[D]**.
3. Recibe la decisión. Si es aceptación: imprime a **stderr** un bloque informativo, exporta las variables de entorno y hace `exec`.
4. El núcleo vincula la instancia con lo que termine escuchando el puerto reservado (ver ARQUITECTURA.md).

### Bloque informativo en la aceptación (stderr)
```
devports: api (proyecto tienda-demo) como PRINCIPAL en http://localhost:8080
devports: api (proyecto tienda-demo) como TEMPORAL en http://localhost:51234, vence 14:50 (30 m)
DEVPORTS_STOP_TOKEN=9f1c...   (guárdalo: permite detener o extender esta prueba)
```
(El token solo aparece para Temporales.)

### Variables de entorno que recibe el proceso hijo
| Variable | Valor |
|---|---|
| `DEVPORTS_PROJECT`, `DEVPORTS_SERVICE` | Proyecto y servicio |
| `DEVPORTS_ROLE` | `principal` o `temporal` |
| `DEVPORTS_PORT` y `PORT` | Puerto asignado |
| `DEVPORTS_INSTANCE_REF` | Referencia corta pública (la que muestra `ls`) |
| `DEVPORTS_STOP_TOKEN` | Solo Temporales. Capacidad para `stop` y `extend` |
| `DEVPORTS_EXPIRES_AT` | Solo Temporales. Fecha y hora ISO 8601 |

### Mensaje de rechazo (código 10), texto
```
devports: conflicto en api (proyecto tienda-demo)
Ya corre la instancia Principal de "api":
  http://localhost:8080   pid 4121   rama feature/x   worktree /Users/.../tienda-api
  desde hace 2 h 10 min   sana
No se inició nada. Alternativas:
  Reutilizarla:         devports run api --reuse
  Levantar una prueba:  devports run api --as test --reason "<motivo>" -- <comando con {PORT}>
  Reemplazarla:         detén la instancia desde la app de barra y vuelve a ejecutar
```

### Rechazo con `--json`
```json
{
  "ok": false, "code": 10, "error": "conflict",
  "project": "tienda-demo", "service": "api",
  "existing": {
    "ref": "a1b2", "role": "principal", "url": "http://localhost:8080",
    "port": 8080, "pid": 4121, "branch": "feature/x",
    "worktree": "/Users/.../tienda-api",
    "started_at": "2026-10-16T12:00:00-03:00", "healthy": true
  },
  "alternatives": [
    {"kind": "reuse",   "command": "devports run api --reuse"},
    {"kind": "test",    "command": "devports run api --as test --reason \"<motivo>\" -- <comando con {PORT}>"},
    {"kind": "stop_in_app", "command": null, "needs_human": true}
  ]
}
```
Todas las respuestas `--json` llevan `ok`, `code` y, si `ok` es falso, `error` con uno de: `conflict`, `unhealthy`, `limit`, `no_command`, `not_found`, `bad_args`, `port_busy`, `no_port`.

### Éxito de `--reuse`
Código 0, sin `exec`. En texto: `devports: api ya corre (Principal) en http://localhost:8080 (pid 4121)`. En `--json`: `{"ok":true,"code":0,"reused":true,"instance":{...}}`.

### Modo interactivo opcional
Solo si `DEVPORTS_INTERACTIVE=1` **y** la entrada estándar es una terminal, un conflicto (código 10) pregunta: `[r]eutilizar / [t]emporal / [c]ancelar`. Con `t` pide el motivo (por defecto "prueba manual") y sigue como Temporal. Sin esa variable, nunca pregunta **[C]**.

### Falla abierto (núcleo inactivo)
1. Si el socket no responde, ejecuta una vez `open -g -b <identificador del paquete>` (salvo `DEVPORTS_NO_AUTOSTART=1`) y reintenta durante hasta 2 s **[P]**. Se usa el identificador (`-b`) y no el nombre (`-a`) porque no es ambiguo; en la Mac `open` vuelve en 40 a 130 ms y el proceso ya existe a los 60 a 160 ms (T0.7, V8).
2. Si sigue sin responder, imprime en stderr `devports: núcleo inactivo, sin protección` y ejecuta el comando igual, sin registrar nada. No hay código de salida propio: es el del comando.
3. Lo que se coló aparecerá luego como "sin registrar" por la detección **[C]**.

### Códigos de salida de `run` **[P]**
| Código | Significado |
|---|---|
| 0 | Éxito de `--reuse` (en otros casos el proceso es reemplazado por el comando y el código es el del comando) |
| 10 | Conflicto: ya existe una Principal sana |
| 11 | La Principal existe pero no está sana |
| 12 | Tope de Temporales alcanzado |
| 13 | No hay comando que ejecutar |
| 14 | Manifiesto o servicio no encontrado o inválido |
| 15 | Sin asignar (reservado; ADR-023 eliminó `--replace`, no se reutiliza) |
| 16 | Argumentos inválidos (`--as` sin `--reason`, `--ttl` fuera de rango, etc.) |
| 18 | Puerto preferido ocupado por un proceso ajeno |
| 19 | No se pudo asignar un puerto libre |
| 126, 127 | El comando no se pudo ejecutar o no existe (convención del shell) |

---

## 4. `devports ls`
```
devports ls [--project <p>] [--all] [--json]
```
Lista instancias. Por defecto, las del proyecto resuelto por la carpeta actual; si no hay, todos los proyectos. `--all` suma "sin registrar" y "sin proyecto".

Texto:
```
PROYECTO     SERVICIO  ROL        ESTADO    PUERTO  PID    RAMA       VENCE   REF
tienda-demo  api       principal  sana      8080    4121   feature/x  -       a1b2
tienda-demo  api       temporal   sana      51234   5530   feature/x  14:50   c3d4
tienda-demo  admin     principal  caida     8081    -      -          -       -
(sin registrar) tienda-demo/api  :8082  pid 6012  duplica a la Principal   e5f6
```
`--json`: arreglo de objetos con `ref`, `project`, `service`, `role` (`principal`, `temporal`, `unregistered`), `state` (`healthy`, `unhealthy`, `down`), `port`, `url`, `pid`, `branch`, `worktree`, `started_at`, `expires_at` (Temporales), `registered`, `launcher_chain` (cadena de ancestros), `duplicate_of` (ref de la Principal, si aplica).
**Nunca incluye `DEVPORTS_STOP_TOKEN`** **[D]**.

## 5. `devports stop <token>` y `devports extend <token> [--by <duración>]`
- `stop`: detiene el Temporal cuyo token coincide. Secuencia de parada en ARQUITECTURA.md. Código 0 si se detuvo o ya no existía; 30 si el token no corresponde a ningún Temporal **[P]**.
- `extend`: suma tiempo al vencimiento (por defecto 30 min), sin superar 4 h acumulado; código 31 si se superaría el máximo **[P]**.
- No existe ninguna forma de detener o extender una Principal, ni una instancia sin registrar, desde el CLI (ADR-009) **[C]**.

## 6. `devports log`
```
devports log [--today | --since <duración>] [--project <p>] [--json]
```
Muestra el registro de actividad (ADR-014): lanzamientos, rechazos con su motivo, Temporales creados, extendidos y vencidos, duplicados detectados, con la atribución disponible. Ejemplo:
```
14:02 api  rechazado   un agente (Cursor > zsh) intentó levantar otra instancia; la Principal sigue en :8080
14:20 api  temporal    :51234 para "QA login", vence 14:50
14:49 api  aviso       el temporal :51234 vence en 1 minuto
14:50 api  vencido     temporal :51234 detenido
```

## 7. `devports adopt [carpeta]`
Solo lectura: imprime la propuesta de manifiesto (MANIFIESTO.md sección 8) para la carpeta (por defecto, la actual). **No escribe archivos**: escribir un manifiesto es una acción de la persona, desde la app **[D]**. Motivo: "adoptar" podría legitimar duplicados, así que no queda al alcance de un agente.

## 8. Otros
- `devports version`, `devports help`.
- Variables de control: `DEVPORTS_INTERACTIVE=1` (pregunta opcional), `DEVPORTS_SOCKET=<ruta>` (otro socket, útil en pruebas), `DEVPORTS_NO_AUTOSTART=1` (no intentar abrir la app).

---

## 9. Integración con agentes: bloque para `AGENTS.md` y `CLAUDE.md` **[D]**
```markdown
## Servidores de desarrollo (devports)
- No levantes servidores con el comando directo (`npm run dev`, `php spark serve`, `docker run -p ...`). Usa `devports run <servicio>`.
- Antes de asumir qué corre, ejecuta `devports ls --json`.
- Si `devports run` responde "conflicto", ya existe el servidor oficial: usa la URL del mensaje, o `devports run <servicio> --reuse`. No levantes otro.
- Para una prueba con datos de prueba: `devports run <servicio> --as test --reason "<motivo>" -- <comando con {PORT}>`. Es temporal (vence a los 30 minutos). Guarda `DEVPORTS_STOP_TOKEN` y, al terminar, ejecuta `devports stop <token>`.
- Usa `{PORT}` en tus argumentos, nunca `$PORT`: el shell lo expandiría vacío.
- No detengas procesos que no levantaste tú. Detener una instancia oficial lo hace la persona desde la app.
```

## 10. Ejemplo: `start-all.sh` generado por `ci4-kickstart` (Fase 3) **[D]**
```bash
#!/usr/bin/env bash
cd "$(dirname "$0")"
devports run mysql --reuse
devports run api   --reuse &
devports run admin --reuse &
devports run bff   --reuse &
wait
```
Cada línea es idempotente: lo que ya corre no se vuelve a iniciar y lo que falta se levanta como Principal.
