//! Running a queued operation in the application runtime and reporting it.
use super::{
    ActionError, AdoptedSummary, AppHandle, CoreHandle, Emitter, Manager, OPERATION_EVENT,
    OperationFailure, OperationKind, OperationRequest, OperationStore, OperationTarget,
    OperationView, PANEL_LABEL, UseOutcome, UseReport, UseRequest, missing,
};

/// Starts a queued operation in the application runtime, independent of any panel.
pub fn spawn(app: AppHandle, core: CoreHandle, store: OperationStore, id: String) {
    tauri::async_runtime::spawn(async move {
        execute(app, core, store, id).await;
    });
}

async fn execute(app: AppHandle, core: CoreHandle, store: OperationStore, id: String) {
    let Some(request) = store.request(&id) else {
        return;
    };
    if request.kind == OperationKind::SetConsumers {
        set_consumers(&app, &core, &store, &id, &request).await;
        return;
    }
    for (index, target) in request.targets.iter().enumerate() {
        let Some(changed) = store.begin_target(&id, index) else {
            continue;
        };
        emit(&app, &changed);
        let result = perform(&core, request.kind, target).await;
        if let Some(changed) = store.finish_target(&id, index, result) {
            emit(&app, &changed);
        }
    }
}

/// A consumer change validates every queued manifest before writing any, so its
/// targets are begun together and finished from one core report.
async fn set_consumers(
    app: &AppHandle,
    core: &CoreHandle,
    store: &OperationStore,
    id: &str,
    request: &OperationRequest,
) {
    let mut begun = Vec::new();
    for (index, target) in request.targets.iter().enumerate() {
        if let Some(changed) = store.begin_target(id, index) {
            emit(app, &changed);
            begun.push((index, target));
        }
    }
    let (shared, requests) = consumer_requests(&begun);
    let outcomes = match core.set_shared_consumers(&shared, requests).await {
        Ok(reports) => consumer_outcomes(&reports),
        Err(error) => begun
            .iter()
            .map(|_| Err(OperationFailure::from(error.clone())))
            .collect(),
    };
    for ((index, _), outcome) in begun.iter().zip(outcomes) {
        if let Some(changed) = store.finish_target(id, *index, outcome) {
            emit(app, &changed);
        }
    }
}

pub(super) fn consumer_requests(
    targets: &[(usize, &OperationTarget)],
) -> (String, Vec<UseRequest>) {
    let shared = targets
        .iter()
        .find_map(|(_, target)| target.consumers.as_ref())
        .map(|consumers| consumers.shared.clone())
        .unwrap_or_default();
    let requests = targets
        .iter()
        .filter_map(|(_, target)| {
            target.consumers.as_ref().map(|consumers| UseRequest {
                project: target.reference.clone(),
                include: consumers.include,
                base: consumers.base.clone(),
            })
        })
        .collect();
    (shared, requests)
}

pub(super) fn consumer_outcomes(
    reports: &[UseReport],
) -> Vec<Result<Option<AdoptedSummary>, OperationFailure>> {
    reports
        .iter()
        .map(|report| match &report.result {
            Ok(UseOutcome::Saved | UseOutcome::Unchanged) => Ok(None),
            Err(error) => Err(OperationFailure::from(error.clone())),
        })
        .collect()
}

async fn perform(
    core: &CoreHandle,
    kind: OperationKind,
    target: &OperationTarget,
) -> Result<Option<AdoptedSummary>, OperationFailure> {
    match kind {
        OperationKind::Stop | OperationKind::StopProject => {
            let expected = target
                .stop_expectation
                .clone()
                .ok_or_else(|| missing("the confirmed identity of the target"))?;
            let outcome = if kind == OperationKind::StopProject {
                core.stop_project(&target.reference, expected).await
            } else {
                core.stop(&target.reference, expected).await
            };
            outcome.map(|()| None).map_err(OperationFailure::from)
        }
        OperationKind::Extend => core
            .extend(&target.reference)
            .await
            .map(|()| None)
            .map_err(OperationFailure::from),
        OperationKind::Keep => core
            .ignore(&target.reference)
            .await
            .map(|()| None)
            .map_err(OperationFailure::from),
        OperationKind::SetScanFolders => {
            let folders = target
                .scan_folders
                .clone()
                .ok_or_else(|| missing("the folders"))?;
            core.set_scan_folders(folders.roots, folders.base)
                .await
                .map(|_| None)
                .map_err(OperationFailure::from)
        }
        OperationKind::Classify => {
            let choice = target
                .classification
                .ok_or_else(|| missing("the classification"))?;
            core.set_classification(&target.reference, choice.class)
                .await
                .map(|()| None)
                .map_err(OperationFailure::from)
        }
        OperationKind::Restore => core
            .restore_ignored(&target.reference)
            .await
            .map(|()| None)
            .map_err(OperationFailure::from),
        OperationKind::SetConsumers => Err(OperationFailure::from(ActionError::Internal {
            detail: "consumer changes run as one batch".to_owned(),
        })),
        OperationKind::Adopt => {
            let reviewed = target
                .adoption
                .as_ref()
                .ok_or_else(|| missing("a reviewed adoption"))?;
            core.apply_adoption(&reviewed.id, reviewed.edits.clone())
                .await
                .map(|applied| {
                    Some(AdoptedSummary {
                        project: applied.project,
                        service: applied.service,
                    })
                })
                .map_err(OperationFailure::from)
        }
    }
}

/// Emits only when a panel exists; the store remains the source of truth otherwise.
pub fn emit(app: &AppHandle, operation: &OperationView) {
    if app.get_webview_window(PANEL_LABEL).is_some()
        && let Err(error) = app.emit_to(PANEL_LABEL, OPERATION_EVENT, operation)
    {
        crate::diagnostics::report(&error);
    }
}
