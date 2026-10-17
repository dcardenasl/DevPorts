# DevPorts (nombre de trabajo)

App de barra de menús para macOS que mantiene el orden de los servidores de desarrollo: qué corre, de qué proyecto es, cuál es el oficial y cuáles son pruebas con vencimiento. Incluye un CLI (`devports run`) para que los agentes de IA no dupliquen servidores.

**Estado:** construido y en pruebas de uso real (fases 1 y 2). Repositorio privado.
**Stack:** Rust, Tauri v2, React + TypeScript, SQLite. Solo macOS en v1.

- Cola de trabajo: [`TASKS.md`](TASKS.md)
- Reglas del proyecto y cómo retomar: [`CLAUDE.md`](CLAUDE.md)
- Especificación: [`docs/`](docs/) (empezar por [`docs/LEEME.md`](docs/LEEME.md))

## Probarla

```bash
tools/install.sh            # compila, sella e instala ~/Applications/DevPorts.app y la abre
tools/launchd-check.sh      # (opcional) comprueba el reinicio tras caída y la salida limpia
tools/docker-checks.sh      # (opcional, con Docker) las pruebas contra el demonio real
tools/measure.sh 120        # memoria y CPU en reposo contra el presupuesto
```

El menú del ícono (clic derecho) instala el comando `devports` y activa el inicio al abrir sesión.
