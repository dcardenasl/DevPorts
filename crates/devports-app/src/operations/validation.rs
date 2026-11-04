//! Request validation, the idempotency digest and the aggregate status.
use super::{
    MAX_TARGETS, OperationError, OperationKind, OperationRequest, OperationStatus,
    OperationTargetView, TargetStatus,
};

pub(super) fn request_digest(request: &OperationRequest) -> Result<String, OperationError> {
    serde_json::to_vec(request)
        .map(|bytes| devports_core::hash::sha256_hex(&bytes))
        .map_err(|_| OperationError::InvalidRequest)
}

pub(super) fn validate(request: &OperationRequest) -> Result<(), OperationError> {
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

pub(super) fn aggregate(targets: &[OperationTargetView]) -> OperationStatus {
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
