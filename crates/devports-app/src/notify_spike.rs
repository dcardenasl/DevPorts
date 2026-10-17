//! One notification with an action button, to answer V6 (spike only).
//!
//! Run it from the application bundle (`UNUserNotificationCenter` needs one):
//! after `DEVPORTS_SPIKE_NOTIFY_AFTER` seconds (default 3) it asks for permission
//! and shows "api vence en 1 minuto" with a single "Extender 30 min" button, and
//! writes to stderr whatever the user does with it.

#![allow(
    clippy::print_stderr,
    reason = "spike instrumentation: the person running the spike reads these lines"
)]

use std::time::Duration;

use tauri::AppHandle;

use crate::notifications::{Notifier, Response};

const DEFAULT_DELAY_SECONDS: u64 = 3;
const NOTIFICATION_ID: &str = "spike-lease-expiring";

pub fn start(_app: &AppHandle) {
    let notifier = match Notifier::new(|id, response: Response| {
        eprintln!("notify: user performed {response:?} on {id}");
    }) {
        Ok(notifier) => notifier,
        Err(error) => {
            eprintln!("notify: {error}");
            return;
        }
    };
    let delay = std::env::var("DEVPORTS_SPIKE_NOTIFY_AFTER")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_DELAY_SECONDS);

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(delay)).await;
        eprintln!("notify: requesting authorization");
        notifier.request_authorization(|granted| {
            eprintln!("notify: authorization granted: {granted}");
        });
        // Give the person a moment to answer the system prompt the first time.
        tokio::time::sleep(Duration::from_secs(2)).await;
        eprintln!("notify: showing");
        notifier.post_lease_expiring(
            NOTIFICATION_ID,
            "api vence en 1 minuto",
            "Prueba «QA login» del proyecto tienda-demo",
            |error| match error {
                None => eprintln!("notify: delivered to the system"),
                Some(message) => eprintln!("notify: could not post: {message}"),
            },
        );
    });
}
