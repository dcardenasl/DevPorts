//! The menu-bar icon: a template image with the attention counter next to it.

use tauri::AppHandle;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

const TRAY_ID: &str = "main";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";
const INSTALL_ID: &str = "install-cli";
const AUTOSTART_ID: &str = "autostart";

/// Creates the tray icon. Left click toggles the panel, right click opens a
/// native menu with "Abrir panel" and "Salir".
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, OPEN_ID, "Abrir panel", true, None::<&str>)?;
    let install = MenuItem::with_id(
        app,
        INSTALL_ID,
        "Instalar el comando devports…",
        true,
        None::<&str>,
    )?;
    let autostart = CheckMenuItem::with_id(
        app,
        AUTOSTART_ID,
        "Abrir al iniciar sesión",
        true,
        crate::integration::autostart_enabled(),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Salir", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &install, &autostart, &separator, &quit])?;
    let autostart_item = autostart.clone();

    TrayIconBuilder::with_id(TRAY_ID)
        // A template image: macOS tints it for the light and dark menu bar.
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("DevPorts")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            OPEN_ID => {
                if let Err(error) = crate::panel::open(app) {
                    crate::diagnostics::report(&error);
                }
            }
            INSTALL_ID => install_command(),
            AUTOSTART_ID => {
                let wanted = !crate::integration::autostart_enabled();
                match crate::integration::set_autostart(wanted) {
                    Ok(()) => {}
                    Err(error) => crate::integration::tell(&format!(
                        "No se pudo cambiar el inicio automático: {error}"
                    )),
                }
                // Show what is true now, whatever happened.
                let _ = autostart_item.set_checked(crate::integration::autostart_enabled());
            }
            QUIT_ID => {
                // Closing the core's socket cleanly before the process ends.
                if let Some(core) = tauri::Manager::try_state::<crate::host::CoreState>(app) {
                    core.0.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            // The positioner needs every tray event to know where the icon is.
            tauri_plugin_positioner::on_tray_event(app, &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && let Err(error) = crate::panel::toggle(app)
            {
                crate::diagnostics::report(&error);
            }
        })
        .build(app)?;
    Ok(())
}

/// Shows `count` next to the icon, or nothing when it is zero.
pub fn show_count(app: &AppHandle, count: u32) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let title = (count > 0).then(|| count.to_string());
        if let Err(error) = tray.set_title(title) {
            crate::diagnostics::report(&error);
        }
    }
}

/// Links the command and tells the person where it went and what to do next.
fn install_command() {
    match crate::integration::install_command() {
        Ok(link) => {
            let folder = link
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            let hint = if crate::integration::on_path(&folder) {
                String::new()
            } else {
                format!(
                    "\n\nAgrega {} a tu PATH (por ejemplo en ~/.zshrc: export PATH=\"{}:$PATH\").",
                    folder.display(),
                    folder.display()
                )
            };
            crate::integration::tell(&format!(
                "Listo: {}\n\nAhora puedes usar `devports ls`.{hint}",
                link.display()
            ));
        }
        Err(error) => crate::integration::tell(&format!("No se pudo instalar el comando: {error}")),
    }
}
