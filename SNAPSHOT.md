# DevPorts: Snapshot
> Actualizado: 2026-10-17 · Detalle completo: [ASSET.md](ASSET.md)

## ⚡ Retomar en 30 segundos

| Campo | Valor |
|---|---|
| **Etapa** | 🚧 v0.1 en pruebas: núcleo + CLI + app de barra construidos |
| **Última sesión** | 2026-10-17 |
| **¿Dónde quedé?** | Núcleo, CLI y app de barra construidos y verificados contra la máquina real (procesos, Docker Desktop, launchd). Las pruebas de PRUEBAS.md existen todas y una guarda las exige. Instalada en `~/Applications/DevPorts.app`. UXR-1.1 a UXR-1.3 y UXR-2.1 completas; quedan las correcciones UXR restantes y las puertas humanas: G1 (3 días de uso real, E1–E7, revisión del portafolio) y G2 (5 días de uso diario), más reiniciar el Mac una vez (M-15). |
| **Próximo paso** | Ejecutar [UXR-0](TASKS.md#uxr-0): identificar el bundle activo y recorrer la app nativa con recursos controlados; luego UXR-2.2. UXR-2.1 (`0970bd8`) unifica confirmación y revalidación exacta de toda parada; [entrega y límites](docs/audits/2026-10-17-uxr-2-1.md). UI 32/32 y `tools/check.sh full` verdes; las paradas host fueron solo sobre servidores desechables de prueba. Evidencia Tauri/VoiceOver nativa sigue en UXR-0/UXR-7.2. UXR-1.3: [entrega](docs/audits/2026-10-17-uxr-1-3.md); UXR-1.2: [entrega](docs/audits/2026-10-17-uxr-1-2.md); UXR-1.1: [entrega](docs/audits/2026-10-17-uxr-1-1.md). |
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
| [docs/DECISIONES.md](docs/DECISIONES.md) | ADR-001 a ADR-035 y puntos abiertos (fuente de verdad) | Siempre antes de implementar |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Fases, tareas y puertas | Para planificar |
| [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md) | Diseño técnico y verificaciones V1 a V12 | Al construir |
| [docs/CLI.md](docs/CLI.md) | Comandos, códigos de salida, mensajes | Al construir el CLI |
| [docs/MANIFIESTO.md](docs/MANIFIESTO.md) | Esquemas YAML y algoritmos de identidad | Al construir el núcleo |
| [docs/PRUEBAS.md](docs/PRUEBAS.md) | Estrategia y catálogo de pruebas | Al escribir pruebas |
| [docs/PRD.md](docs/PRD.md) | Requisitos y criterios de aceptación | Para validar alcance |
