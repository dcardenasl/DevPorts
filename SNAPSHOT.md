# DevPorts: Snapshot
> Actualizado: 2026-10-17 · Detalle completo: [ASSET.md](ASSET.md)

## ⚡ Retomar en 30 segundos

| Campo | Valor |
|---|---|
| **Etapa** | 🚧 v0.1 en pruebas: núcleo + CLI + app de barra construidos |
| **Última sesión** | 2026-10-17 |
| **¿Dónde quedé?** | Núcleo, CLI y app de barra construidos y verificados contra la máquina real (procesos, Docker Desktop, launchd). Las pruebas de PRUEBAS.md existen todas y una guarda las exige. Instalada en `~/Applications/DevPorts.app`. UXR-1 y UXR-2 completos; quedan las correcciones UXR restantes y las puertas humanas: G1 (3 días de uso real, E1–E7, revisión del portafolio) y G2 (5 días de uso diario), más reiniciar el Mac una vez (M-15). |
| **Próximo paso** | Ejecutar [UXR-3.1](TASKS.md#uxr-3-1): elegibilidad de adopción y edición validada de manifiestos, con comentarios y conflictos externos. UXR-2.3 (`423321c`) incorpora detalles reutilizables, estados reales y puertos observados; [entrega](docs/audits/2026-10-17-uxr-2-3.md). Controles completos, UI 41/41 y teclado a 380×520 verdes. UXR-0 sigue abierta por timeout de acceso al bundle; la app instalada no se actualizó. Tauri/VoiceOver, recuperación nativa y presupuesto con las correcciones quedan en UXR-7.2. |
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
