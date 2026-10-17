<p align="center">
  <img src="crates/devports-app/icons/icon.png" alt="DevPorts" width="96" height="96">
</p>

<h1 align="center">DevPorts</h1>

<p align="center">
  <strong>Un servidor oficial. Pruebas con vencimiento. Tu entorno en orden.</strong><br>
  Una app de barra de menús para macOS y un CLI para saber qué corre,
  de qué proyecto es y evitar que cada terminal o agente levante otro servidor.
</p>

**En pruebas de uso real.** Núcleo, CLI y panel construidos; pendientes las puertas
G1/G2 antes de cerrar la entrega. DevPorts sigue siendo un nombre de trabajo;
la licencia y la publicación aún no están decididas.

## Qué hace

Trabajar con varios repositorios, terminales y agentes termina dejando servidores
repetidos y puertos ocupados. DevPorts observa los procesos y contenedores que ya
están corriendo y los organiza por proyecto y servicio. Al ejecutar mediante
`devports run`, aplica una política común: una **Principal** por servicio y
**Temporales** con motivo y vencimiento para las pruebas.

- **Panel en la barra de menús.** Atención, Proyectos, Compartidos, Sin proyecto y
  Actividad; puertos, procesos, ramas y estado en un mismo lugar.
- **Prevención de duplicados.** Si ya existe una Principal sana, rechaza otro
  lanzamiento o permite reutilizarla con `--reuse`.
- **Pruebas con plazo.** Temporales de 30 minutos por defecto, hasta dos por
  servicio y un máximo acumulado de cuatro horas; aviso antes de vencer y extensión.
- **Procesos y Docker.** Reconoce servidores del host y contenedores del contexto
  activo, incluidos servicios compartidos como MySQL.
- **Adopción desde lo observado.** La app permite convertir procesos y contenedores
  existentes en servicios declarados en YAML. El CLI propone el manifiesto sin escribirlo.
- **Parada protegida.** Verifica la identidad del proceso antes de señalarlo;
  los servicios `kind: data` nunca se detienen automáticamente.
- **Inicio de sesión opcional.** Se activa desde el menú e incluye recuperación
  tras una caída; salir normalmente no provoca un reinicio.

DevPorts observa servicios existentes y ejecuta comandos cuando se lo pides.
No sustituye un supervisor como pm2 ni elimina contenedores o volúmenes.

## Instalar desde el código

Necesitas una Mac, las herramientas de línea de comandos de Xcode, Rust mediante
`rustup` (la versión está fijada en `rust-toolchain.toml`), Node.js y `pnpm`
compatibles con el frontend de `ui/package.json`. Docker es opcional para los
servicios del host; para contenedores necesitas un demonio accesible en el contexto activo.

Desde la raíz del repositorio:

```bash
tools/install.sh
```

Compila la interfaz y los binarios, crea el paquete con firma local e instala y
abre `~/Applications/DevPorts.app`. Si había una copia instalada, la reemplaza.
La firma es ad hoc; no equivale a una distribución notarizada por Apple.

Con clic derecho en el ícono de la barra:

1. Instala el comando `devports` (crea el enlace en `~/.local/bin/devports`).
2. Activa el inicio al abrir sesión si lo quieres.
3. Permite las notificaciones de macOS para recibir los avisos de vencimiento.

Si tu terminal no encuentra el comando, añade `~/.local/bin` al `PATH` de tu shell:

```bash
export PATH="$HOME/.local/bin:$PATH"
devports version
```

Para construir el paquete sin instalarlo: `tools/bundle.sh`. El resultado queda en
`target/bundle/DevPorts.app`; `tools/bundle.sh --debug` permite una compilación de desarrollo.

## Primer proyecto

Crea `.devports.yml` en la raíz de tu proyecto, o usa **Adoptar** desde el panel
para partir de un servidor que ya esté corriendo. Este ejemplo usa Python:

```yaml
version: 1
project: mi-proyecto
services:
  web:
    cwd: .
    start: python3 -m http.server ${PORT}
    port: 8000
    match: python3 -m http.server N
```

Desde esa carpeta:

```bash
devports run web          # inicia la Principal en el puerto preferido
devports ls              # consulta las instancias del proyecto
```

En otra terminal, `devports run web --reuse` reutiliza la Principal sana sin iniciar
otro proceso. Para una prueba independiente:

```bash
devports run web --as test --reason "comprobar cambios" --ttl 30m -- python3 -m http.server '{PORT}'
```

DevPorts asigna el puerto del Temporal. `${PORT}` se usa en `start` del manifiesto;
`{PORT}` en el comando explícito posterior a `--`.

Los proyectos se descubren bajo las raíces configuradas, hasta tres niveles por
defecto. Si trabajas fuera de la raíz predeterminada `~/developer`, configura
`~/.config/devports/config.yml`, por ejemplo:

```yaml
version: 1
roots:
  - ~/Developer
  - ~/Proyectos
```

El [manual de manifiestos](docs/MANIFIESTO.md) explica servicios PHP, Node,
Docker, comprobaciones de salud y servicios compartidos.

## CLI para personas y agentes

| Comando | Uso |
|---|---|
| `devports run web` | Inicia el servicio aplicando la política de duplicados |
| `devports run web --reuse` | Si hay una Principal sana, devuelve éxito sin iniciar otra |
| `devports ls --all` | Incluye instancias sin registrar y sin proyecto |
| `devports ls --all --json` | Consulta estructurada para herramientas |
| `devports log --since 2h` | Muestra actividad reciente |
| `devports adopt .` | Propone un manifiesto sin modificar archivos |
| `devports extend TOKEN --by 30m` | Extiende el Temporal autorizado por ese token |
| `devports stop TOKEN` | Detiene el Temporal autorizado por ese token |

`TOKEN` es el `DEVPORTS_STOP_TOKEN` de la instancia temporal, no su PID ni su
referencia visible. Los tokens no aparecen en las listas ni en el registro de actividad.

Por defecto el CLI no pregunta. Ante un conflicto devuelve un código de salida
estable; `--json` está disponible en `run`, `ls` y `log`. Para sesiones manuales
puede habilitarse el modo interactivo con `DEVPORTS_INTERACTIVE=1`.

Si la app está inactiva, `run` intenta abrirla. Si sigue sin responder, ejecuta el
comando sin protección y lo avisa: ese lanzamiento no tiene registro ni vencimiento
garantizado. Detalles y códigos en el [manual del CLI](docs/CLI.md).

## Configuración y datos

| Ruta | Contenido |
|---|---|
| `<proyecto>/.devports.yml` | Servicios del proyecto |
| `~/.config/devports/config.yml` | Raíces, límites, observación y retención |
| `~/.config/devports/shared.yml` | Servicios compartidos entre proyectos |
| `~/Library/Application Support/devports/devports.db` | Estado y actividad en SQLite |
| `~/Library/LaunchAgents/com.davidcardenas.devports.plist` | Inicio de sesión, cuando se activa |

La configuración global es opcional. Los valores predeterminados y las reglas de
validación están en [MANIFIESTO.md](docs/MANIFIESTO.md).

## Cómo está construido

| Componente | Responsabilidad |
|---|---|
| `devports-proto` | Contratos de comunicación compartidos |
| `devports-core` | Observación, identidad, política, vencimientos y SQLite; independiente de Tauri |
| `devports-cli` | Cliente liviano del núcleo, sin dependencia del núcleo ni de Tauri |
| `devports-app` | App Tauri v2 que aloja el núcleo, panel, notificaciones e integración con macOS |
| `ui` | Panel React + TypeScript + Vite, con tipos generados desde Rust |

El panel se crea al abrirlo y se destruye al cerrarlo. La lógica de las decisiones
vive en Rust. El [documento de arquitectura](docs/ARQUITECTURA.md) describe los
límites entre componentes, la comunicación local y las garantías de parada.

## Desarrollo y verificación

```bash
scripts/install-git-hooks.sh
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui run build
cargo run -p devports-app
```

Para probar el comando instalado, las notificaciones y el inicio de sesión, usa
el paquete construido con `tools/install.sh`.

```bash
tools/check.sh full                 # Rust, matriz de pruebas y cargo-deny
pnpm --dir ui run typecheck
pnpm --dir ui run lint
pnpm --dir ui run format:check
pnpm --dir ui test
```

`tools/check.sh full` requiere `cargo-deny` instalado. Las comprobaciones reales
de macOS y Docker se ejecutan aparte:

```bash
tools/docker-checks.sh              # requiere Docker activo
tools/launchd-check.sh              # supervisión y salida limpia
tools/measure.sh 120                # memoria y CPU durante 120 segundos
```

La CI comprueba `dev` con un flujo ligero y ejecuta las verificaciones completas
en Ubuntu y macOS al preparar un PR hacia `main`. El catálogo y la matriz de
cobertura están en [PRUEBAS.md](docs/PRUEBAS.md).

## Estado y próximos pasos

Las fases 1 y 2 están construidas. Queda validar su comportamiento cotidiano:

- **G1:** tres días de uso real, escenarios E1–E7 y revisión del portafolio documentada.
- **Interfaz:** confirmar o ajustar las decisiones provisionales de ADR-027.
- **Inicio de sesión:** reiniciar la Mac y comprobar el arranque automático (M-15).
- **G2:** cinco días de uso diario y cierre de las verificaciones antes del release.

La integración con `ci4-kickstart` corresponde a la Fase 3. Linux, Windows,
servidor MCP y adaptadores por agente quedan fuera de la v1. El presupuesto de
memoria y CPU debe comprobarse con la aplicación completa; las cifras del prototipo
no sustituyen esa medición.

## Documentación

- [Guía de lectura](docs/LEEME.md) y [requisitos del producto](docs/PRD.md).
- [Tareas y siguiente paso](TASKS.md), [estado de continuidad](SNAPSHOT.md) y [evidencia](ARCHIVES.md).
- [Decisiones](docs/DECISIONES.md), [arquitectura](docs/ARQUITECTURA.md) y [roadmap](docs/ROADMAP.md).
- [Historial de cambios](CHANGELOG.md) y [reglas para colaborar](CLAUDE.md).

## Licencia

Pendiente de decisión (ADR-020). El repositorio permanece privado y todavía no
se ha declarado una licencia de distribución.
