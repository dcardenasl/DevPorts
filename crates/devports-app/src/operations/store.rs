//! Bounded in-memory operation history and its idempotent bookkeeping.
use super::validation::{aggregate, request_digest, validate};
use super::{
    AdoptedSummary, Arc, ConsumerSummary, MAX_OPERATIONS, Mutex, OperationFailure,
    OperationRequest, OperationStatus, OperationTargetView, OperationView, PoisonError,
    TargetStatus, TypedFailure, VecDeque,
};

#[derive(Debug, Clone)]
pub(super) struct Entry {
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

pub(super) fn is_finished(entry: &Entry) -> bool {
    matches!(
        entry.view.status,
        OperationStatus::Succeeded | OperationStatus::Partial | OperationStatus::Failed
    )
}
