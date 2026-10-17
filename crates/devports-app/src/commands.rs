//! The commands the panel calls. They only translate: every rule is the core's.

#![allow(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value; the signatures are fixed"
)]

use devports_core::runtime::Snapshot;
use devports_proto::LogArgs;
use devports_proto::views::LogEntry;
use tauri::{AppHandle, State};

use crate::host::CoreState;
use crate::operations::{self, OperationRequest, OperationStore, OperationView};
use crate::panel_state::{PanelState, PanelStateStore};

#[tauri::command]
pub fn get_snapshot(core: State<'_, CoreState>) -> Snapshot {
    core.0.snapshot().as_ref().clone()
}

#[tauri::command]
pub async fn preview_project_stop(
    core: State<'_, CoreState>,
    project: String,
) -> Result<Vec<devports_proto::views::InstanceView>, String> {
    let targets = core
        .0
        .preview_project_stop(&project)
        .await
        .map_err(|error| error.to_string())?;
    if targets.len() > operations::MAX_TARGETS {
        return Err(
            "hay demasiadas instancias para una sola operación; detén servicios individualmente"
                .to_owned(),
        );
    }
    Ok(targets)
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
pub fn get_operations(store: State<'_, OperationStore>) -> Vec<OperationView> {
    store.list()
}

#[tauri::command]
pub fn get_operation(store: State<'_, OperationStore>, id: String) -> Option<OperationView> {
    store.get(&id)
}

#[tauri::command]
pub fn start_operation(
    app: AppHandle,
    core: State<'_, CoreState>,
    store: State<'_, OperationStore>,
    request: OperationRequest,
) -> Result<OperationView, String> {
    let (operation, start) = store.start(request).map_err(|error| error.to_string())?;
    if start {
        operations::spawn(
            app,
            core.0.clone(),
            store.inner().clone(),
            operation.id.clone(),
        );
    }
    Ok(operation)
}

#[tauri::command]
pub fn retry_operation(
    app: AppHandle,
    core: State<'_, CoreState>,
    store: State<'_, OperationStore>,
    id: String,
) -> Result<OperationView, String> {
    let operation = store.retry(&id).map_err(|error| error.to_string())?;
    operations::spawn(app, core.0.clone(), store.inner().clone(), id);
    Ok(operation)
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
