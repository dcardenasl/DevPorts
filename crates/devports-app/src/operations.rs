//! Human-requested operations that outlive the ephemeral panel webview.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use devports_core::adopt::review::{AdoptionError, AdoptionTarget as AdoptTarget, ServiceEdits};
use devports_core::classify::ProcessClass;
use devports_core::engine::{ActionError, StopExpectation};
use devports_core::reasons::CommandError;
use devports_core::runtime::CoreHandle;
use devports_core::settings::SettingsError;
use devports_core::uses::{UseOutcome, UseReport, UseRequest, UsesError};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::panel::PANEL_LABEL;

const MAX_OPERATIONS: usize = 48;
pub(crate) const MAX_TARGETS: usize = 64;

/// Event carrying an operation snapshot to an open panel.
pub const OPERATION_EVENT: &str = "operation";

/// An idempotent request from the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Stop only confirmed application instances belonging to a project.
    StopProject,
    /// Extend a temporary instance.
    Extend,
    /// Ignore an observed instance.
    Keep,
    /// Forget one saved ignore rule; the target `reference` is the rule id.
    Restore,
    /// Move a loose process to another panel group; the target `reference` is the instance.
    Classify,
    /// Replace the scan folders of `config.yml`; one target, `reference` is `config`.
    SetScanFolders,
    /// Adopt an observed instance into a declared destination.
    Adopt,
    /// Declare or remove one shared service in the confirmed projects' manifests.
    SetConsumers,
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
    /// Frozen review and exact confirmed fields; never exposed in operation history.
    #[serde(default)]
    pub adoption: Option<AdoptionSubmission>,
    /// Exact facts displayed in the manual-stop confirmation.
    #[serde(default)]
    pub stop_expectation: Option<StopExpectation>,
    /// Confirmed state of this project, present only for consumer changes. The
    /// target `reference` is the project name.
    #[serde(default)]
    pub consumers: Option<ConsumerSubmission>,
    /// Where the person wants the process listed, for classification changes.
    #[serde(default)]
    pub classification: Option<ClassificationChoice>,
    /// The reviewed folder list, for scan-folder changes.
    #[serde(default)]
    pub scan_folders: Option<ScanFoldersSubmission>,
}

/// The folders the person reviewed and the version of `config.yml` they were read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanFoldersSubmission {
    /// Folders exactly as they will be written.
    pub roots: Vec<String>,
    /// Version token of the reviewed file.
    pub base: String,
}

/// A classification correction; `class: None` goes back to the automatic guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationChoice {
    /// The chosen group, or `None` for automatic.
    pub class: Option<ProcessClass>,
}

/// One project's confirmed association with a shared service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerSubmission {
    /// The shared service; identical across the targets of one operation.
    pub shared: String,
    /// Declare (`true`) or stop declaring (`false`) it.
    pub include: bool,
    /// The reviewed version token of the project's manifest.
    pub base: String,
}

/// Public part of a [`ConsumerSubmission`]; the version token stays in the shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerSummary {
    /// The shared service.
    pub shared: String,
    /// Requested state.
    pub include: bool,
}

/// Exact reviewed adoption submitted by the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptionSubmission {
    /// Opaque core review identifier.
    pub id: String,
    /// Exact confirmed editable values.
    pub edits: ServiceEdits,
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
    /// Review identifier only, for reconnecting a draft without exposing commands.
    pub adoption_review: Option<String>,
    /// Typed adoption failure, alongside the human-readable message.
    pub adoption_error: Option<AdoptionError>,
    /// Requested consumer state, for consumer changes.
    pub consumers: Option<ConsumerSummary>,
    /// Requested classification, for classification changes.
    pub classification: Option<ClassificationChoice>,
    /// Number of folders requested, for scan-folder changes; the list itself stays in the shell.
    pub scan_folder_count: Option<u32>,
    /// Typed scan-folder failure, alongside the human-readable message.
    pub settings_error: Option<SettingsError>,
    /// Typed failure of an action on an instance or a rule, alongside the message.
    pub action_error: Option<ActionError>,
    /// Typed consumer-change failure, alongside the human-readable message.
    pub uses_error: Option<UsesError>,
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
    digest: String,
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
        let digest = request_digest(&request)?;
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(existing) = entries.iter().find(|entry| entry.view.id == request.id) {
            if existing.digest == digest {
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
                adoption_review: target.adoption.as_ref().map(|adoption| adoption.id.clone()),
                adoption_error: None,
                consumers: target.consumers.as_ref().map(|consumers| ConsumerSummary {
                    shared: consumers.shared.clone(),
                    include: consumers.include,
                }),
                uses_error: None,
                classification: target.classification,
                scan_folder_count: target
                    .scan_folders
                    .as_ref()
                    .map(|folders| u32::try_from(folders.roots.len()).unwrap_or(u32::MAX)),
                settings_error: None,
                action_error: None,
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
            digest,
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
        outcome: Result<Option<AdoptedSummary>, OperationFailure>,
    ) -> Option<OperationView> {
        self.update_target(id, index, |target| {
            if target.status != TargetStatus::Running {
                return false;
            }
            match outcome {
                Ok(result) => {
                    target.status = TargetStatus::Succeeded;
                    target.error = None;
                    target.adoption_error = None;
                    target.uses_error = None;
                    target.settings_error = None;
                    target.action_error = None;
                    target.result = result;
                }
                Err(error) => {
                    target.status = TargetStatus::Failed;
                    target.error = Some(error.message);
                    (
                        target.adoption_error,
                        target.uses_error,
                        target.settings_error,
                        target.action_error,
                    ) = (None, None, None, None);
                    match error.typed.map(|typed| *typed) {
                        Some(TypedFailure::Adoption(error)) => target.adoption_error = Some(error),
                        Some(TypedFailure::Uses(error)) => target.uses_error = Some(error),
                        Some(TypedFailure::Settings(error)) => target.settings_error = Some(error),
                        Some(TypedFailure::Action(error)) => target.action_error = Some(error),
                        None => {}
                    }
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
        // Successful receipts retain a digest, not a copy of the command.
        for (request, result) in entry.request.targets.iter_mut().zip(&entry.view.targets) {
            if result.status == TargetStatus::Succeeded {
                request.adoption = None;
            }
        }
        Some(entry.view.clone())
    }
}

/// The typed form of a failure, for the sheet that started the operation.
enum TypedFailure {
    Adoption(AdoptionError),
    Uses(UsesError),
    Settings(SettingsError),
    Action(ActionError),
}

/// Safe failure payload retained by the shell, without command arguments. The typed
/// part is boxed so a `Result` carrying it stays small.
pub struct OperationFailure {
    message: String,
    typed: Option<Box<TypedFailure>>,
}

impl OperationFailure {
    fn typed(message: String, typed: TypedFailure) -> Self {
        Self {
            message,
            typed: Some(Box::new(typed)),
        }
    }
}

impl From<String> for OperationFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            typed: None,
        }
    }
}

impl From<AdoptionError> for OperationFailure {
    fn from(error: AdoptionError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Adoption(error))
    }
}

impl From<SettingsError> for OperationFailure {
    fn from(error: SettingsError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Settings(error))
    }
}

impl From<ActionError> for OperationFailure {
    fn from(error: ActionError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Action(error))
    }
}

/// A request that reached the shell without what its kind needs (the panel checks first).
fn missing(what: &str) -> OperationFailure {
    OperationFailure::from(ActionError::Internal {
        detail: format!("the request is missing {what}"),
    })
}

impl From<OperationError> for CommandError {
    fn from(error: OperationError) -> Self {
        match error {
            OperationError::InvalidRequest => Self::OperationInvalid,
            OperationError::IdConflict => Self::OperationIdConflict,
            OperationError::AtCapacity => Self::OperationAtCapacity,
            OperationError::NotFound => Self::OperationNotFound,
            OperationError::NothingToRetry => Self::NothingToRetry,
        }
    }
}

impl From<UsesError> for OperationFailure {
    fn from(error: UsesError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Uses(error))
    }
}

fn request_digest(request: &OperationRequest) -> Result<String, OperationError> {
    serde_json::to_vec(request)
        .map(|bytes| devports_core::hash::sha256_hex(&bytes))
        .map_err(|_| OperationError::InvalidRequest)
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
    let stopping = matches!(
        request.kind,
        OperationKind::Stop | OperationKind::StopProject
    );
    let project = request
        .targets
        .first()
        .and_then(|target| target.stop_expectation.as_ref())
        .and_then(|expected| expected.project.as_deref());
    if request.kind == OperationKind::StopProject && project.is_none() {
        return Err(OperationError::InvalidRequest);
    }
    // All targets of one consumer change refer to the same shared service.
    let shared = request
        .targets
        .first()
        .and_then(|target| target.consumers.as_ref())
        .map(|consumers| consumers.shared.clone());
    let mut references = std::collections::HashSet::new();
    for target in &request.targets {
        if target.reference.is_empty()
            || target.reference.len() > 128
            || !references.insert(&target.reference)
            || (request.kind == OperationKind::StopProject
                && target.stop_expectation.as_ref().is_none_or(|expected| {
                    expected.project.as_deref() != project
                        || expected.kind != "app"
                        || expected.shared
                }))
            || (request.kind == OperationKind::Adopt) != target.adopt_target.is_some()
            || (request.kind == OperationKind::Adopt) != target.adoption.is_some()
            || (request.kind == OperationKind::SetConsumers) != target.consumers.is_some()
            || (request.kind == OperationKind::Classify) != target.classification.is_some()
            || (request.kind == OperationKind::SetScanFolders) != target.scan_folders.is_some()
            || (request.kind == OperationKind::SetScanFolders && request.targets.len() != 1)
            || target.scan_folders.as_ref().is_some_and(|folders| {
                folders.roots.len() > devports_core::settings::MAX_ROOTS
                    || folders.roots.iter().any(|root| root.len() > 1024)
                    || folders.base.is_empty()
                    || folders.base.len() > 64
            })
            || target.consumers.as_ref().is_some_and(|consumers| {
                !devports_core::manifest::is_valid_name(&consumers.shared)
                    || consumers.base.is_empty()
                    || consumers.base.len() > 64
                    || Some(&consumers.shared) != shared.as_ref()
            })
            || target.adoption.as_ref().is_some_and(|adoption| {
                adoption.id.is_empty()
                    || adoption.id.len() > 64
                    || adoption.edits.name.len() > 63
                    || adoption
                        .edits
                        .start
                        .as_ref()
                        .is_some_and(|start| start.len() > 16_384)
            })
            || stopping != target.stop_expectation.is_some()
            || (stopping
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

fn consumer_requests(targets: &[(usize, &OperationTarget)]) -> (String, Vec<UseRequest>) {
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

fn consumer_outcomes(
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
                    adoption: None,
                    consumers: None,
                    classification: None,
                    scan_folders: None,
                    stop_expectation: Some(stop_expectation()),
                },
                OperationTarget {
                    reference: "second".to_owned(),
                    adopt_target: None,
                    adoption: None,
                    consumers: None,
                    classification: None,
                    scan_folders: None,
                    stop_expectation: Some(stop_expectation()),
                },
            ],
        }
    }

    fn fail_target(store: &OperationStore, id: &str, index: usize, error: &str) {
        store.begin_target(id, index).expect("target begins");
        store
            .finish_target(id, index, Err(error.to_owned().into()))
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
        let mut project = request("operation-1");
        project.kind = OperationKind::StopProject;
        store.start(project).expect("start");
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

    // covers: UXR-2.2
    #[test]
    fn project_stop_requests_cannot_mix_projects_or_include_data_and_shared_targets() {
        for policy in ["project", "data", "shared", "missing"] {
            let store = OperationStore::default();
            let mut project = request("project-stop");
            project.kind = OperationKind::StopProject;
            let expected = project.targets[1]
                .stop_expectation
                .as_mut()
                .expect("expectation");
            match policy {
                "project" => expected.project = Some("another".to_owned()),
                "data" => expected.kind = "data".to_owned(),
                "shared" => expected.shared = true,
                _ => expected.project = None,
            }
            assert_eq!(store.start(project), Err(OperationError::InvalidRequest));
        }
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

    fn consumer_request(id: &str, shared: &[&str]) -> OperationRequest {
        OperationRequest {
            id: id.to_owned(),
            kind: OperationKind::SetConsumers,
            targets: shared
                .iter()
                .enumerate()
                .map(|(index, shared)| OperationTarget {
                    reference: format!("project-{index}"),
                    adopt_target: None,
                    adoption: None,
                    consumers: Some(ConsumerSubmission {
                        shared: (*shared).to_owned(),
                        include: true,
                        base: "a".repeat(64),
                    }),
                    classification: None,
                    scan_folders: None,
                    stop_expectation: None,
                })
                .collect(),
        }
    }

    // covers: UXR-4.1
    #[test]
    fn consumer_changes_need_one_shared_service_and_a_version_per_project() {
        let store = OperationStore::default();
        assert!(
            store
                .start(consumer_request("c-1", &["mysql", "mysql"]))
                .is_ok()
        );
        assert_eq!(
            store.start(consumer_request("c-2", &["mysql", "redis"])),
            Err(OperationError::InvalidRequest)
        );
        let mut without_base = consumer_request("c-3", &["mysql"]);
        without_base.targets[0]
            .consumers
            .as_mut()
            .expect("consumers")
            .base
            .clear();
        assert_eq!(
            store.start(without_base),
            Err(OperationError::InvalidRequest)
        );
        let mut wrong_kind = consumer_request("c-4", &["mysql"]);
        wrong_kind.kind = OperationKind::Keep;
        assert_eq!(store.start(wrong_kind), Err(OperationError::InvalidRequest));
        let mut missing = consumer_request("c-5", &["mysql"]);
        missing.targets[0].consumers = None;
        assert_eq!(store.start(missing), Err(OperationError::InvalidRequest));
    }

    // covers: UXR-4.1
    #[test]
    fn consumer_results_are_per_project_typed_and_hide_the_version_token() {
        let store = OperationStore::default();
        store
            .start(consumer_request("c-1", &["mysql", "mysql", "mysql"]))
            .expect("start");
        let targets: Vec<_> = store
            .request("c-1")
            .expect("request")
            .targets
            .into_iter()
            .enumerate()
            .collect();
        let borrowed: Vec<(usize, &OperationTarget)> = targets
            .iter()
            .map(|(index, target)| (*index, target))
            .collect();
        let (shared, requests) = consumer_requests(&borrowed);
        assert_eq!(shared, "mysql");
        assert_eq!(requests.len(), 3);
        let reports = vec![
            UseReport {
                project: "project-0".to_owned(),
                result: Ok(UseOutcome::Saved),
            },
            UseReport {
                project: "project-1".to_owned(),
                result: Err(UsesError::Io {
                    detail: "disk full".to_owned(),
                }),
            },
            UseReport {
                project: "project-2".to_owned(),
                result: Ok(UseOutcome::Unchanged),
            },
        ];
        for (index, outcome) in consumer_outcomes(&reports).into_iter().enumerate() {
            store.begin_target("c-1", index).expect("begins");
            store
                .finish_target("c-1", index, outcome)
                .expect("finishes");
        }
        let view = store.get("c-1").expect("view");
        assert_eq!(view.status, OperationStatus::Partial);
        assert_eq!(
            view.targets[1].uses_error,
            Some(UsesError::Io {
                detail: "disk full".to_owned()
            })
        );
        assert!(view.targets[0].uses_error.is_none());
        let wire = serde_json::to_string(&view).expect("serializes");
        assert!(
            !wire.contains(&"a".repeat(64)),
            "the version token stays in the shell"
        );
        // Retry re-queues only the failed project.
        let retried = store.retry("c-1").expect("retry");
        assert_eq!(retried.targets[0].status, TargetStatus::Succeeded);
        assert_eq!(retried.targets[1].status, TargetStatus::Queued);
        assert_eq!(retried.targets[2].status, TargetStatus::Succeeded);
    }

    // covers: UXR-5.1
    #[test]
    fn a_restore_names_only_a_rule_and_can_be_retried() {
        let store = OperationStore::default();
        let restore = OperationRequest {
            id: "restore-1".to_owned(),
            kind: OperationKind::Restore,
            targets: vec![OperationTarget {
                reference: "0123456789abcdef0123456789abcdef".to_owned(),
                adopt_target: None,
                adoption: None,
                consumers: None,
                classification: None,
                scan_folders: None,
                stop_expectation: None,
            }],
        };
        assert!(store.start(restore.clone()).is_ok());
        fail_target(&store, "restore-1", 0, "the store is busy");
        assert_eq!(
            store.retry("restore-1").expect("retry").targets[0].status,
            TargetStatus::Queued
        );
        let mut with_stop = restore;
        with_stop.id = "restore-2".to_owned();
        with_stop.targets[0].stop_expectation = Some(stop_expectation());
        assert_eq!(store.start(with_stop), Err(OperationError::InvalidRequest));
    }

    // covers: UXR-5.2
    #[test]
    fn a_classification_names_one_instance_and_may_go_back_to_automatic() {
        let store = OperationStore::default();
        let request = |id: &str, class: Option<ProcessClass>| OperationRequest {
            id: id.to_owned(),
            kind: OperationKind::Classify,
            targets: vec![OperationTarget {
                reference: "a1b2".to_owned(),
                adopt_target: None,
                adoption: None,
                consumers: None,
                classification: Some(ClassificationChoice { class }),
                scan_folders: None,
                stop_expectation: None,
            }],
        };
        assert!(
            store
                .start(request("c-1", Some(ProcessClass::Auxiliary)))
                .is_ok()
        );
        assert!(store.start(request("c-2", None)).is_ok());
        let mut bare = request("c-3", None);
        bare.targets[0].classification = None;
        assert_eq!(store.start(bare), Err(OperationError::InvalidRequest));
        let mut wrong_kind = request("c-4", None);
        wrong_kind.kind = OperationKind::Keep;
        assert_eq!(store.start(wrong_kind), Err(OperationError::InvalidRequest));
        let wire = serde_json::to_string(&store.get("c-2").expect("view")).expect("serializes");
        assert!(wire.contains("\"classification\":{\"class\":null}"));
    }

    // covers: UXR-6.1
    #[test]
    fn a_scan_folder_change_is_one_bounded_target_with_a_version() {
        let store = OperationStore::default();
        let request = |id: &str, roots: Vec<String>, base: &str| OperationRequest {
            id: id.to_owned(),
            kind: OperationKind::SetScanFolders,
            targets: vec![OperationTarget {
                reference: "config".to_owned(),
                adopt_target: None,
                adoption: None,
                consumers: None,
                classification: None,
                scan_folders: Some(ScanFoldersSubmission {
                    roots,
                    base: base.to_owned(),
                }),
                stop_expectation: None,
            }],
        };
        let base = "a".repeat(64);
        assert!(
            store
                .start(request("s-1", vec!["/work".to_owned()], &base))
                .is_ok()
        );
        assert_eq!(
            store.start(request("s-2", vec!["x".repeat(2000)], &base)),
            Err(OperationError::InvalidRequest)
        );
        assert_eq!(
            store.start(request("s-3", vec!["/work".to_owned()], "")),
            Err(OperationError::InvalidRequest)
        );
        let many = (0..=devports_core::settings::MAX_ROOTS)
            .map(|n| format!("/w{n}"))
            .collect();
        assert_eq!(
            store.start(request("s-4", many, &base)),
            Err(OperationError::InvalidRequest)
        );
        let mut wrong_kind = request("s-5", vec!["/work".to_owned()], &base);
        wrong_kind.kind = OperationKind::Keep;
        assert_eq!(store.start(wrong_kind), Err(OperationError::InvalidRequest));
        fail_target(&store, "s-1", 0, "config.yml is read-only");
        let wire = serde_json::to_string(&store.get("s-1").expect("view")).expect("serializes");
        assert!(wire.contains("\"scanFolderCount\":1"));
        assert!(
            !wire.contains(&base),
            "the version token stays in the shell"
        );
        assert_eq!(
            store.retry("s-1").expect("retry").targets[0].status,
            TargetStatus::Queued
        );
    }
}
