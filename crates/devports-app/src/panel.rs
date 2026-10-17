//! The panel: created when it is opened, destroyed when it loses focus.
//!
//! Keeping no webview alive while the panel is closed is what keeps the
//! application light in the menu bar (ADR-017, rule 3).

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

const PANEL_LABEL: &str = "panel";
const PANEL_WIDTH: f64 = 380.0;
const PANEL_HEIGHT: f64 = 520.0;
/// A click on the icon that closes the panel first makes the panel lose focus,
/// which destroys it; the click's own handler then runs. Within this window a
/// "closed" panel means "the click that closed it", not "open it again".
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// When the panel last closed because it lost focus.
#[derive(Debug, Default)]
pub struct PanelState {
    closed_by_blur_at: Mutex<Option<Instant>>,
}

impl PanelState {
    fn mark_closed(&self) {
        *self
            .closed_by_blur_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    fn closed_just_now(&self) -> bool {
        self.closed_by_blur_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|at| at.elapsed() < REOPEN_GUARD)
    }
}

/// Opens the panel if it is closed, closes it if it is open.
pub fn toggle(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(PANEL_LABEL).is_some() {
        return close(app);
    }
    if app.state::<PanelState>().closed_just_now() {
        return Ok(());
    }
    open(app)
}

/// Creates the panel, anchors it under the tray icon and gives it focus.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if let Some(existing) = app.get_webview_window(PANEL_LABEL) {
        return existing.set_focus();
    }
    let window = WebviewWindowBuilder::new(app, PANEL_LABEL, WebviewUrl::App("index.html".into()))
        .title("DevPorts")
        .inner_size(PANEL_WIDTH, PANEL_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;

    anchor(&window)?;
    window.show()?;
    window.set_focus()?;

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            handle.state::<PanelState>().mark_closed();
            // Failing to close an already closing window is not worth reporting.
            let _ = close(&handle);
        }
    });
    Ok(())
}

/// Destroys the panel and its webview.
pub fn close(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window(PANEL_LABEL) {
        Some(window) => window.destroy(),
        None => Ok(()),
    }
}

/// Puts the panel under the tray icon, or in the top-right corner if no click
/// on the icon has told the positioner where it is yet (opened from the menu).
fn anchor(window: &WebviewWindow) -> tauri::Result<()> {
    window
        .move_window(Position::TrayBottomCenter)
        .or_else(|_| window.move_window(Position::TopRight))
}
