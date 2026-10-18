//! Hosts the core inside the application and bridges it to the interface.
//!
//! The core is started once, here, before any window exists: it keeps watching
//! the machine whether or not the panel is open. The panel (a webview created
//! on demand) reads snapshots through commands and receives each new one as an
//! event; the tray counter and the notifications follow the same snapshots.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use devports_core::engine::Notice;
use devports_core::runtime::{Core, CoreHandle, CoreOptions, RuntimeError};
use tauri::{AppHandle, Emitter, Manager};

use crate::i18n::{Msg, current, text};
use crate::notifications::{Notifier, Response};
use crate::preferences::PreferencesStore;

/// Event the panel listens to.
const SNAPSHOT_EVENT: &str = "snapshot";
/// One notification per project in this window (`notification_dedupe_window`).
const DEDUPE_WINDOW: Duration = Duration::from_mins(10);

/// The handle commands use to reach the core.
#[derive(Clone)]
pub struct CoreState(pub CoreHandle);

/// Why the application cannot run.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// There is no home folder to keep files in.
    #[error("cannot tell where to keep DevPorts files: HOME is not set")]
    NoHome,
    /// Another DevPorts is already running for this user.
    #[error("DevPorts is already running")]
    AlreadyRunning,
    /// The core failed to start.
    #[error(transparent)]
    Core(RuntimeError),
}

/// Starts the core and the tasks that bridge it to the interface.
///
/// # Errors
///
/// [`HostError`] when the core cannot start.
pub fn start(app: &AppHandle) -> Result<CoreHandle, HostError> {
    let options = CoreOptions::from_env().ok_or(HostError::NoHome)?;
    let core =
        tauri::async_runtime::block_on(Core::start_system(options)).map_err(
            |error| match error {
                RuntimeError::Ipc(devports_core::ipc::IpcError::AlreadyRunning(_)) => {
                    HostError::AlreadyRunning
                }
                other => HostError::Core(other),
            },
        )?;
    let handle = core.handle();
    load_preferences(app, &handle);
    // The core's own tasks live on Tauri's runtime until the process ends.
    tauri::async_runtime::spawn(core.wait());

    spawn_termination_bridge(app.clone(), handle.clone());
    spawn_snapshot_bridge(app.clone(), handle.clone());
    spawn_notice_bridge(app, &handle);
    Ok(handle)
}

/// Copies the saved preferences into the shell before any panel can open. A store that
/// cannot be read leaves the defaults (`system`): a preference never stops the app.
fn load_preferences(app: &AppHandle, handle: &CoreHandle) {
    match tauri::async_runtime::block_on(handle.preferences()) {
        Ok(preferences) => app.state::<PreferencesStore>().replace(preferences),
        Err(error) => crate::diagnostics::report(&error),
    }
}

/// `SIGTERM` (what `launchctl bootout` and a system shutdown send) and `SIGINT`
/// quit exactly like the menu's "Salir": the core closes its socket and the
/// process exits with code 0, so the launch agent does not bring it back.
fn spawn_termination_bridge(app: AppHandle, handle: CoreHandle) {
    use tokio::signal::unix::{SignalKind, signal};
    tauri::async_runtime::spawn(async move {
        let (Ok(mut term), Ok(mut interrupt)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
        ) else {
            crate::diagnostics::report(&"cannot listen for termination signals");
            return;
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = interrupt.recv() => {}
        }
        handle.shutdown();
        // Let the socket be removed before the process goes.
        tokio::time::sleep(Duration::from_millis(300)).await;
        app.exit(0);
    });
}

/// Pushes every snapshot to the panel and keeps the tray counter right.
fn spawn_snapshot_bridge(app: AppHandle, handle: CoreHandle) {
    tauri::async_runtime::spawn(async move {
        let mut changes = handle.watch();
        loop {
            let snapshot = changes.borrow_and_update().clone();
            crate::tray::show_count(
                &app,
                u32::try_from(snapshot.attention.len()).unwrap_or(u32::MAX),
            );
            // No panel, no listener: the event is simply dropped.
            if let Err(error) = app.emit_to(crate::panel::PANEL_LABEL, SNAPSHOT_EVENT, &*snapshot) {
                crate::diagnostics::report(&error);
            }
            if changes.changed().await.is_err() {
                return;
            }
        }
    });
}

/// Turns the core's notices into system notifications.
fn spawn_notice_bridge(app: &AppHandle, handle: &CoreHandle) {
    let responder = handle.clone();
    let opener = app.clone();
    let extend_title = text(current(app), Msg::ExtendAction);
    let notifier = Notifier::new(&extend_title, move |id, response| match response {
        Response::Extend => {
            if let Some(reference) = id.strip_prefix("lease:") {
                let reference = reference.to_owned();
                let responder = responder.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = responder.extend(&reference).await {
                        crate::diagnostics::report(&error);
                    }
                });
            }
        }
        Response::Opened => {
            if let Err(error) = crate::panel::open(&opener) {
                crate::diagnostics::report(&error);
            }
        }
        Response::Dismissed | Response::Other(_) => {}
    });
    let notifier = match notifier {
        Ok(notifier) => notifier,
        Err(error) => {
            // Running the bare binary (cargo run): everything but notifications works.
            crate::diagnostics::report(&error);
            return;
        }
    };
    notifier.request_authorization(|granted| {
        if !granted {
            crate::diagnostics::report(&"notifications are not allowed for DevPorts");
        }
    });

    let mut notices = handle.notices();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_by_project: HashMap<String, Instant> = HashMap::new();
        while let Ok(notice) = notices.recv().await {
            match notice {
                Notice::LeaseExpiring {
                    reference,
                    project,
                    service,
                    reason,
                    ..
                } => {
                    // Said in the language in force when it happens, not when the app started.
                    let locale = current(&app);
                    let name = service.unwrap_or_else(|| text(locale, Msg::UnnamedTest));
                    let detail = text(
                        locale,
                        Msg::LeaseDetail {
                            reason: reason.as_deref(),
                            project: project.as_deref(),
                        },
                    );
                    notifier.post_lease_expiring(
                        &format!("lease:{reference}"),
                        &text(locale, Msg::LeaseExpiring { name: &name }),
                        &detail,
                        report_failure,
                    );
                }
                Notice::DuplicateDetected {
                    reference,
                    project,
                    service,
                    port,
                } => {
                    let now = Instant::now();
                    let recent = last_by_project
                        .get(&project)
                        .is_some_and(|at| now.saturating_duration_since(*at) < DEDUPE_WINDOW);
                    if recent {
                        continue;
                    }
                    last_by_project.insert(project.clone(), now);
                    let locale = current(&app);
                    notifier.post_plain(
                        &format!("duplicate:{reference}"),
                        &text(locale, Msg::DuplicateTitle { service: &service }),
                        &text(
                            locale,
                            Msg::DuplicateBody {
                                port,
                                project: &project,
                            },
                        ),
                        report_failure,
                    );
                }
            }
        }
    });
}

fn report_failure(error: Option<String>) {
    if let Some(message) = error {
        crate::diagnostics::report(&message);
    }
}
