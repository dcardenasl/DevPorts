# Plan — Tema claro/oscuro e internacionalización (UXR-9)

> Fuente de diseño de las tareas UXR-9.0 a UXR-9.6 de [TASKS.md](../../TASKS.md#tema-e-idioma). Las decisiones de producto se registran como ADR-040 (tema) y ADR-041 (idioma) en la tarea UXR-9.0; hasta entonces este documento es la propuesta **[P]** confirmada por David en sus tres decisiones de alcance (ver Contexto). Si este plan y `docs/DECISIONES.md` se contradicen, manda `DECISIONES.md`.

## Contexto
Hoy el panel es solo oscuro (`UI-REFERENCIA.md:14`, «modo claro: diferido» desde ADR-027) y todo el texto está en español sin mecanismo de i18n (NF-10, CLAUDE.md «Idioma»). Se pide: tema Sistema/Claro/Oscuro e idioma Sistema/Español/English, robustos, predecibles y sin deuda.

**Decisiones confirmadas por David [C]:**
1. Idiomas: **español + inglés**.
2. Alcance: **panel + bandeja + notificaciones**. CLI, `--json`, goldens y `docs/` siguen en español (NF-10 se enmienda solo para este límite).
3. Preferencias en una **tabla SQLite `preferences`** (no en `config.yml`).

**Hallazgos que condicionan el diseño (exploración del 2026-10-18):**
- `ui/src/styles.css`: 16 tokens en un único `:root` oscuro, pero **~39 colores literales fuera de tokens** (teñidos `rgba(61,214,160,…)`, blancos con alfa, texto sobre relleno `#06241a`/`#1a0d0a`/`#fff`, píldora `#fff/#101316`, puntos `#5b656c/#7b868d`, `#262d33`).
- `ui/src/text.ts` (443 líneas) ya centraliza el texto del panel, sin `as const` necesario ni JSX con español suelto. Escapan: `settings.ts`, `adoption.ts`, 3 `aria-label` (`App.tsx:324,407`, `ActivityView.tsx:159`), `"Falló"` (`App.tsx:340,585`), `toLocaleTimeString("es")` (`time.ts:38`), `plural()` propio (`text.ts:441`), `<html lang="es">`.
- **El núcleo envía texto en español al panel**: `LogEntry.summary` (`messages::event_summary`), `SettingsError.message`, `RootProblem.message`, `ScanFolders.reason`, `AdoptionDestination.reason`, `AdoptionPreview.warnings` y los errores `String` de `commands.rs`. Traducir solo `ui/` dejaría Actividad y errores en español.
- El webview es `incognito` → `localStorage` inútil. El panel se crea y destruye en cada apertura; la ventana arranca con alfa 0 hasta `panel_ready`. `PanelState` es volátil (nunca va a disco) y no debe guardar preferencias.
- Bandeja: menú creado una vez (`tray.rs`); notificaciones: textos en `host.rs`, acción «Extender 30 min» en `notifications.rs` (categoría registrada una vez).
- El núcleo es dueño de SQLite (un actor, un escritor; `store.rs` con `MIGRATIONS` y `PRAGMA user_version`, hoy 3 entradas).
- Último ADR = 039, última tarea = UXR-8, último NF = NF-10.

## Diseño

### A. Preferencias (fuente única)
- `devports-proto`: tipos `Preferences { theme: ThemePreference, language: LanguagePreference }`; `ThemePreference = system|light|dark`, `LanguagePreference = system|es|en` (specta → `ui/src/bindings.ts`, nunca a mano).
- `devports-core/src/store.rs`: **migración 4** `preferences(key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL)` con `CHECK` por clave. Lectura con valor por defecto `system` si falta o no se reconoce (nunca falla el arranque). Mensaje del actor `get/set_preferences` (regla 8: el actor es dueño de SQLite; sin `Mutex` sobre el dominio). No genera evento de log (no es una acción sobre procesos).
- `devports-app`: comandos Tauri `get_preferences` / `set_preferences`. **No** van en el socket (ADR-009, regla 6). `set_preferences` valida, persiste, actualiza una copia en el shell (para abrir el panel sin esperar I/O), reconstruye la bandeja y devuelve lo guardado. Los nombres de comando solo se escriben en `ui/src/api.ts`; mock en `mock-preferences.ts`.
- No toca `config.yml` ni ADR-035.

### B. Tema
- **Un solo mecanismo, sin parpadeo**: `<html data-theme="light|dark">` siempre explícito (resuelto) y `color-scheme` acorde.
  - Resolución síncrona antes del primer pintado: el shell añade al URL `index.html?…&theme=system|light|dark&lang=es|en` (patrón ya usado por `?tab=` en `panel.rs`); un módulo `ui/src/boot.ts`, importado el primero por `main.tsx` (sin script inline: CSP `script-src 'self'`), lee los parámetros, resuelve `system` con `matchMedia('(prefers-color-scheme: dark)')` y fija `data-theme` (el `color-scheme` lo declara cada bloque de `theme.css`). La ventana sigue transparente hasta `panel_ready`, posterior al montaje, y el CSS sin atributo es el oscuro. Sin parámetros (navegador en desarrollo) usa sistema/`navigator`.
  - `ui/src/theme.ts`: lógica pura `resolveTheme(pref, systemDark)` + `applyTheme()` y un listener `matchMedia` (solo con preferencia = sistema) que cubre el cambio de apariencia de macOS con el panel abierto. Testeable.
  - Ventana nativa coherente: `set_theme(None|Light|Dark)` de Tauri desde el shell (API pública, sin `unsafe` nuevo; `window_style.rs` no cambia).
- **Tokens semánticos completos** en `ui/src/theme.css` (`:root[data-theme="dark"]` y `[data-theme="light"]`): superficies, texto, `--green/amber/coral/blue` y los que hoy son literales: `--tint-*` (14 %), `--wash-*` (7–8 %), `--edge-*`, `--on-green/amber/coral` (texto sobre relleno), `--hover/--press/--surface`, `--dot-idle/--dot-dashed`, `--chip-active-bg/fg`, `--field`, `--shadow`. `styles.css` no contiene ningún color literal; un test lo vigila.
- Paleta clara con los mismos roles (verde/ámbar/coral/azul oscurecidos para AA sobre claro). Se documenta en `UI-REFERENCIA.md` §6 (tabla por tema) y se retira «modo claro no diseñado» (L14, L152, L191). Sin `color-mix`/`light-dark()` (target `safari15`, `LSMinimumSystemVersion 12`): valores explícitos y verificables.
- `ui/src/contrast.test.ts` se **parametriza por tema** (lee cada bloque `[data-theme=…]`, los mismos 21 pares + texto-sobre-relleno movido a tokens, AA ≥ 4.5; foco y bordes de UI ≥ 3:1). Test nuevo: ningún `#hex`/`rgb(` fuera de `theme.css`; ambos temas definen exactamente el mismo conjunto de tokens.

### C. i18n del panel (sin librería: catálogo tipado, coherente con `text.ts`)
Se descartan i18next/react-intl/Lingui: añaden dependencia y claves-string que fallan en ejecución, sin mejorar lo que TypeScript estricto ya garantiza con dos idiomas.
- `ui/src/i18n/`:
  - `es.ts` (movido desde `text.ts`) define el tipo `Catalog = typeof es`; `en.ts: Catalog` → **completitud y firmas de funciones verificadas en compilación**.
  - `locale.ts`: `Locale = "es" | "en"`, `LOCALES`, `resolveLocale(pref, systemTags)` (regla de respaldo documentada en ADR-041).
  - `format.ts`: envoltorios de `Intl` por locale, con caché: `plural` con `Intl.PluralRules`, `RelativeTimeFormat` («hace…»), `DateTimeFormat` (`clock`), `NumberFormat`, `ListFormat`. Reemplaza `plural()` propio, los ternarios `count === 1` y `toLocaleTimeString("es")`. `countdown` (mm:ss) no cambia.
  - `provider.tsx`: `I18nProvider` + `useI18n()` → `{ locale, t, format }`. Sustituye `import { text }` en los 16 archivos por `const { t } = useI18n()`; los helpers puros (`time.ts`, `settings.ts`, `adoption.ts`) reciben `t`/`locale` por parámetro.
  - Actualiza `document.documentElement.lang` al cambiar de idioma.
- Guardias automáticos: regla ESLint `no-restricted-syntax` contra `JSXText` no vacío y literales en `aria-label|title|placeholder|alt`; test de paridad de claves y aridad es↔en; tests de plurales y fechas en ambos locales; las pruebas existentes fijan `es` mediante el provider del helper de render (no se reescriben ~372 consultas) y se añade una suite `en` de humo por vista.
- Añadir un idioma futuro = un archivo `xx.ts` + una variante en `Locale` (Rust y TS); el compilador lista lo que falta.

### D. Texto que viene del núcleo → datos tipados (evita la deuda)
Principio: el núcleo envía **códigos + parámetros** y el panel los presenta. El texto en español de `messages.rs` sigue existiendo solo para CLI/socket (`--json` y goldens intactos).
- `LogEntry` gana `facts` tipado (`port`, `reason`, `launcher`, `existing_url`, `expires_at`, …) derivado del payload; `kind` ya existe. El panel renderiza `t.activity.event[kind](facts)`; `summary` queda como respaldo para un `kind` desconocido (núcleo más nuevo) y para el CLI. `#[serde(default)]` mantiene la compatibilidad.
- Enums de panel con `message: String` (`SettingsError::{Ineligible,InvalidList,Unsupported,Io}`, `RootProblem`, `ScanFolders.reason`, `AdoptionDestination.reason`, `AdoptionPreview.warnings`, `UsesError`, `AdoptionError`) pasan a **variantes/razones tipadas con parámetros**. El detalle técnico del SO o del archivo (rutas, errno) viaja como `detail` y se muestra tal cual bajo el texto traducido.
- Los `Result<_, String>` de `commands.rs` → un `CommandError { code, … }` tipado único (specta) con mapeo en el panel; `ActionError` recibe `Serialize` + specta.
- Test de guardia en Rust: los tipos exportados al panel no contienen campos de texto libre salvo `detail` y nombres puestos por el usuario (lista blanca revisada en `crates/devports-core/tests/bindings.rs`).

### E. Bandeja y notificaciones (Rust, `devports-app`)
- `crates/devports-app/src/i18n.rs`: `Locale {Es, En}`, `resolve(pref, system)` con `sys-locale` (API segura, sin `unsafe`; pasar `cargo deny`) y **catálogo por `match` exhaustivo** (`enum Msg` → texto) para: menú de bandeja (4 ítems + tooltip), diálogos de `tray.rs`/`integration::tell`, textos de `host.rs`, acción «Extender 30 min». Una variante sin traducir no compila.
- `set_preferences` reconstruye el menú de la bandeja y vuelve a registrar la categoría de notificación (idempotente) con el idioma nuevo.
- `notifications.rs` solo recibe el título localizado (sigue dentro de los archivos con `unsafe` permitidos); no se crean archivos `unsafe` nuevos.

### F. UI de ajustes
Secciones «Apariencia» e «Idioma» en `SettingsSheet` (encima de las carpetas): dos grupos de radio accesibles (Sistema/Claro/Oscuro; Sistema/Español/English con autónimos). Se aplican al instante (no destructivo: sin vista previa ni operación), con reversión y aviso si el guardado falla. Independientes del borrador de carpetas (ADR-035/036 sin cambios).

## Tareas (ver fichas en TASKS.md)
| ID | Contenido | Depende de |
|---|---|---|
| UXR-9.0 | ADR-040/041, NF-11, PRUEBAS §4, UI-REFERENCIA (paleta clara) | — |
| UXR-9.1 | Preferencias: proto + migración 4 + actor + comandos + `api.ts`/mock + bindings | 9.0 |
| UXR-9.2 | Tema: `theme.css`, tokenizar literales, `boot.ts`, `theme.ts`, URL en `panel.rs`, `set_theme`, contraste ×2 | 9.1 |
| UXR-9.3 | Catálogo i18n del panel: `i18n/`, 16 consumidores, `Intl`, ESLint, `en.ts`, paridad | 9.1 |
| UXR-9.4 | Actividad como datos tipados: `LogEntry.facts` | 9.3 |
| UXR-9.7 | Errores y razones del núcleo como datos tipados: `reasons.rs`, enums de error, `CommandError` (hecha) | 9.3, 9.4 |
| UXR-9.8 | Avisos del núcleo (`problems`), `stale` y `docker_error` como datos tipados (hecha) | 9.7 |
| UXR-9.5 | Bandeja y notificaciones: `i18n.rs`, reconstrucción en caliente | 9.1 |
| UXR-9.6 | Ajustes de apariencia/idioma en `SettingsSheet` y cierre | 9.2, 9.3, 9.4, 9.5 |

9.2, 9.3 y 9.5 son independientes entre sí tras 9.1 (sesiones paralelas posibles; coinciden solo en `styles.css`/`SettingsSheet`, por eso ninguna de ellas edita `SettingsSheet`: lo hace 9.6).

## Uso de /commit-flow
Cada tarea termina con `/commit-flow` (reglas de CLAUDE.md: una línea, inglés, imperativo, sin cuerpo ni trailers, sin `--no-verify`/`--amend`, archivos nombrados, `git diff --cached --stat` antes, 1–3 commits por tarea en orden base→superficie, hook fallido → arreglar y commit **nuevo**, sin push). Commits previstos:
- 9.0: `docs(adr): record adr-040 and adr-041 for theme and language`
- 9.1: `feat(proto): add the preferences types` → `feat(core): store preferences in sqlite with migration 4` → `feat(app): add the preferences commands`
- 9.2: `refactor(ui): move every color into theme tokens` → `feat(ui): resolve light and dark themes before first paint` → `test(ui): check contrast for both themes`
- 9.3: `refactor(ui): replace the text module with a typed catalog` → `feat(ui): add the english catalog and intl formatting`
- 9.4: `refactor(core): send typed facts and error codes to the panel` → `refactor(ui): render core events from typed facts`
- 9.5: `feat(app): localize the tray and notifications`
- 9.6: `feat(ui): add appearance and language to settings` → `docs: close uxr-9 with evidence`

CHANGELOG `[Unreleased]` solo en los `feat`; nunca inventar versión.

## Fuera de alcance
CLI/`--json`/goldens/`docs/` en inglés, más de dos idiomas, RTL, monedas, temas personalizados, selector en la bandeja, vibrancy.

## Riesgos y mitigación
- Tema equivocado un fotograma → resolución por URL en `boot.ts` antes de montar, ventana a alfa 0 hasta `panel_ready`; test de `boot.ts` en jsdom.
- Contraste de la paleta clara → lo decide el test AA antes de cerrar; se iteran los valores, no el umbral.
- Regresión de `App.test.tsx` → provider fijado a `es` en el helper de render.
- Núcleo más nuevo o más antiguo que el panel → campos `#[serde(default)]` y respaldo con `summary`.
- Deriva de `bindings.ts` → regenerar con `UPDATE_BINDINGS=1 cargo test -p devports-core --features bindings --test bindings`.

## Verificación global
- `tools/check.sh full` (fmt, matriz con IDs `covers:` nuevos, clippy `-D warnings`, tests, `cargo deny`) y en `ui/`: `pnpm run typecheck && pnpm run lint && pnpm run format:check && pnpm test && pnpm run build`.
- Pruebas nuevas: migración 4 y valores por defecto; resolución de locale; `theme.ts`; contraste ×2 temas; paridad es/en; plurales y fechas; Actividad en `en`; exhaustividad de `Msg`.
- Manual (el panel nativo no se puede automatizar desde el agente; pedirle a David la comprobación nativa): `tools/install.sh`, alternar tema/idioma en Ajustes, cambiar la apariencia de macOS con el panel abierto y cerrado, reabrir (sin parpadeo), bandeja y una notificación en ambos idiomas. En el navegador de desarrollo, con los mocks, capturas claro/oscuro × es/en de cada pestaña.
