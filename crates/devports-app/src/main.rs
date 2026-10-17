//! Menu-bar application shell.
//!
//! Phase 0 spike (task T0.5): a tray icon that shows an attention counter, a
//! 380x520 panel anchored under the icon that is *created when opened and
//! destroyed when it loses focus*, and no Dock icon. It answers V5 and V10 of
//! `ARQUITECTURA.md` (does Tauri give a polished anchored panel, and does
//! destroying the webview return the memory) and is replaced by the real shell
//! in task T2.1.

mod attention;
mod panel;
mod tray;

#[cfg(feature = "spike-autopilot")]
mod autopilot;
#[cfg(feature = "spike-notify")]
mod notify_spike;

use tauri::{ActivationPolicy, RunEvent};

fn main() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_positioner::init());
    #[cfg(feature = "spike-notify")]
    let builder = builder.plugin(tauri_plugin_notification::init());
    let built = builder
        .manage(attention::Attention::default())
        .manage(panel::PanelState::default())
        .invoke_handler(tauri::generate_handler![
            attention::get_counter,
            attention::bump_counter,
            attention::reset_counter,
        ])
        .setup(|app| {
            // Menu-bar utility: no Dock icon and no application menu.
            app.set_activation_policy(ActivationPolicy::Accessory);
            tray::create(app.handle())?;
            #[cfg(feature = "spike-autopilot")]
            autopilot::start(app.handle());
            #[cfg(feature = "spike-notify")]
            notify_spike::start(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!());

    match built {
        Ok(app) => app.run(|_app, event| {
            // The panel is destroyed on every close, so the last window going
            // away must not quit the application: it lives in the menu bar.
            // An explicit `exit(code)` carries a code and is let through.
            if let RunEvent::ExitRequested {
                api, code: None, ..
            } = event
            {
                api.prevent_exit();
            }
        }),
        Err(error) => {
            report_startup_failure(&error);
            std::process::exit(1);
        }
    }
}

/// There is no logger yet and no window to show the failure in.
#[allow(clippy::print_stderr)]
fn report_startup_failure(error: &tauri::Error) {
    eprintln!("devports: could not start: {error}");
}
