//! Binding a registered instance to the process that serves it.

use super::*;

// covers: U-10
#[test]
pub(super) fn the_launcher_itself_is_bound_after_exec() {
    let (world, reference) = principal_world();
    // `exec` keeps the launcher's pid: the server IS process 500.
    let launcher_start = epoch(&world);
    world.source.set(vec![proc(
        500,
        1,
        launcher_start,
        "/work/shop/api",
        None,
        &["devports"],
    )]);
    // Re-register so the launcher's identity is known: the engine reads it at run time.
    drop(world);
    let mut world = world_with_launcher(500, launcher_start);
    let reference2 = accepted(&world.run(&request("api", RunRole::Principal)))
        .instance_ref
        .clone();
    world.clock.advance(Duration::from_secs(2));
    let now = epoch(&world);
    world.scan(vec![proc(
        500,
        1,
        launcher_start,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    let view = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert_eq!(view.instance_ref, reference2);
    assert_eq!(view.state, InstanceState::Healthy, "{view:?} at {now}");
    assert_eq!(view.pid, Some(500));
    assert!(world.events().contains(&"bound".to_owned()));
    let _ = reference;
}

pub(super) fn world_with_launcher(pid: u32, start_secs: i64) -> World {
    let world = world();
    world.source.set(vec![proc(
        pid,
        1,
        start_secs,
        "/work/shop/api",
        None,
        &["devports"],
    )]);
    world
}

// covers: U-10
#[test]
pub(super) fn a_child_of_the_launcher_is_bound() {
    let mut world = world_with_launcher(500, epoch_of_start());
    let outcome = world.run(&request("api", RunRole::Principal));
    let reference = accepted(&outcome).instance_ref.clone();
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    world.scan(vec![
        proc(500, 1, epoch_of_start(), "/work/shop/api", None, &["sh"]),
        // `npm run dev` forks the real server: a descendant, in another folder.
        proc(
            510,
            500,
            now,
            "/elsewhere",
            Some(8080),
            &["node", "server.js"],
        ),
    ]);
    let view = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert_eq!(view.instance_ref, reference);
    assert_eq!(view.pid, Some(510));
    assert_eq!(view.state, InstanceState::Healthy);
}

// covers: U-10
#[test]
pub(super) fn a_detached_server_inside_the_project_is_bound_by_directory() {
    let (mut world, reference) = principal_world();
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    // Parent is launchd (1): no ancestry, but it runs inside the project.
    world.scan(vec![proc(
        610,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    let view = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert_eq!(view.instance_ref, reference);
    assert_eq!(view.pid, Some(610));
}

// covers: I-05
#[test]
pub(super) fn a_stranger_on_the_reserved_port_is_not_bound_i_05() {
    let (mut world, reference) = principal_world();
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    world.scan(vec![proc(
        77,
        1,
        now,
        "/somewhere/else",
        Some(8080),
        &["node", "other.js"],
    )]);

    assert!(world.events().contains(&"port_stolen".to_owned()));
    assert!(
        world.engine.tracked.is_empty(),
        "the reservation was released"
    );
    assert!(
        !world
            .engine
            .views()
            .iter()
            .any(|view| view.instance_ref == reference),
        "nothing is bound to the stranger"
    );
    // The stranger is just an unregistered process now.
    assert!(
        world
            .engine
            .views()
            .iter()
            .any(|view| view.pid == Some(77) && !view.registered)
    );
}

// covers: I-05
#[test]
pub(super) fn a_process_that_started_before_the_reservation_is_not_bound() {
    let (mut world, _) = principal_world();
    let before = epoch(&world) - 600;
    world.scan(vec![proc(
        88,
        1,
        before,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    assert!(world.events().contains(&"port_stolen".to_owned()));
}

// covers: U-07
#[test]
pub(super) fn an_unused_reservation_is_released_after_the_grace_period() {
    let (mut world, _) = principal_world();
    world.clock.advance(Duration::from_secs(59));
    world.engine.tick();
    assert_eq!(world.engine.tracked.len(), 1, "still waiting");
    world.clock.advance(Duration::from_secs(2));
    world.engine.tick();
    assert!(world.engine.tracked.is_empty());
    assert!(world.events().contains(&"reservation_released".to_owned()));
    // The port is free for the next run.
    assert!(matches!(
        world.run(&request("api", RunRole::Principal)),
        RunOutcome::Accepted(_)
    ));
}

#[test]
pub(super) fn when_the_process_disappears_the_instance_ends_and_the_service_shows_as_down() {
    let (mut world, _) = principal_world();
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    world.scan(vec![proc(
        610,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    assert_eq!(
        world.instance(Some("shop"), "api", InstanceRole::Principal)[0].state,
        InstanceState::Healthy
    );

    world.scan(Vec::new());

    let view = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert_eq!(view.state, InstanceState::Down);
    assert_eq!(view.instance_ref, "-");
    assert!(world.events().contains(&"process_gone".to_owned()));
    // A new run is accepted: the old one is over.
    assert!(matches!(
        world.run(&request("api", RunRole::Principal)),
        RunOutcome::Accepted(_)
    ));
}

// ------------------------------------------------------- observed: u-05, i-02
