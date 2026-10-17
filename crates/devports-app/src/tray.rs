//! The menu-bar icon: a template image with the attention counter next to it.

use tauri::AppHandle;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

const TRAY_ID: &str = "main";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";

/// Creates the tray icon. Left click toggles the panel, right click opens a
/// native menu with "Abrir panel" and "Salir".
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, OPEN_ID, "Abrir panel", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        // A template image: macOS tints it for the light and dark menu bar.
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("DevPorts")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            OPEN_ID => {
                if let Err(error) = crate::panel::open(app) {
                    report(&error);
                }
            }
            QUIT_ID => app.exit(0),
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
                report(&error);
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
            report(&error);
        }
    }
}

/// Nothing can be shown to the user yet; the spike reports on stderr.
#[allow(clippy::print_stderr)]
fn report(error: &tauri::Error) {
    eprintln!("devports: {error}");
}
