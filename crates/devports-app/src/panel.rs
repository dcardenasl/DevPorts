//! The panel: created when it is opened, destroyed when it loses focus.
//!
//! Keeping no webview alive while the panel is closed is what keeps the
//! application light in the menu bar (ADR-017, rule 3).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use devports_core::clock::{Clock, SystemClock};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

const PANEL_LABEL: &str = "panel";
const PANEL_WIDTH: f64 = 380.0;
const PANEL_HEIGHT: f64 = 520.0;
/// How long the panel waits for the page to say it has painted before showing
/// itself anyway, so a page that fails to load never leaves an invisible panel.
const REVEAL_FALLBACK: Duration = Duration::from_millis(1500);
/// Corner radius of the panel, in points (`UI-REFERENCIA.md`).
const PANEL_CORNER_RADIUS: f64 = 14.0;
/// A click on the icon that closes the panel first makes the panel lose focus,
/// which destroys it; the click's own handler then runs. Within this window a
/// "closed" panel means "the click that closed it", not "open it again".
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// Whether the current panel has been made visible to the user yet.
#[derive(Debug, Default)]
pub struct Revealed(AtomicBool);

/// Remembers that the panel has just closed because it lost focus, so the click
/// that caused it is not mistaken for a request to open it again.
#[derive(Debug)]
pub struct ReopenGuard<C: Clock> {
    clock: C,
    window: Duration,
    closed_by_blur_at: Mutex<Option<Instant>>,
}

/// The guard the application uses, on the real clock.
pub type PanelState = ReopenGuard<SystemClock>;

impl Default for PanelState {
    fn default() -> Self {
        Self::new(SystemClock, REOPEN_GUARD)
    }
}

impl<C: Clock> ReopenGuard<C> {
    /// Creates a guard that remembers a close for `window`.
    pub fn new(clock: C, window: Duration) -> Self {
        Self {
            clock,
            window,
            closed_by_blur_at: Mutex::new(None),
        }
    }

    /// Records that the panel closed because it lost focus.
    pub fn mark_closed(&self) {
        *self
            .closed_by_blur_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(self.clock.now());
    }

    /// Whether the panel closed by losing focus within the guard window.
    pub fn closed_just_now(&self) -> bool {
        self.closed_by_blur_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|at| self.clock.now().saturating_duration_since(at) < self.window)
    }
}

/// What a click on the tray icon should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleAction {
    /// The panel is open: close it.
    Close,
    /// The panel is closed and was not just closed by this click: open it.
    Open,
    /// The panel just closed by losing focus; this click is the one that did it.
    Ignore,
}

/// Decides what a click on the icon does.
pub fn decide_toggle(panel_is_open: bool, closed_just_now: bool) -> ToggleAction {
    match (panel_is_open, closed_just_now) {
        (true, _) => ToggleAction::Close,
        (false, true) => ToggleAction::Ignore,
        (false, false) => ToggleAction::Open,
    }
}

/// Opens the panel if it is closed, closes it if it is open.
pub fn toggle(app: &AppHandle) -> tauri::Result<()> {
    let is_open = app.get_webview_window(PANEL_LABEL).is_some();
    let closed_just_now = app.state::<PanelState>().closed_just_now();
    match decide_toggle(is_open, closed_just_now) {
        ToggleAction::Close => close(app),
        ToggleAction::Open => open(app),
        ToggleAction::Ignore => Ok(()),
    }
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
        .focused(false)
        .build()?;

    // On screen but fully transparent and deaf to the mouse until the page has
    // painted; see `window_style::style`.
    app.state::<Revealed>().0.store(false, Ordering::Release);
    #[cfg(target_os = "macos")]
    crate::window_style::style(&window, PANEL_CORNER_RADIUS, 0.0)?;
    window.set_ignore_cursor_events(true)?;
    anchor(&window)?;
    window.show()?;

    // The window stays hidden until the page reports that it has painted
    // (`panel_ready`), so the user never sees an empty rectangle.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(REVEAL_FALLBACK).await;
        // The page may have revealed it already, or the panel may be gone: `reveal`
        // then does nothing and returns `Ok`.
        if let Err(error) = reveal(&app) {
            crate::diagnostics::report(&error);
        }
    });
    Ok(())
}

/// Makes the panel visible, focuses it and starts closing it when it loses
/// focus. Does nothing if there is no panel or it was already revealed.
pub fn reveal(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        return Ok(());
    };
    if app.state::<Revealed>().0.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    crate::window_style::set_opacity(&window, 1.0)?;
    window.set_ignore_cursor_events(false)?;
    window.set_focus()?;

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            handle.state::<PanelState>().mark_closed();
            if let Err(error) = close(&handle) {
                crate::diagnostics::report(&error);
            }
        }
    });
    Ok(())
}

/// Called by the page once it has painted its first frame.
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value; the signature is fixed"
)]
pub fn panel_ready(app: AppHandle) -> Result<(), String> {
    reveal(&app).map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use devports_core::clock::ManualClock;

    use super::*;

    fn guard() -> (Arc<ManualClock>, ReopenGuard<Arc<ManualClock>>) {
        let clock = Arc::new(ManualClock::new("2026-10-17T12:00:00Z".parse().unwrap()));
        (Arc::clone(&clock), ReopenGuard::new(clock, REOPEN_GUARD))
    }

    #[test]
    fn a_fresh_guard_remembers_nothing() {
        let (_clock, guard) = guard();
        assert!(!guard.closed_just_now());
    }

    #[test]
    fn a_close_is_remembered_only_for_the_guard_window() {
        let (clock, guard) = guard();
        guard.mark_closed();
        assert!(guard.closed_just_now());

        clock.advance(Duration::from_millis(299));
        assert!(guard.closed_just_now(), "still inside the window");

        clock.advance(Duration::from_millis(1));
        assert!(!guard.closed_just_now(), "the window is over");
    }

    #[test]
    fn a_new_close_restarts_the_window() {
        let (clock, guard) = guard();
        guard.mark_closed();
        clock.advance(Duration::from_millis(250));
        guard.mark_closed();
        clock.advance(Duration::from_millis(250));
        assert!(guard.closed_just_now());
    }

    #[test]
    fn a_click_does_the_right_thing_in_every_state() {
        assert_eq!(decide_toggle(true, false), ToggleAction::Close);
        assert_eq!(decide_toggle(true, true), ToggleAction::Close);
        assert_eq!(decide_toggle(false, true), ToggleAction::Ignore);
        assert_eq!(decide_toggle(false, false), ToggleAction::Open);
    }

    /// The sequence that used to flicker: the panel loses focus because the user
    /// clicked the icon (it closes), then the click itself arrives.
    #[test]
    fn clicking_the_icon_with_the_panel_open_closes_it_and_does_not_reopen_it() {
        let (clock, guard) = guard();
        // Focus lost: the panel is destroyed and the guard armed.
        guard.mark_closed();
        clock.advance(Duration::from_millis(80));
        // The click's own handler now sees a closed panel.
        assert_eq!(
            decide_toggle(false, guard.closed_just_now()),
            ToggleAction::Ignore
        );
        // A deliberate click a moment later opens it.
        clock.advance(Duration::from_secs(2));
        assert_eq!(
            decide_toggle(false, guard.closed_just_now()),
            ToggleAction::Open
        );
    }
}
