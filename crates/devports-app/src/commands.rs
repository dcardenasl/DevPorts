//! The commands the panel calls. They only translate: every rule is the core's.

#![allow(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value; the signatures are fixed"
)]

use devports_core::adopt::apply::ApplyError;
use devports_core::runtime::{AdoptTarget, Snapshot};
use devports_proto::LogArgs;
use devports_proto::views::LogEntry;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::host::CoreState;
use crate::panel_state::{PanelState, PanelStateStore};

/// Where the panel asked an adoption to go.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptRequest {
    /// Into an existing project.
    Project(String),
    /// Into a project created in the process's folder.
    New(Option<String>),
    /// A container declared in `shared.yml`.
    Shared,
}

/// What was written, for the panel to confirm.
#[derive(Debug, Serialize)]
pub struct Adopted {
    project: String,
    service: String,
}

#[tauri::command]
pub fn get_snapshot(core: State<'_, CoreState>) -> Snapshot {
    core.0.snapshot().as_ref().clone()
}

#[tauri::command]
pub fn get_panel_state(state: State<'_, PanelStateStore>) -> PanelState {
    state.get()
}

#[tauri::command]
pub fn set_panel_state(store: State<'_, PanelStateStore>, state: PanelState) -> Result<(), String> {
    store.set(state).map_err(str::to_owned)
}

#[tauri::command]
pub async fn stop_instance(core: State<'_, CoreState>, reference: String) -> Result<(), String> {
    core.0
        .stop(&reference)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn extend_instance(core: State<'_, CoreState>, reference: String) -> Result<(), String> {
    core.0
        .extend(&reference)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn keep_instance(core: State<'_, CoreState>, reference: String) -> Result<(), String> {
    core.0
        .ignore(&reference)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn adopt_instance(
    core: State<'_, CoreState>,
    reference: String,
    target: AdoptRequest,
) -> Result<Adopted, String> {
    let target = match target {
        AdoptRequest::Project(name) => AdoptTarget::Project(name),
        AdoptRequest::New(name) => AdoptTarget::New(name),
        AdoptRequest::Shared => AdoptTarget::Shared,
    };
    core.0
        .adopt(&reference, target)
        .await
        .map(|applied| Adopted {
            project: applied.project,
            service: applied.service,
        })
        .map_err(|error: ApplyError| error.to_string())
}

#[tauri::command]
pub async fn get_log(
    core: State<'_, CoreState>,
    since_seconds: Option<u64>,
) -> Result<Vec<LogEntry>, String> {
    Ok(core
        .0
        .log(LogArgs {
            since_seconds,
            project: None,
        })
        .await
        .entries)
}

/// Opens a local server in the default browser. Only `http` on this machine is
/// accepted: the panel never opens anything the person did not start.
#[tauri::command]
#[allow(
    clippy::disallowed_methods,
    reason = "`open` is the only way to hand a URL to the default browser; the argument is validated first"
)]
pub fn open_url(url: String) -> Result<(), String> {
    if !is_local_http(&url) {
        return Err("solo se abren direcciones locales".to_owned());
    }
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map(drop)
        .map_err(|error| error.to_string())
}

fn is_local_http(url: &str) -> bool {
    ["http://localhost", "http://127.0.0.1", "http://[::1]"]
        .iter()
        .any(|prefix| {
            url.strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/']))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_http_urls_are_opened() {
        assert!(is_local_http("http://localhost:8080"));
        assert!(is_local_http("http://localhost"));
        assert!(is_local_http("http://127.0.0.1:3000/app"));
        assert!(!is_local_http("https://localhost:8080"));
        assert!(!is_local_http("http://localhost.evil.com"));
        assert!(!is_local_http("http://localhost@evil.com"));
        assert!(!is_local_http("file:///etc/passwd"));
        assert!(!is_local_http("javascript:alert(1)"));
    }
}
