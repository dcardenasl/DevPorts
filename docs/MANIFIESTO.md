# DevPorts: Especificación de archivos de configuración

> Estado: v1 (esquema `version: 1`). Fuente de verdad ante contradicciones: `DECISIONES.md`.
> Marcas: **[C]** confirmado por David. **[D]** derivado de decisiones confirmadas. **[P]** propuesta a validar.

Hay tres archivos:

| Archivo | Ubicación | Qué contiene |
|---|---|---|
| Manifiesto de proyecto | `<carpeta del proyecto>/.devports.yml` | Servicios del proyecto **[C]** |
| Servicios compartidos | `~/.config/devports/shared.yml` | Servicios usados por varios proyectos, como un MySQL único **[C]** |
| Configuración | `~/.config/devports/config.yml` | Carpetas raíz, límites, retención **[C]** los valores marcados, **[P]** el resto |

Los tres son YAML. devports nunca ejecuta nada al leerlos: solo `devports run` ejecuta un comando, y lo hace con los permisos del usuario, igual que `npm run` (ver ARQUITECTURA.md, seguridad).

---

## 1. Manifiesto de proyecto: `.devports.yml`

Vive en la carpeta raíz del proyecto, que puede contener varios repos y no ser un repo (ADR-001).

### Ejemplo completo (proyecto CI4 del kit)
```yaml
version: 1
project: tienda-demo
uses: [mysql]                       # servicios compartidos (shared.yml)
services:
  api:
    cwd: ./tienda-api               # relativa a este archivo
    start: php spark serve --port ${PORT}
    port: 8080                      # puerto preferido de la Principal
    health: http:/health            # tcp (por defecto) | http:/ruta
    match: php spark serve          # huella; normalmente la completa "adoptar"
  admin:
    cwd: ./tienda-admin
    start: php spark serve --port ${PORT}
    port: 8081
  bff:
    cwd: ./tienda-bff
    start: npm run dev -- --port ${PORT}
    port: 5173
    match: node vite --port N
  cache:
    docker: tienda-redis            # contenedor suelto, en vez de cwd
    kind: data
    port: 6379
    start: docker start tienda-redis
```

### Campos

**Nivel superior**

| Campo | Tipo | Obligatorio | Regla |
|---|---|---|---|
| `version` | entero | sí | Debe ser `1`. Otro valor: error "versión no soportada". |
| `project` | texto | sí | Patrón `^[a-z0-9][a-z0-9-]{0,62}$`. Es el nombre mostrado y el usado por `--project`. |
| `uses` | lista de textos | no | Nombres definidos en `shared.yml`. Un nombre inexistente es advertencia, no error. La app la edita desde «Gestionar proyectos» (lista de una línea o en bloque, comentarios intactos); formas con comillas, anclas o varias líneas se rechazan antes de guardar y se editan a mano. Declara una asociación, no una conexión detectada. |
| `services` | mapa nombre a servicio | sí | Puede estar vacío. Cada nombre cumple el mismo patrón que `project` y es único. |

**Servicio**

| Campo | Tipo | Obligatorio | Por defecto | Regla |
|---|---|---|---|---|
| `cwd` | texto (ruta) | exactamente uno de `cwd` o `docker` | n/a | Relativa al manifiesto; se resuelve a ruta absoluta sin seguir enlaces fuera del proyecto. Servicios de host. |
| `docker` | texto | exactamente uno de `cwd` o `docker` | n/a | Nombre del contenedor. Servicios en contenedor. |
| `start` | texto | no | n/a | Un solo comando. Puede contener `${PORT}` (ver sección 4). Permitido también en servicios `docker` (por ejemplo `docker start nombre`). |
| `port` | entero 1 a 65535 | no | n/a | Puerto **preferido** de la Principal. No es identidad. Advertencia si es menor que 1024. Único dentro del proyecto. |
| `health` | texto | no | `tcp` | `tcp` o `http:/ruta` (la ruta empieza con `/`). Ver sección 5. |
| `kind` | `app` o `data` | no | `app` | `data` = con estado: jamás se detiene automáticamente (ADR-006). |
| `match` | texto | no | n/a | Huella de identidad (sección 3). Si falta, la identidad usa solo `cwd`/`docker`. |

### Reglas de validación
Errores (el manifiesto no se carga y se informa el motivo con número de línea):
- `version` distinta de `1`; `project` o nombres de servicio con formato inválido; nombres repetidos.
- Un servicio sin `cwd` ni `docker`, o con ambos.
- `port` fuera de 1 a 65535 o repetido entre servicios del mismo proyecto.
- `health` con formato inválido; `kind` distinto de `app` o `data`; `start` con saltos de línea.

Advertencias (se carga igual):
- Clave desconocida (compatibilidad futura). La clave `test` produce el mensaje: "ignorada: devports no gestiona configuración de pruebas" (ADR-010).
- `uses` con un nombre que no existe en `shared.yml`.
- `port` menor que 1024; `port` igual al preferido de un servicio de otro proyecto.

El manifiesto no tiene bloque `test:` ni puertos de Temporales. Los puertos de Temporales los asigna devports (ADR-010).

---

## 2. Servicios compartidos: `~/.config/devports/shared.yml` **[C]**

```yaml
version: 1
services:
  mysql:
    docker: mysql-dev
    kind: data
    port: 3306
    start: docker start mysql-dev
```
Mismo esquema de servicio que el manifiesto, salvo que el nombre del servicio es el que los proyectos citan en `uses`. Un servicio compartido pertenece a un grupo global "Compartidos", no a un proyecto. En la interfaz se lista junto a los proyectos que lo usan; antes de detenerlo, la app muestra esa lista (ADR-013). Un proyecto se asocia o desasocia desde el compartido o desde su propia tarjeta («Gestionar proyectos»); desasociar solo quita la línea de `uses`, no detiene ni borra nada. Si `shared.yml` declara `start`, la app ofrece copiarlo cuando el recurso está detenido; solo ofrece abrir el navegador si declara `port` y `health: http:/ruta`.

---

## 3. Huella de identidad (`match`) **[D]**, algoritmo **[P]**

Sirve para reconocer procesos **no registrados** (los que no pasaron por `devports run`). Los procesos registrados se asocian por el puerto reservado, no por `match` (ADR-010).

### Normalización de la línea de comandos
Se aplica igual al proceso observado y al valor de `match`:
1. Tomar `argv` completo del proceso.
2. `argv[0]`: quedarse con el nombre base (`/usr/bin/php` pasa a `php`).
3. Cada token que sea una ruta absoluta (empieza con `/` o `~`): quedarse con el nombre base.
4. Cada token numérico entre 1024 y 65535: reemplazar por `N`.
5. Cada token con forma `texto:número` (por ejemplo `localhost:3000`, `0.0.0.0:8080`): reemplazar el número por `N`.
6. Cada token `--opcion=número` en ese rango: reemplazar el número por `N`.
7. Unir con un espacio.

| Línea observada | Normalizada |
|---|---|
| `/usr/bin/php spark serve --port 8080` | `php spark serve --port N` |
| `node /Users/x/app/node_modules/.bin/vite --port=5173` | `node vite --port=N` |
| `python3 -m http.server 8001` | `python3 -m http.server N` |
| `php -S localhost:8000 -t public` | `php -S localhost:N -t public` |

### Precisiones de implementación (T0.3)
Decisiones que el algoritmo anterior dejaba abiertas; el código y sus pruebas (U-01, U-02, PT-01, PT-02) las fijan:
- El rango 1024 a 65535 aplica también a la forma `texto:número` y a `--opcion=número`; `localhost:80` no cambia.
- Cada token pasa por todas las reglas **en una sola pasada** (primero el nombre base de rutas, luego los puertos). Así `/opt/8080` pasa a `N` y la normalización es idempotente (PT-01).
- `argv[0]` se reduce a su nombre base aunque sea una ruta relativa, y **el programa se compara por su familia**: minúsculas y sin versión final (`python3`, `python3.14` y el `Python` en que se relanza el de Homebrew son la misma familia; `php8.2` es `php`). La familia solo se aplica cuando el primer token del `match` coincide con el `argv[0]` del proceso; en cualquier otra posición los tokens se comparan exactos. Así `match: python3 -m http.server N` reconoce lo que el kernel informa.
- Un `match` vacío no reconoce nada. Un `match` es una secuencia contigua de tokens, en el mismo orden.
- Las rutas de proyecto y de `cwd` se comparan **canonicalizadas** (`/var` y `/private/var`; mayúsculas y minúsculas en volúmenes que no las distinguen). Se compara por componentes de ruta: `/work/shop-old` no está dentro de `/work/shop`.
- Desempate de varios servicios: `match` con más tokens; luego el `port` en el que escucha el proceso; si persiste, ambiguo. Los servicios de contenedor (`docker`) nunca reciben procesos del host.

### Cuándo un proceso coincide con un servicio
Un proceso coincide con el servicio S del proyecto P si se cumplen **las dos**:
1. Su directorio de trabajo resuelto es igual a `S.cwd` o está dentro de él.
2. Si S tiene `match`: la línea normalizada del proceso contiene `match` (normalizado) como secuencia contigua de tokens.

**Ambigüedad:** si un proceso coincide con varios servicios, se elige el de `match` más largo; si empatan, el que tenga `port` igual al puerto en escucha; si persiste, queda "ambiguo" y se muestra como sin registrar con la advertencia (nunca se elige al azar).

**Dentro de un proyecto pero sin servicio:** el proceso aparece como "sin registrar en el proyecto P" y es candidato a "adoptar".

### Contenedores
Un contenedor coincide con el servicio S si `S.docker` es igual a su nombre. Los contenedores de compose, sin necesidad de manifiesto, se asignan al proyecto por las etiquetas `com.docker.compose.project` y `com.docker.compose.project.working_dir` (ADR-012).

---

## 4. Variables y sustitución

En `start` (manifiesto):
- `${PORT}` lo sustituye devports por el puerto asignado: el `port` preferido para la Principal; un puerto libre para un Temporal.

En el comando pasado después de `--` en `devports run`:
- El shell del que llama **expande `$PORT` y `${PORT}` antes de que devports vea los argumentos**, y quedaría vacío. Por eso devports sustituye el token literal **`{PORT}`** (sin `$`). Ejemplo: `devports run api --as test --reason "QA" -- php spark serve --port {PORT}`. **[D]**

El proceso hijo recibe además las variables de entorno listadas en CLI.md (`DEVPORTS_PORT`, `PORT`, etc.).

---

## 5. Salud (`health`)

- `tcp`: la instancia está sana si una conexión a `127.0.0.1:<puerto>` se establece en menos de 1 segundo.
- `http:/ruta`: sana si `GET http://127.0.0.1:<puerto>/ruta` responde con código 200 a 399 en menos de 1 segundo.
- Una instancia cuyo proceso vive pero no pasa el chequeo durante 3 comprobaciones seguidas se marca **no sana** (colgada). **[P]** (3 comprobaciones)
- Frecuencia: cada 5 segundos para instancias registradas y a demanda cuando `devports run` consulta. **[P]**

---

## 6. Configuración: `~/.config/devports/config.yml`

```yaml
version: 1
roots: ["~/developer"]        # [P] raíz por defecto (ADR-019); editable desde Configuración en el panel
max_depth: 3                  # [C]
import_ops_registry: false    # [P] importar ~/.ci4-ops/projects.yml (campo ruta_local)
lease:
  default: 30m                # [C]
  max_total: 4h               # [C] máximo acumulado por instancia
  extend_by: 30m              # [C]
  warn_before: 60s            # [C]
  max_per_service: 2          # [C] Temporales simultáneos
notifications:
  dedupe_window: 10m          # [C] una notificación por proyecto cada 10 minutos
observation:
  host_interval: 1500ms       # [P] "cada 1 a 2 segundos"
  temporal_port_range: [49152, 65535]   # [P]
  reservation_grace: 60s      # [P] reserva sin uso se libera
events_retention_days: 30     # [P]
known_agents:                 # [P] etiquetas para la cadena de ancestros; extensible
  claude: "Claude Code"
  cursor: "Cursor"
  codex: "Codex"
  Code: "VS Code"
```
Todas las claves son opcionales; los valores mostrados son los por defecto.

---

## 7. Descubrimiento de proyectos **[C]** (valores marcados en ADR-019)

1. Se recorren las `roots` hasta `max_depth` niveles buscando archivos `.devports.yml`. Se ignoran `node_modules`, `vendor`, `.git`, `target` y carpetas ocultas salvo la propia `.devports.yml`.
2. Cada hallazgo define un proyecto. Si un manifiesto contiene a otro, ambos son proyectos independientes (el más profundo manda para los procesos que caen en él).
3. Se vuelve a revisar ante cambios (observador de archivos sobre las raíces) y al abrir el panel. **[P]** (mecanismo)
4. Si `import_ops_registry` es `true`, las rutas `ruta_local` de `~/.ci4-ops/projects.yml` se suman como raíces.
5. Un proceso cuyo directorio de trabajo no cae bajo ningún proyecto se muestra "sin proyecto" con el botón "adoptar".
6. Proyecto sin manifiesto pero dentro de un repo git: respaldo, la raíz del repo es el proyecto provisional y la app sugiere crear el manifiesto (ADR-001).

---

## 8. "Adoptar": generar el manifiesto desde lo observado **[C]** (flujo), algoritmo **[D]**

Entrada: una carpeta (la raíz del proyecto, o cualquier carpeta si aún no hay manifiesto). Disponible como botón en la app. El CLI solo ofrece la propuesta de solo lectura (`devports adopt`, ver CLI.md).

1. Reunir las instancias observadas cuyo directorio de trabajo cae dentro de la carpeta, y los contenedores cuyo `working_dir` de compose cae dentro.
2. Para cada proceso del host proponer un servicio:
   - nombre: nombre de la carpeta de su `cwd` (en minúsculas, con guiones); si se repite, agregar `-<puerto>`;
   - `cwd`: ruta relativa a la carpeta;
   - `port`: el puerto en escucha (el menor, si hay varios);
   - `match`: la línea normalizada (sección 3);
   - `start`: la línea cruda con el número de puerto reemplazado por `${PORT}`, marcada "revisar" cuando el proceso observado parece ser hijo de un lanzador (por ejemplo `npm`, `composer`), porque la línea real que debe ejecutarse puede ser otra.
3. Para cada contenedor proponer `docker: <nombre>`, `port` publicado, y `kind: data` si la imagen es de una lista de bases de datos y cachés conocidas (`mysql`, `mariadb`, `postgres`, `redis`, `mongo`, `memcached`, `elasticsearch`). David confirma o corrige.
4. Mostrar la propuesta como diferencia contra el manifiesto actual (si existe) y escribir solo al confirmar.
5. **Escritura sin destruir:** si el manifiesto ya existe, agregar o actualizar únicamente los servicios afectados preservando comentarios y orden. No reescribir el archivo completo con un serializador que pierda comentarios.
6. "Marcar como legítimo" (ADR-002) ejecuta este mismo flujo para un solo proceso observado.

---

## 9. Versionado del esquema

`version: 1`. Cambios compatibles (claves opcionales nuevas) no cambian el número. Un cambio incompatible sube a `2` y devports mantiene la lectura de `1` con un aviso de migración. Un archivo con versión mayor que la soportada se rechaza con el mensaje "actualiza devports".
