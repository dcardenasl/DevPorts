//! The typed log entries.

use super::*;

// covers: UXR-6.3
#[test]
pub(super) fn entries_carry_typed_topics_so_a_view_never_reads_the_sentence() {
    use devports_proto::views::LogTopic::{Alert, Rejection, Temporal};
    let topics =
        |kind: &str, payload: serde_json::Value| crate::messages::event_topics(kind, &payload);
    assert_eq!(topics("run_rejected", serde_json::json!({})), [Rejection]);
    assert_eq!(
        topics("run_accepted", serde_json::json!({"role": "temporal"})),
        [Temporal]
    );
    assert!(topics("run_accepted", serde_json::json!({"role": "principal"})).is_empty());
    assert_eq!(
        topics("stop_failed", serde_json::json!({})),
        [Temporal, Alert]
    );
    assert_eq!(topics("lease_expired", serde_json::json!({})), [Temporal]);
    assert_eq!(topics("duplicate_detected", serde_json::json!({})), [Alert]);
    assert!(topics("core_started", serde_json::json!({})).is_empty());
    // A sentence that happens to say "Temporal" is not a topic.
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    let entries = world.engine.log(&LogArgs::default()).unwrap().entries;
    assert!(
        entries
            .iter()
            .all(|entry| !entry.topics.contains(&Temporal))
    );
}

// covers: UXR-6.3
#[test]
pub(super) fn a_store_that_cannot_be_read_is_an_error_not_an_empty_log() {
    let world = world();
    world.engine.store.break_events_for_test();
    let result = world.engine.log(&LogArgs::default());
    assert!(
        matches!(result, Err(crate::engine::LogError::Storage { .. })),
        "{result:?}"
    );
    // The socket says so too, so `devports log` fails instead of printing "no activity".
    let mut world = world;
    let (response, _) = world.engine.handle_request(devports_proto::Request::new(
        "log-1",
        devports_proto::Op::Log(LogArgs::default()),
    ));
    assert!(!response.ok);
    assert_eq!(
        response.error.map(|error| error.code),
        Some(devports_proto::ErrorCode::Unavailable)
    );
}
