# DevPorts: Snapshot
> Actualizado: 2026-10-16 · Detalle completo: [ASSET.md](ASSET.md)

## ⚡ Retomar en 30 segundos

| Campo | Valor |
|---|---|
| **Etapa** | 📝 v0.0, especificado, sin código |
| **Última sesión** | 2026-10-16 |
| **¿Dónde quedé?** | Definición completa (24 decisiones; ADR-023 y 024 cerraron `--replace`, Principal de hecho y raíz) y plan de implementación en `TASKS.md`. Repositorio git creado (`main` + `dev`). Script de línea base listo, probado solo en Linux. |
| **Próximo paso** | Seguir `TASKS.md` (cola de trabajo). David: `./tools/baseline.sh once` y luego `start` en la Mac (tarea S0). |
| **Bloqueante activo** | Ninguno |
| **Decisión pendiente** | Runtime de Docker y uso de compose (se mide en T0.4). Nombre y licencia en la Fase 4. |

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
| [docs/DECISIONES.md](docs/DECISIONES.md) | ADR-001 a ADR-025 y puntos abiertos (fuente de verdad) | Siempre antes de implementar |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Fases, tareas y puertas | Para planificar |
| [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md) | Diseño técnico y verificaciones V1 a V12 | Al construir |
| [docs/CLI.md](docs/CLI.md) | Comandos, códigos de salida, mensajes | Al construir el CLI |
| [docs/MANIFIESTO.md](docs/MANIFIESTO.md) | Esquemas YAML y algoritmos de identidad | Al construir el núcleo |
| [docs/PRUEBAS.md](docs/PRUEBAS.md) | Estrategia y catálogo de pruebas | Al escribir pruebas |
| [docs/PRD.md](docs/PRD.md) | Requisitos y criterios de aceptación | Para validar alcance |
