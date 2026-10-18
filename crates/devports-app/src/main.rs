//! Menu-bar application shell.
//!
//! It hosts the core (which keeps watching whether or not a window exists),
//! shows an icon with the attention counter, and opens a 380x520 panel anchored
//! under the icon that is *created when opened and destroyed when it loses
//! focus*, so a closed panel costs no webview memory (ADR-017, rule 3).

mod commands;
mod diagnostics;
mod host;
mod integration;
mod notifications;
mod operations;
mod panel;
mod panel_state;
mod tray;
#[cfg(target_os = "macos")]
mod window_style;

use tauri::{ActivationPolicy, RunEvent};

fn main() {
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(panel::PanelState::default())
        .manage(panel::Revealed::default())
        .manage(panel_state::PanelStateStore::default())
        .manage(operations::OperationStore::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::preview_project_stop,
            commands::adoption_options,
            commands::preview_adoption,
            commands::validate_adoption,
            commands::shared_consumers,
            commands::ignore_preview,
            commands::ignored_rules,
            commands::get_panel_state,
            commands::set_panel_state,
            commands::get_operations,
            commands::get_operation,
            commands::start_operation,
            commands::retry_operation,
            commands::get_log,
            commands::open_url,
            panel::panel_ready,
        ])
        .setup(|app| {
            // Menu-bar utility: no Dock icon and no application menu.
            app.set_activation_policy(ActivationPolicy::Accessory);
            match host::start(app.handle()) {
                Ok(handle) => {
                    tauri::Manager::manage(app, host::CoreState(handle));
                }
                // The first instance keeps the menu bar; this one has nothing to add.
                Err(host::HostError::AlreadyRunning) => std::process::exit(0),
                Err(error) => return Err(error.into()),
            }
            tray::create(app.handle())?;
            // `DevPorts --show` opens the panel at launch: handy for a first look
            // and for scripts that cannot click the menu-bar icon.
            if std::env::args().any(|argument| argument == "--show") {
                panel::open(app.handle())?;
            }
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
