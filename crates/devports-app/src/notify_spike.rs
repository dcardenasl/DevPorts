//! One notification with an action button, to answer V6 (spike only).
//!
//! After `DEVPORTS_SPIKE_NOTIFY_AFTER` seconds (default 3) it shows "Vence en 1
//! minuto" with a single action, "Extender 30 min", and writes to stderr
//! whatever the user does with it: a click on the banner (`tap`) or on the
//! button (`extend`).

#![allow(
    clippy::print_stderr,
    reason = "spike instrumentation: the person running the spike reads these lines"
)]

use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

const ACTION_TYPE: &str = "lease-expiring";
const EXTEND_ACTION: &str = "extend";
const DEFAULT_DELAY_SECONDS: u64 = 3;

pub fn start(app: &AppHandle) {
    let delay = std::env::var("DEVPORTS_SPIKE_NOTIFY_AFTER")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_DELAY_SECONDS);
    let notifications = app.notification();

    let extend = Action::builder(EXTEND_ACTION, "Extender 30 min").build();
    let lease_type = ActionType::builder(ACTION_TYPE)
        .actions(vec![extend])
        .build();
    report(
        "register action type",
        notifications.register_action_types(vec![lease_type]),
    );
    report(
        "register handler",
        notifications.on_action(|performed| {
            eprintln!("notify: user performed `{}`", performed.action_id());
        }),
    );

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(delay)).await;
        eprintln!("notify: showing");
        report(
            "show",
            app.notification()
                .builder()
                .title("api vence en 1 minuto")
                .body("Prueba «QA login» del proyecto tienda-demo")
                .action_type_id(ACTION_TYPE)
                .show(),
        );
    });
}

fn report<E: std::fmt::Display>(what: &str, result: Result<(), E>) {
    match result {
        Ok(()) => eprintln!("notify: {what}: ok"),
        Err(error) => eprintln!("notify: {what}: {error}"),
    }
}
