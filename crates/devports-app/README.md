# devports-app

El shell de la barra de menú (Tauri v2). Aloja el core y lo conecta con el panel. **No decide reglas**: `commands.rs` solo traduce.

Mapa archivo por archivo: [`docs/MAPA-DEL-CODIGO.md`](../../docs/MAPA-DEL-CODIGO.md), sección 6.

## Ideas que explican el diseño
- El core arranca en `host.rs` **antes** de que exista ninguna ventana y sigue vigilando con el panel cerrado.
- El panel (`panel.rs`) se crea al abrir y se destruye al perder foco, para no mantener un webview vivo.
- Lo que debe sobrevivir a esa destrucción vive en el shell: `panel_state.rs` (contexto) y `operations.rs` (operaciones en curso).
- Es una app "Accessory": sin ícono en el Dock ni menú de aplicación.

## Añadir un comando para el panel
1. Función en `src/commands.rs`.
2. Registrarla en `tauri::generate_handler![]` de `src/main.rs`.
3. Escribir su nombre una sola vez en `ui/src/api.ts`.
