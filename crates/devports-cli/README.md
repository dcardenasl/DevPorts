# devports-cli

El binario `devports`. Cliente fino: pregunta al core por el socket, imprime y, en `run`, reemplaza su proceso con el comando pedido (`exec`, mismo pid).

Depende **solo** de `devports-proto`. No toma decisiones: toda regla es del core.

- `main.rs`: subcomandos (`run`, `ls`, `stop`, `extend`, `log`, `adopt`, `version`) y códigos de salida.
- `run.rs`: `devports run`; el único módulo que ejecuta algo.
- `ancestry.rs`: la cadena de procesos que llevó hasta este comando (solo este proceso la conoce).
- `client.rs`: cliente bloqueante del socket, sin runtime async.
- `output.rs`: salida para personas; con `--json` se imprimen los tipos del protocolo tal cual.

Comandos y códigos de salida: [`docs/CLI.md`](../../docs/CLI.md). Flujo completo de `run`: [`docs/MAPA-DEL-CODIGO.md`](../../docs/MAPA-DEL-CODIGO.md), sección 4.
