//! Human-requested operations that outlive the ephemeral panel webview.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use devports_core::engine::StopExpectation;
use devports_core::runtime::{AdoptTarget as CoreAdoptTarget, CoreHandle};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::panel::PANEL_LABEL;

const MAX_OPERATIONS: usize = 48;
const MAX_TARGETS: usize = 64;

/// Event carrying an operation snapshot to an open panel.
pub const OPERATION_EVENT: &str = "operation";

/// An idempotent request from the panel.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationRequest {
    /// Client-generated idempotency key, reused after a lost response.
    pub id: String,
    /// Human action to perform.
    pub kind: OperationKind,
    /// Concrete targets captured by the panel.
    pub targets: Vec<OperationTarget>,
}

/// Supported human operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    /// Stop an observed instance.
    Stop,
    /// Extend a temporary instance.
    Extend,
    /// Ignore an observed instance.
    Keep,
    /// Adopt an observed instance into a declared destination.
    Adopt,
}

/// One concrete instance and optional adoption destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationTarget {
    /// Stable instance reference, never a PID or token.
    pub reference: String,
    /// Destination is present only for adoption.
    #[serde(default)]
    pub adopt_target: Option<AdoptTarget>,
    /// Exact facts displayed in the manual-stop confirmation.
    #[serde(default)]
    pub stop_expectation: Option<StopExpectation>,
}

/// Adoption destination accepted by the operation contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptTarget {
    /// Existing project name.
    Project(String),
    /// New project name, or use the process folder when absent.
    New(Option<String>),
    /// The shared resources manifest.
    Shared,
}

/// Aggregate operation lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationStatus {
    /// Accepted but not yet executing.
    Queued,
    /// One or more targets are executing.
    Running,
    /// Every target succeeded.
    Succeeded,
    /// At least one target succeeded and another failed.
    Partial,
    /// Every completed target failed.
    Failed,
}

/// One target's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetStatus {
    /// Waiting to execute.
    Queued,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Succeeded,
    /// Failed and available for explicit retry.
    Failed,
}

/// Safe destination summary returned after an adoption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptedSummary {
    /// Project or shared destination returned by the core.
    pub project: String,
    /// Service name written by the core.
    pub service: String,
}

/// Public result for one target; contains no command arguments or tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationTargetView {
    /// Stable instance reference.
    pub reference: String,
    /// Requested adoption destination, if any.
    pub adopt_target: Option<AdoptTarget>,
    /// Minimal confirmed stop identity, never process arguments or secrets.
    pub stop_expectation: Option<StopExpectation>,
    /// Current target lifecycle.
    pub status: TargetStatus,
    /// Human-readable failure reason, retained until retry succeeds.
    pub error: Option<String>,
    /// Actual destination returned on a successful adoption.
    pub result: Option<AdoptedSummary>,
    /// Number of execution attempts, including the initial attempt.
    pub attempts: u32,
}

/// Public, queryable operation snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationView {
    /// Idempotency key.
    pub id: String,
    /// Human action.
    pub kind: OperationKind,
    /// Aggregate lifecycle.
    pub status: OperationStatus,
    /// Monotonic snapshot revision for event/query reconciliation.
    pub revision: u32,
    /// Target outcomes in the original confirmed order.
    pub targets: Vec<OperationTargetView>,
}

#[derive(Debug, Clone)]
struct Entry {
    request: OperationRequest,
    view: OperationView,
}

/// Bounded in-memory operation history owned by the application process.
#[derive(Debug, Clone, Default)]
pub struct OperationStore(Arc<Mutex<VecDeque<Entry>>>);

/// Failure to accept or retry an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OperationError {
    /// The request shape or its identifiers are invalid.
    #[error("invalid operation request")]
    InvalidRequest,
    /// An id was reused for a different request.
    #[error("operation id already belongs to a different request")]
    IdConflict,
    /// The bounded history is full of active operations.
    #[error("too many operations are still running")]
    AtCapacity,
    /// The id is not known to this app process.
    #[error("operation was not found")]
    NotFound,
    /// The operation has no failed targets to retry.
    #[error("operation has no failed targets to retry")]
    NothingToRetry,
}

impl OperationStore {
    /// Accepts a request exactly once. Repeating the same id and payload returns the
    /// original snapshot and does not schedule a second execution.
    pub fn start(
        &self,
        request: OperationRequest,
    ) -> Result<(OperationView, bool), OperationError> {
        validate(&request)?;
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(existing) = entries.iter().find(|entry| entry.view.id == request.id) {
            if existing.request == request {
                return Ok((existing.view.clone(), false));
            }
            return Err(OperationError::IdConflict);
        }
        while entries.len() >= MAX_OPERATIONS {
            if let Some(index) = entries.iter().position(is_finished) {
                entries.remove(index);
            } else {
                return Err(OperationError::AtCapacity);
            }
        }
        let targets = request
            .targets
            .iter()
            .map(|target| OperationTargetView {
                reference: target.reference.clone(),
                adopt_target: target.adopt_target.clone(),
                stop_expectation: target.stop_expectation.clone(),
                status: TargetStatus::Queued,
                error: None,
                result: None,
                attempts: 0,
            })
            .collect();
        let view = OperationView {
            id: request.id.clone(),
            kind: request.kind,
            status: OperationStatus::Queued,
            revision: 0,
            targets,
        };
        entries.push_back(Entry {
            request,
            view: view.clone(),
        });
        Ok((view, true))
    }

    /// Returns the newest-first operation snapshots.
    pub fn list(&self) -> Vec<OperationView> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .rev()
            .map(|entry| entry.view.clone())
            .collect()
    }

    /// Returns one operation snapshot.
    pub fn get(&self, id: &str) -> Option<OperationView> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|entry| entry.view.id == id)
            .map(|entry| entry.view.clone())
    }

    /// Marks one queued target as running and returns the changed snapshot.
    pub fn begin_target(&self, id: &str, index: usize) -> Option<OperationView> {
        self.update_target(id, index, |target| {
            if target.status != TargetStatus::Queued {
                return false;
            }
            target.status = TargetStatus::Running;
            target.attempts = target.attempts.saturating_add(1);
            true
        })
    }

    /// Completes a running target with its safe result or user-facing error.
    pub fn finish_target(
        &self,
        id: &str,
        index: usize,
        outcome: Result<Option<AdoptedSummary>, String>,
    ) -> Option<OperationView> {
        self.update_target(id, index, |target| {
            if target.status != TargetStatus::Running {
                return false;
            }
            match outcome {
                Ok(result) => {
                    target.status = TargetStatus::Succeeded;
                    target.error = None;
                    target.result = result;
                }
                Err(error) => {
                    target.status = TargetStatus::Failed;
                    target.error = Some(error);
                }
            }
            true
        })
    }

    /// Resets only failed targets to queued for an explicit retry.
    pub fn retry(&self, id: &str) -> Result<OperationView, OperationError> {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = entries
            .iter_mut()
            .find(|entry| entry.view.id == id)
            .ok_or(OperationError::NotFound)?;
        let mut changed = false;
        for target in &mut entry.view.targets {
            if target.status == TargetStatus::Failed {
                target.status = TargetStatus::Queued;
                target.error = None;
                changed = true;
            }
        }
        if !changed {
            return Err(OperationError::NothingToRetry);
        }
        entry.view.status = OperationStatus::Queued;
        entry.view.revision = entry.view.revision.saturating_add(1);
        Ok(entry.view.clone())
    }

    /// Returns the immutable request details needed to execute queued targets.
    pub fn request(&self, id: &str) -> Option<OperationRequest> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|entry| entry.view.id == id)
            .map(|entry| entry.request.clone())
    }

    fn update_target(
        &self,
        id: &str,
        index: usize,
        update: impl FnOnce(&mut OperationTargetView) -> bool,
    ) -> Option<OperationView> {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = entries.iter_mut().find(|entry| entry.view.id == id)?;
        let target = entry.view.targets.get_mut(index)?;
        if !update(target) {
            return None;
        }
        entry.view.status = aggregate(&entry.view.targets);
        entry.view.revision = entry.view.revision.saturating_add(1);
        Some(entry.view.clone())
    }
}

fn validate(request: &OperationRequest) -> Result<(), OperationError> {
    if request.id.is_empty()
        || request.id.len() > 64
        || !request
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        || request.targets.is_empty()
        || request.targets.len() > MAX_TARGETS
    {
        return Err(OperationError::InvalidRequest);
    }
    let mut references = std::collections::HashSet::new();
    for target in &request.targets {
        if target.reference.is_empty()
            || target.reference.len() > 128
            || !references.insert(&target.reference)
            || (request.kind == OperationKind::Adopt) != target.adopt_target.is_some()
            || (request.kind == OperationKind::Stop) != target.stop_expectation.is_some()
            || (request.kind == OperationKind::Stop
                && target
                    .stop_expectation
                    .as_ref()
                    .and_then(|expectation| expectation.target_identity.as_ref())
                    .is_none())
        {
            return Err(OperationError::InvalidRequest);
        }
    }
    Ok(())
}

fn aggregate(targets: &[OperationTargetView]) -> OperationStatus {
    let succeeded = targets
        .iter()
        .filter(|target| target.status == TargetStatus::Succeeded)
        .count();
    let failed = targets
        .iter()
        .filter(|target| target.status == TargetStatus::Failed)
        .count();
    let active = targets
        .iter()
        .any(|target| matches!(target.status, TargetStatus::Queued | TargetStatus::Running));
    if active {
        if targets
            .iter()
            .any(|target| target.status == TargetStatus::Running)
        {
            OperationStatus::Running
        } else {
            OperationStatus::Queued
        }
    } else if failed > 0 && succeeded > 0 {
        OperationStatus::Partial
    } else if failed > 0 {
        OperationStatus::Failed
    } else {
        OperationStatus::Succeeded
    }
}

fn is_finished(entry: &Entry) -> bool {
    matches!(
        entry.view.status,
        OperationStatus::Succeeded | OperationStatus::Partial | OperationStatus::Failed
    )
}

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

async fn perform(
    core: &CoreHandle,
    kind: OperationKind,
    target: &OperationTarget,
) -> Result<Option<AdoptedSummary>, String> {
    match kind {
        OperationKind::Stop => core
            .stop(
                &target.reference,
                target
                    .stop_expectation
                    .clone()
                    .ok_or_else(|| "falta confirmar la identidad del objetivo".to_owned())?,
            )
            .await
            .map(|()| None)
            .map_err(|error| error.to_string()),
        OperationKind::Extend => core
            .extend(&target.reference)
            .await
            .map(|()| None)
            .map_err(|error| error.to_string()),
        OperationKind::Keep => core
            .ignore(&target.reference)
            .await
            .map(|()| None)
            .map_err(|error| error.to_string()),
        OperationKind::Adopt => {
            let Some(destination) = target.adopt_target.clone() else {
                return Err("no se especificó el destino de adopción".to_owned());
            };
            let destination = match destination {
                AdoptTarget::Project(name) => CoreAdoptTarget::Project(name),
                AdoptTarget::New(name) => CoreAdoptTarget::New(name),
                AdoptTarget::Shared => CoreAdoptTarget::Shared,
            };
            core.adopt(&target.reference, destination)
                .await
                .map(|applied| {
                    Some(AdoptedSummary {
                        project: applied.project,
                        service: applied.service,
                    })
                })
                .map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use devports_proto::views::InstanceRole;

    fn stop_expectation() -> StopExpectation {
        StopExpectation {
            target_identity: Some("confirmed-target".to_owned()),
            pid: Some(42),
            container: None,
            port: Some(8080),
            project: Some("demo".to_owned()),
            service: Some("api".to_owned()),
            kind: "app".to_owned(),
            role: InstanceRole::Principal,
            shared: false,
            started_at: Some("2026-10-17T12:00:00Z".to_owned()),
        }
    }

    fn request(id: &str) -> OperationRequest {
        OperationRequest {
            id: id.to_owned(),
            kind: OperationKind::Stop,
            targets: vec![
                OperationTarget {
                    reference: "first".to_owned(),
                    adopt_target: None,
                    stop_expectation: Some(stop_expectation()),
                },
                OperationTarget {
                    reference: "second".to_owned(),
                    adopt_target: None,
                    stop_expectation: Some(stop_expectation()),
                },
            ],
        }
    }

    fn fail_target(store: &OperationStore, id: &str, index: usize, error: &str) {
        store.begin_target(id, index).expect("target begins");
        store
            .finish_target(id, index, Err(error.to_owned()))
            .expect("target finishes");
    }

    // covers: UXR-1.3
    #[test]
    fn repeating_an_identical_idempotency_key_does_not_queue_another_run() {
        let store = OperationStore::default();
        let original = request("operation-1");
        assert!(store.start(original.clone()).expect("first start").1);
        let (repeated, should_run) = store.start(original).expect("same request");
        assert!(!should_run);
        assert_eq!(
            repeated,
            store.get("operation-1").expect("stored operation")
        );
        assert_eq!(store.list().len(), 1);
    }

    // covers: UXR-1.3
    #[test]
    fn reusing_an_id_for_a_different_request_is_rejected() {
        let store = OperationStore::default();
        store.start(request("operation-1")).expect("first start");
        let mut different = request("operation-1");
        different.targets[0].reference = "changed".to_owned();
        assert_eq!(store.start(different), Err(OperationError::IdConflict));
    }

    // covers: UXR-1.3
    #[test]
    fn partial_retry_resets_only_failed_targets_and_keeps_successes() {
        let store = OperationStore::default();
        store.start(request("operation-1")).expect("start");
        store.begin_target("operation-1", 0).expect("first begins");
        store
            .finish_target("operation-1", 0, Ok(None))
            .expect("first succeeds");
        fail_target(&store, "operation-1", 1, "target changed");
        let partial = store.get("operation-1").expect("partial result");
        assert_eq!(partial.status, OperationStatus::Partial);
        let retried = store.retry("operation-1").expect("retry pending target");
        assert_eq!(retried.targets[0].status, TargetStatus::Succeeded);
        assert_eq!(retried.targets[1].status, TargetStatus::Queued);
        assert_eq!(retried.status, OperationStatus::Queued);
    }

    // covers: UXR-1.3
    #[test]
    fn request_validation_rejects_duplicate_targets_and_mismatched_destinations() {
        let store = OperationStore::default();
        let mut duplicate = request("operation-1");
        duplicate.targets[1].reference = "first".to_owned();
        assert_eq!(store.start(duplicate), Err(OperationError::InvalidRequest));

        let mut missing_destination = request("operation-2");
        missing_destination.kind = OperationKind::Adopt;
        assert_eq!(
            store.start(missing_destination),
            Err(OperationError::InvalidRequest)
        );
    }

    // covers: UXR-1.3
    #[test]
    fn history_is_bounded_and_never_evicts_an_active_operation() {
        let store = OperationStore::default();
        for index in 0..MAX_OPERATIONS {
            store
                .start(request(&format!("op-{index}")))
                .expect("within capacity");
        }
        assert_eq!(
            store.start(request("overflow")),
            Err(OperationError::AtCapacity)
        );
        fail_target(&store, "op-0", 0, "failed");
        fail_target(&store, "op-0", 1, "failed");
        store
            .start(request("overflow"))
            .expect("evicts old finished operation");
        assert!(store.get("op-0").is_none());
        assert!(store.get("op-1").is_some());
    }
}
