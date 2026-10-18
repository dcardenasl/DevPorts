# devports-core

Toda la lógica del producto. **No depende de `tauri`** (lo vigila `tests/architecture.rs`), así que se prueba y se ejecuta sin interfaz.

Mapa completo y orden de lectura: [`docs/MAPA-DEL-CODIGO.md`](../../docs/MAPA-DEL-CODIGO.md), sección 3.

## Por dónde empezar
1. `src/lib.rs`: lista de módulos.
2. `src/engine.rs`: la máquina de estados. Recibe datos, devuelve estado + `Effect`s. No lee el reloj ni toca procesos.
3. `src/runtime.rs`: el actor que posee el `Engine` y las tareas que lo alimentan.

## Cómo se prueba
- Unitarias: `src/**/tests.rs`, junto a cada módulo. Usan `ManualClock`, `FakeProcessSource` y `FakeContainerSource`.
- Integración: `tests/` (sockets, procesos y SQLite reales; `architecture.rs`, `bindings.rs`).
- Sin interfaz: `cargo run -p devports-core --example headless`.

```bash
cargo test -p devports-core
```
