//! Temporal leases: warnings, expiry, extension and tokens.

use super::*;

// covers: U-06
#[test]
pub(super) fn the_warning_comes_once_a_minute_before_the_end_u_06() {
    let (mut world, reference, _, port) = bound_temporal();
    world.clock.advance(Duration::from_mins(28));
    assert!(world.engine.tick().is_empty(), "29 minutes in: nothing yet");

    world.clock.advance(Duration::from_secs(61));
    let effects = world.engine.tick();
    assert_eq!(effects.len(), 1);
    let Effect::Notify(Notice::LeaseExpiring {
        reference: warned,
        port: warned_port,
        ..
    }) = &effects[0]
    else {
        panic!("expected the expiry notice, got {effects:?}");
    };
    assert_eq!(warned, &reference);
    assert_eq!(*warned_port, Some(port));

    world.clock.advance(Duration::from_secs(5));
    assert!(
        world.engine.tick().is_empty(),
        "the warning is not repeated"
    );
    assert!(world.events().contains(&"lease_warned".to_owned()));
}

// covers: U-06, I-03
#[test]
pub(super) fn at_the_end_of_the_lease_the_instance_is_stopped_exactly_once() {
    let (mut world, reference, _, _) = bound_temporal();
    world.clock.advance(Duration::from_mins(30));
    let effects = world.engine.tick();
    let stops: Vec<&TerminationRequest> = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Terminate(request) => Some(request),
            Effect::Notify(_) => None,
        })
        .collect();
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0].reference, reference);
    assert_eq!(stops[0].reason, StopReason::LeaseExpired);
    assert_eq!(
        stops[0].target,
        Target::Process(ProcessId {
            pid: 700,
            start: ProcessStart(
                u64::try_from(START.parse::<Timestamp>().unwrap().as_second() + 1).unwrap()
                    * 1_000_000
            )
        })
    );
    assert_eq!(
        world.instance(Some("shop"), "api", InstanceRole::Temporal)[0].state,
        InstanceState::Stopping
    );

    world.clock.advance(Duration::from_secs(1));
    assert!(
        world
            .engine
            .tick()
            .iter()
            .all(|effect| !matches!(effect, Effect::Terminate(_))),
        "no second stop while the first is in flight"
    );

    world
        .engine
        .termination_finished(&reference, StopReason::LeaseExpired, &Terminated::Stopped);
    assert!(
        world
            .instance(Some("shop"), "api", InstanceRole::Temporal)
            .is_empty()
    );
    assert!(world.events().contains(&"lease_expired".to_owned()));
    assert!(world.engine.tracked.is_empty());
}

#[test]
pub(super) fn what_survives_is_forgotten_and_never_retried() {
    let (mut world, reference, _, _) = bound_temporal();
    world.clock.advance(Duration::from_mins(31));
    world.engine.tick();
    world.engine.termination_finished(
        &reference,
        StopReason::LeaseExpired,
        &Terminated::StillAlive,
    );

    let view = &world.instance(Some("shop"), "api", InstanceRole::Temporal)[0];
    assert_eq!(view.state, InstanceState::Forgotten);
    assert!(world.events().contains(&"stop_failed".to_owned()));
    world.clock.advance(Duration::from_secs(60));
    assert!(
        world
            .engine
            .tick()
            .iter()
            .all(|effect| !matches!(effect, Effect::Terminate(_)))
    );
}

// covers: U-06
#[test]
pub(super) fn extending_adds_time_up_to_the_maximum_and_rearms_the_warning() {
    let (mut world, _, token, _) = bound_temporal();
    world.clock.advance(Duration::from_secs(29 * 60 + 30));
    world.engine.tick();
    let result = world.engine.extend_by_token(&token, None).unwrap();
    assert_eq!(result.leased_total_seconds, 3600);
    // The warning fires again for the new end (60 minutes in).
    world.clock.advance(Duration::from_mins(30));
    let effects = world.engine.tick();
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::Notify(Notice::LeaseExpiring { .. })))
    );

    // 4 hours in total is the ceiling.
    for _ in 0..6 {
        world.engine.extend_by_token(&token, None).unwrap();
    }
    let error = world.engine.extend_by_token(&token, None).unwrap_err();
    assert_eq!(error.code, ErrorCode::LeaseLimit);
    let error = world.engine.extend_by_token(&token, Some(60)).unwrap_err();
    assert_eq!(error.code, ErrorCode::LeaseLimit);
}

// covers: U-08
#[test]
pub(super) fn a_token_that_matches_nothing_is_refused() {
    let (mut world, _, _, _) = bound_temporal();
    assert_eq!(
        world.engine.extend_by_token("nope", None).unwrap_err().code,
        ErrorCode::UnknownToken
    );
    assert_eq!(
        world.engine.stop_by_token("nope").unwrap_err().code,
        ErrorCode::UnknownToken
    );
}

// covers: U-08, I-08
#[test]
pub(super) fn a_token_holder_stops_only_its_own_temporal() {
    let (mut world, reference, token, _) = bound_temporal();
    let other = accepted(&world.run(&request("api", RunRole::Test))).clone();

    let (result, effects) = world.engine.stop_by_token(&token).unwrap();

    assert_eq!(result.instance_ref, reference);
    assert!(result.stopped);
    assert_eq!(effects.len(), 1);
    assert!(
        matches!(&effects[0], Effect::Terminate(request) if request.reference == reference && request.reason == StopReason::TokenRequest)
    );
    // The other Temporal and the Principal are untouched.
    assert!(
        world
            .instance(Some("shop"), "api", InstanceRole::Temporal)
            .iter()
            .any(|view| view.instance_ref == other.instance_ref
                && view.state == InstanceState::Starting)
    );
}

#[test]
pub(super) fn stopping_twice_or_after_the_end_is_not_an_error() {
    let (mut world, reference, token, _) = bound_temporal();
    world.engine.stop_by_token(&token).unwrap();
    world
        .engine
        .termination_finished(&reference, StopReason::TokenRequest, &Terminated::Stopped);
    let (again, effects) = world.engine.stop_by_token(&token).unwrap();
    assert!(!again.stopped);
    assert!(effects.is_empty());
    assert_eq!(again.instance_ref, reference);
}

#[test]
pub(super) fn a_temporal_that_never_bound_is_released_on_stop_without_effects() {
    let mut world = world();
    let temporal = accepted(&world.run(&request("api", RunRole::Test))).clone();
    let (result, effects) = world
        .engine
        .stop_by_token(temporal.token.as_deref().unwrap())
        .unwrap();
    assert!(result.stopped);
    assert!(effects.is_empty());
    assert!(world.engine.tracked.is_empty());
}

// covers: I-10
#[test]
pub(super) fn a_lease_that_expired_while_the_core_was_down_is_stopped_at_once_i_10() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    let (reference, port) = {
        let mut world = world_with(
            Store::open(&path).unwrap(),
            Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
        );
        let temporal = accepted(&world.run(&request("api", RunRole::Test))).clone();
        world.clock.advance(Duration::from_secs(1));
        let now = epoch(&world);
        world.scan(vec![proc(
            700,
            1,
            now,
            "/work/shop/api",
            Some(temporal.port),
            &["php"],
        )]);
        (temporal.instance_ref, temporal.port)
    };

    // The core restarts two hours later and the process is still alive.
    let later = Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap()));
    later.advance(Duration::from_hours(2));
    let mut world = world_with(Store::open(&path).unwrap(), later);
    assert_eq!(
        world.engine.tracked.len(),
        1,
        "the lease survived the restart"
    );
    let now = epoch(&world);
    let started = START.parse::<Timestamp>().unwrap().as_second() + 1;
    let _ = now;
    let effects = world.scan(vec![proc(
        700,
        1,
        started,
        "/work/shop/api",
        Some(port),
        &["php"],
    )]);
    assert!(
        effects
            .iter()
            .all(|effect| !matches!(effect, Effect::Terminate(_))),
        "scan alone does not stop"
    );
    let effects = world.engine.tick();
    assert!(
        effects.iter().any(|effect| matches!(effect, Effect::Terminate(request) if request.reference == reference && request.reason == StopReason::LeaseExpired)),
        "{effects:?}"
    );
}

// ------------------------------------------------------------- people's actions
