# Plan — Docker es opcional (UXR-12)

> Fuente de las tareas UXR-12.x de [TASKS.md](../../TASKS.md#docker-opcional). Decisión: ADR-043 en [DECISIONES.md](../DECISIONES.md). Origen: pregunta de David del 2026-10-18 («si no uso Docker, ¿está bien que insista en que no está activado?»).

## Problema
El núcleo consulta Docker cada vez que la conexión falla (reintento con espera de 2, 5, 15 y 30 s) y publica `docker_error` en el estado. El panel convierte ese error en la franja «Docker no responde» en las cinco pestañas, sin importar si algo depende de Docker. Quien no usa Docker, o lo tiene apagado sin tener nada declarado en él, ve un aviso que no puede resolver ni descartar. Además no hay forma de silenciarlo.

## Qué se comprobó en el código
- `crates/devports-core/src/runtime.rs` (`supervise_docker`): cualquier fallo de conexión produce `Msg::Containers(Err(DockerProblem))`; el motor llama a `observe_containers(None)`.
- `crates/devports-core/src/engine/views.rs` (líneas ~310–325 y ~620–630): una instancia pasa a `InstanceState::Unknown` **si y solo si** es un servicio de contenedor (`container_id` o `container` declarado) y `containers` es `None`, es decir, Docker inalcanzable. «Unknown» no lo produce ninguna otra causa.
- `crates/devports-core/src/docker/context.rs`: el socket se resuelve leyendo el almacén de contextos de Docker (`DOCKER_HOST`, `DOCKER_CONTEXT`, `~/.docker/config.json`), sin procesos ni rutas fijas (ADR-012, ADR-025.3).
- Un MySQL de Homebrew no depende de Docker: lo observa el escaneo de puertos y el clasificador lo trata como «posible herramienta de apoyo» (`classify.rs`, lista `DAEMONS`).

## Decisión (ADR-043)
**El aviso de Docker solo aparece cuando algo que el usuario declaró depende de Docker y no se puede observar.** Esa dependencia ya llega en el estado como instancias con `state === "unknown"`; no hace falta ningún dato nuevo del núcleo.

| Situación | Franja en el panel | Línea «Docker» en Configuración |
|---|---|---|
| Docker responde | ninguna | «Responde» |
| Docker no responde y ninguna instancia está `unknown` (no se usa) | **ninguna** | «No responde · ningún servicio declarado lo usa» (dato, no alarma) |
| Docker no responde y hay instancias `unknown` | franja actual, con el número de servicios afectados | «No responde: motivo» |

### Alternativas descartadas
- **Detectar «Docker no instalado»** buscando `/Applications/Docker.app`, `~/.orbstack`, `~/.colima`, `/usr/local/bin/docker`…: son rutas a mano para cada runtime (Desktop, OrbStack, Colima, Rancher, Podman); cualquier runtime nuevo o instalación fuera de sitio se clasificaría mal. Contradice ADR-012 (sin rutas a mano). La regla elegida no necesita saber si Docker existe, solo si algo lo necesita.
- **Ajuste «No uso Docker»** en Configuración: sería estado persistente (preferencia, migración de la base, tipos en `proto`, bindings) que la regla elegida vuelve redundante: quien no declara servicios de Docker no ve nada, y quien los declara necesita saberlo. Menos estado, menos deriva.
- **Dejar de consultar Docker** cuando falla: el sondeo ya cuesta un `connect` local cada 30 s como máximo y es lo que detecta que Docker arrancó; apagarlo exigiría un reinicio de la app.
- **Cambiar el núcleo (nuevo `DockerProblem::NotInstalled`)**: añadiría un contrato y bindings para decir una cosa que el panel puede deducir con los datos que ya recibe.

## Primera apertura (comportamiento documentado)
- Sin configuración previa: no hay `shared.yml` ni proyectos; la app lee procesos que escuchan puertos y consulta Docker.
- MySQL en Docker: aparece como contenedor «sin proyecto» hasta declararlo en `~/.config/devports/shared.yml` o adoptarlo (ADR-013). Al declararlo pasa a poder estar `unknown` con Docker apagado, y entonces la franja tiene sentido.
- MySQL de Homebrew o nativo: aparece en «Sin proyecto» como posible herramienta de apoyo; no depende de Docker y nunca dispara el aviso.

## Límites
Solo `ui/`. Ningún cambio en Rust, `proto`, bindings, base de datos ni comandos de Tauri. Textos nuevos en `es.ts` y `en.ts`.

## Tareas
| ID | Contenido | Depende de |
|---|---|---|
| UXR-12.0 | Este plan, ADR-043 y las fichas | — |
| UXR-12.1 | Regla en el modelo y en la franja de estado | 12.0 |
| UXR-12.2 | Configuración: línea de Docker según el uso | 12.1 |
| UXR-12.3 | Verificación nativa, documentación de la primera apertura y cierre | 12.1, 12.2 |

## Riesgos
- Una instancia `unknown` desaparece de la lista cuando Docker responde: la franja se va sola; la prueba lo fija.
- Al arrancar, antes del primer sondeo, `docker_error` es nulo: la franja ya exigía error; se conserva esa condición.
- Un servicio de contenedor que nunca se declaró no aparece como `unknown` (no existe como instancia): con Docker apagado no hay franja y es correcto, no hay nada que mostrar.

## Uso de /commit-flow
Una tarea = 1 o 2 commits (`feat/fix(ui)` → `test(ui)` → `docs`). Sin push.
