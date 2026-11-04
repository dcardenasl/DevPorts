//! The request, view and status types that travel between the panel and the store.
use super::{
    ActionError, AdoptTarget, AdoptionError, Deserialize, ProcessClass, Serialize, ServiceEdits,
    SettingsError, StopExpectation, UsesError,
};

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
