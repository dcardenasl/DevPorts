# DevPorts: Snapshot
> Actualizado: 2026-10-17 · Detalle completo: [ASSET.md](ASSET.md)

## ⚡ Retomar en 30 segundos

| Campo | Valor |
|---|---|
| **Etapa** | 📝 v0.0, especificado, sin código |
| **Última sesión** | 2026-10-17 |
| **¿Dónde quedé?** | Definición completa (24 decisiones; ADR-023 y 024 cerraron `--replace`, Principal de hecho y raíz) y plan de implementación en `TASKS.md`. Fase B completa y T0.1 hecha: repositorio git (`main` + `dev`), docs saneados, hooks, workspace de 4 crates con lints estrictos, CI y ADR-025. T0.2 hecha: `ProcessSource` de macOS verificada con servidores reales (V1, V2, V3 favorables) y sonda `devports-probe`. T0.3 hecha: manifiesto mínimo, huella de comando, descubrimiento y asignación de procesos a servicios (V11 favorable). T0.4 hecha: fuente de contenedores de Docker con eventos verificada contra el demonio real (V4 favorable; V12: Docker Desktop, compose y `docker run`). T0.5 hecha: spike de Tauri; reposo 21 MB, con panel usado 49 MB (presupuesto justo); anclaje bajo el ícono pendiente de comprobar a mano. T0.6 hecha: el plugin admite botones de acción en macOS; falta que David acepte el permiso y pruebe el botón (T0.9). T0.7 hecha: launchd con `KeepAlive` verificado de verdad (reinicio tras crash en ~4 s, sin reinicio con código 0) y `open -g -b` en 40 a 130 ms. T0.8 hecha: `exec` conserva el pid y la ascendencia se calcula (V9 favorable). Script de línea base listo, probado solo en Linux. |
| **Próximo paso** | Seguir `TASKS.md`: Fase 0 construida y endurecida (H0); esperando a David (S0, uso de la sonda, comprobaciones manuales y puerta G0). David: `./tools/baseline.sh once` y luego `start` en la Mac (tarea S0). |
| **Bloqueante activo** | Ninguno |
| **Decisión pendiente** | Nombre y licencia en la Fase 4. |

## ⏳ Pendiente de escribir
> Decisiones confirmadas en conversación que no llegaron a un artefacto formal.

*(vacío)*

---

## 📁 Archivos de detalle

| Archivo | Contenido | Cuándo cargarlo |
|---|---|---|
| [TASKS.md](TASKS.md) | Cola de trabajo, puertas y protocolo para retomar | Siempre |
| [CLAUDE.md](CLAUDE.md) | Reglas del proyecto y cómo retomar en frío | Al iniciar sesión |
| [ASSET.md](ASSET.md) | Ficha completa del asset | Al trabajar en el asset |
| [docs/verificacion-fase0.md](docs/verificacion-fase0.md) | Resultado de V1 a V12 y lo que falta para la puerta G0 | Antes de decidir G0 |
| [docs/DECISIONES.md](docs/DECISIONES.md) | ADR-001 a ADR-025 y puntos abiertos (fuente de verdad) | Siempre antes de implementar |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Fases, tareas y puertas | Para planificar |
| [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md) | Diseño técnico y verificaciones V1 a V12 | Al construir |
| [docs/CLI.md](docs/CLI.md) | Comandos, códigos de salida, mensajes | Al construir el CLI |
| [docs/MANIFIESTO.md](docs/MANIFIESTO.md) | Esquemas YAML y algoritmos de identidad | Al construir el núcleo |
| [docs/PRUEBAS.md](docs/PRUEBAS.md) | Estrategia y catálogo de pruebas | Al escribir pruebas |
| [docs/PRD.md](docs/PRD.md) | Requisitos y criterios de aceptación | Para validar alcance |
