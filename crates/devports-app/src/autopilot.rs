//! Timer-driven panel use, for measuring memory without a human (spike only).
//!
//! With `P` seconds per phase (`DEVPORTS_SPIKE_PHASE`, default 20): idle for `P`,
//! panel open for `P`, closed for `P`, open again for `P`, closed again for `P`,
//! counter at 3 for `P`, then exit. Each transition is written to stderr so the
//! measuring script can line its samples up with it.

#![allow(
    clippy::print_stderr,
    reason = "spike instrumentation: the measuring script reads these markers"
)]

use std::time::Duration;

use tauri::AppHandle;

const DEFAULT_PHASE_SECONDS: u64 = 20;

pub fn start(app: &AppHandle) {
    let phase = std::env::var("DEVPORTS_SPIKE_PHASE")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_PHASE_SECONDS);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let step = Duration::from_secs(phase);
        eprintln!("autopilot: idle");
        tokio::time::sleep(step).await;
        for round in 1..=2 {
            eprintln!("autopilot: open {round}");
            log(crate::panel::open(&app));
            tokio::time::sleep(step).await;
            eprintln!("autopilot: closed {round}");
            log(crate::panel::close(&app));
            tokio::time::sleep(step).await;
        }
        eprintln!("autopilot: counter");
        crate::tray::show_count(&app, 3);
        tokio::time::sleep(step).await;
        eprintln!("autopilot: exit");
        app.exit(0);
    });
}

fn log(result: tauri::Result<()>) {
    if let Err(error) = result {
        eprintln!("autopilot: {error}");
    }
}
