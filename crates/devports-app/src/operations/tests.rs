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
