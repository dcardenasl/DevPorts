# devports-proto

Los tipos que viajan por el socket entre el CLI y el core (un documento JSON por línea). Solo tipos: sin I/O ni lógica.

Existe como crate aparte para que `devports-cli` dependa **únicamente** de este y arranque en milisegundos, sin arrastrar el core ni Tauri.

- `lib.rs`: `Request`, `Response`, `Op` (las operaciones que expone el socket), `PROTOCOL_VERSION`.
- `run.rs`: `devports run` (`RunRegister`, `RunOutcome`).
- `views.rs`: vistas de solo lectura (`InstanceView`, `LogEntry`). Nunca contienen un token.
- `paths.rs`: dónde vive el socket (core y CLI comparten la regla).
- `duration.rs`: `30m`, `2h`, `1h30m`.
- `preferences.rs`: tema e idioma (solo el panel los edita; el socket no los transporta).

Ampliar `Op` amplía la superficie que un agente puede alcanzar: pide un ADR antes (ver `docs/DECISIONES.md`, ADR-009).
