//! Which instance is the Principal, and what the people see.

use super::*;

// covers: U-05
#[test]
pub(super) fn a_server_started_by_hand_is_the_principal_de_hecho() {
    let mut world = world();
    let now = epoch(&world) - 100;
    world.scan(vec![proc(
        900,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);

    let principal = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert!(!principal.registered);
    assert_eq!(principal.state, InstanceState::Healthy);
    assert!(
        world
            .instance(Some("shop"), "api", InstanceRole::Unregistered)
            .is_empty()
    );

    // E7: an agent now runs `devports run api`: refused, with its URL.
    let outcome = world.run(&request("api", RunRole::Principal));
    let refusal = rejected(&outcome);
    assert_eq!(refusal.error, RejectKind::Conflict);
    assert!(refusal.message.contains("http://localhost:8080"));
}

// covers: I-02
#[test]
pub(super) fn seven_servers_for_one_service_give_one_principal_and_six_duplicates_i_02() {
    let mut world = world();
    let base = epoch(&world) - 1_000;
    let mut table = Vec::new();
    for (index, port) in [8082_u16, 8083, 8080, 8084, 8085, 8086, 8087]
        .into_iter()
        .enumerate()
    {
        let started = base + i64::try_from(index).unwrap() * 10;
        table.push(proc(
            1000 + u32::try_from(index).unwrap(),
            1,
            started,
            "/work/shop/api",
            Some(port),
            &["php", "spark", "serve", "--port", &port.to_string()],
        ));
    }
    let effects = world.scan(table);

    let principal = world.instance(Some("shop"), "api", InstanceRole::Principal);
    let duplicates = world.instance(Some("shop"), "api", InstanceRole::Unregistered);
    assert_eq!(principal.len(), 1);
    assert_eq!(duplicates.len(), 6);
    assert_eq!(
        principal[0].port,
        Some(8080),
        "the preferred port wins even if it is not the oldest"
    );
    assert!(
        duplicates
            .iter()
            .all(|view| view.duplicate_of.as_deref() == Some(principal[0].instance_ref.as_str()))
    );
    let notices = effects
        .iter()
        .filter(|effect| matches!(effect, Effect::Notify(Notice::DuplicateDetected { .. })))
        .count();
    assert_eq!(notices, 6);
    assert_eq!(
        world
            .events()
            .iter()
            .filter(|kind| *kind == "duplicate_detected")
            .count(),
        6
    );
}

// covers: U-05
#[test]
pub(super) fn without_the_preferred_port_the_oldest_is_the_principal() {
    let mut world = world();
    let base = epoch(&world) - 1_000;
    world.scan(vec![
        proc(
            1,
            1,
            base + 50,
            "/work/shop/api",
            Some(9001),
            &["php", "spark", "serve"],
        ),
        proc(
            2,
            1,
            base,
            "/work/shop/api",
            Some(9002),
            &["php", "spark", "serve"],
        ),
        proc(
            3,
            1,
            base + 90,
            "/work/shop/api",
            Some(9003),
            &["php", "spark", "serve"],
        ),
    ]);
    assert_eq!(
        world.instance(Some("shop"), "api", InstanceRole::Principal)[0].port,
        Some(9002)
    );
}

// covers: U-05
#[test]
pub(super) fn a_registered_principal_wins_over_observed_servers() {
    let (mut world, reference) = principal_world();
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    world.scan(vec![
        proc(
            610,
            1,
            now,
            "/work/shop/api",
            Some(8080),
            &["php", "spark", "serve"],
        ),
        proc(
            611,
            1,
            now + 1,
            "/work/shop/api",
            Some(8090),
            &["php", "spark", "serve"],
        ),
    ]);
    let principal = world.instance(Some("shop"), "api", InstanceRole::Principal);
    assert_eq!(principal.len(), 1);
    assert_eq!(principal[0].instance_ref, reference);
    let duplicates = world.instance(Some("shop"), "api", InstanceRole::Unregistered);
    assert_eq!(duplicates.len(), 1);
    assert_eq!(
        duplicates[0].duplicate_of.as_deref(),
        Some(reference.as_str())
    );
}

#[test]
pub(super) fn duplicates_are_announced_once_and_again_if_they_come_back() {
    let mut world = world();
    let base = epoch(&world) - 100;
    let table = vec![
        proc(
            1,
            1,
            base,
            "/work/shop/api",
            Some(8080),
            &["php", "spark", "serve"],
        ),
        proc(
            2,
            1,
            base + 5,
            "/work/shop/api",
            Some(8090),
            &["php", "spark", "serve"],
        ),
    ];
    let first = world.scan(table.clone());
    let second = world.scan(table.clone());
    assert_eq!(first.len(), 1);
    assert!(second.is_empty(), "no repeat while it stays");
    world.scan(vec![table[0].clone()]);
    let third = world.scan(table);
    assert_eq!(third.len(), 1, "a duplicate that comes back is news again");
}

#[test]
pub(super) fn processes_outside_any_project_are_listed_only_with_all() {
    let mut world = world();
    let now = epoch(&world) - 10;
    world.scan(vec![
        proc(
            1,
            1,
            now,
            "/work/shop/api",
            Some(8080),
            &["php", "spark", "serve"],
        ),
        proc(
            2,
            1,
            now,
            "/tmp/scratch",
            Some(7000),
            &["python3", "-m", "http.server", "7000"],
        ),
    ]);
    let default = world.engine.ls(&LsArgs {
        cwd: Some("/work/shop/api".to_owned()),
        ..LsArgs::default()
    });
    assert!(
        default
            .instances
            .iter()
            .all(|view| view.project.as_deref() == Some("shop"))
    );
    assert!(default.instances.iter().any(|view| view.port == Some(8080)));
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    assert!(
        all.instances
            .iter()
            .any(|view| view.pid == Some(2) && view.project.is_none())
    );
    let outside = world.engine.ls(&LsArgs::default());
    assert!(!outside.instances.iter().any(|view| view.pid == Some(2)));
}

#[test]
pub(super) fn declared_services_with_nothing_running_appear_as_down() {
    let mut world = world();
    // Docker has answered: an empty list means nothing runs.
    world.engine.observe_containers(Some(Vec::new()));
    let views = world.engine.views();
    for service in ["api", "admin", "cache"] {
        let view = views
            .iter()
            .find(|view| {
                view.service.as_deref() == Some(service) && view.project.as_deref() == Some("shop")
            })
            .unwrap();
        assert_eq!(view.state, InstanceState::Down, "{service}");
        assert_eq!(view.role, InstanceRole::Principal);
    }
    assert!(views.iter().any(|view| view.shared
        && view.service.as_deref() == Some("mysql")
        && view.state == InstanceState::Down));
}

#[test]
pub(super) fn container_services_are_unknown_until_docker_has_answered() {
    let world = world();
    let views = world.engine.views();
    let state_of = |service: &str| {
        views
            .iter()
            .find(|view| view.service.as_deref() == Some(service))
            .unwrap()
            .state
    };
    assert_eq!(state_of("cache"), InstanceState::Unknown);
    // Host services do not depend on Docker.
    assert_eq!(state_of("api"), InstanceState::Down);
}

// --------------------------------------------------------------------- docker

#[test]
pub(super) fn a_declared_container_is_the_principal_of_its_service() {
    let mut world = world();
    world.engine.observe_containers(Some(vec![container(
        "c1",
        "shop-redis",
        "redis:7",
        6379,
        None,
    )]));
    let view = &world.instance(Some("shop"), "cache", InstanceRole::Principal)[0];
    assert_eq!(view.container.as_deref(), Some("shop-redis"));
    assert_eq!(view.kind, "data");
    assert_eq!(view.state, InstanceState::Healthy);
}

#[test]
pub(super) fn a_container_named_in_shared_yml_is_the_shared_service() {
    let mut world = world();
    world.engine.observe_containers(Some(vec![container(
        "c2",
        "mysql",
        "mysql:8.0",
        3306,
        Some(("docker-services", "/Users/me/docker-services")),
    )]));
    let view = &world.instance(None, "mysql", InstanceRole::Principal)[0];
    assert!(view.shared);
    assert_eq!(view.state, InstanceState::Healthy);
    assert!(
        view.compose.is_none(),
        "it belongs to a shared service, not to a compose group"
    );
}

#[test]
pub(super) fn a_loose_container_is_listed_without_a_project_and_a_compose_one_by_group() {
    let mut world = world();
    world.engine.observe_containers(Some(vec![
        container("c3", "scratch", "alpine", 9999, None),
        container(
            "c4",
            "phpmyadmin",
            "phpmyadmin",
            8888,
            Some(("docker-services", "/Users/me/docker-services")),
        ),
    ]));
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    let scratch = all
        .instances
        .iter()
        .find(|view| view.container.as_deref() == Some("scratch"))
        .unwrap();
    assert!(scratch.project.is_none() && scratch.compose.is_none());
    let admin = all
        .instances
        .iter()
        .find(|view| view.container.as_deref() == Some("phpmyadmin"))
        .unwrap();
    assert_eq!(admin.compose.as_deref(), Some("docker-services"));
}

#[test]
pub(super) fn when_docker_is_unreachable_containers_are_unknown_not_gone() {
    let mut world = world();
    let accepted = accepted(&world.run(&request("cache", RunRole::Principal))).clone();
    world.clock.advance(Duration::from_secs(1));
    world.engine.observe_containers(Some(vec![container(
        "c1",
        "shop-redis",
        "redis:7",
        6379,
        None,
    )]));
    let after_bind = world.instance(Some("shop"), "cache", InstanceRole::Principal)[0].clone();
    assert_eq!(after_bind.state, InstanceState::Healthy);
    assert_eq!(after_bind.ports, vec![6379]);
    assert_eq!(after_bind.instance_ref, accepted.instance_ref);

    world.engine.observe_containers(None);
    let unknown = &world.instance(Some("shop"), "cache", InstanceRole::Principal)[0];
    assert_eq!(
        unknown.state,
        InstanceState::Unknown,
        "unreachable must not claim current health or stopped state"
    );
    assert!(unknown.ports.is_empty());
    assert!(!world.events().contains(&"process_gone".to_owned()));

    let conflict = world.run(&request("cache", RunRole::Principal));
    assert_eq!(rejected(&conflict).error, RejectKind::Conflict);
    assert_eq!(
        rejected(&conflict).existing.as_ref().unwrap().state,
        InstanceState::Unknown
    );
    let mut reuse = request("cache", RunRole::Principal);
    reuse.reuse = true;
    let RunOutcome::Reused(reused) = world.run(&reuse) else {
        panic!("known reference must be preserved without starting another container");
    };
    assert_eq!(reused.instance.instance_ref, accepted.instance_ref);
    assert_eq!(reused.instance.state, InstanceState::Unknown);
    assert!(reused.message.contains("estado desconocido"));
    assert!(!reused.message.contains("ya corre"));
    assert_eq!(world.engine.tracked.len(), 1);

    world.engine.observe_containers(Some(Vec::new()));
    assert_eq!(
        world.instance(Some("shop"), "cache", InstanceRole::Principal)[0].state,
        InstanceState::Down
    );
    assert!(world.events().contains(&"process_gone".to_owned()));
}

#[test]
pub(super) fn a_container_published_after_the_reservation_binds_a_temporal() {
    let mut world = world();
    let mut docker_run = request("api", RunRole::Test);
    docker_run.command = Some(vec![
        "docker".to_owned(),
        "run".to_owned(),
        "-d".to_owned(),
        "-p".to_owned(),
        "{PORT}:80".to_owned(),
        "nginx".to_owned(),
    ]);
    let temporal = accepted(&world.run(&docker_run)).clone();
    world.clock.advance(Duration::from_secs(2));
    let mut started = container("c9", "happy_nginx", "nginx", temporal.port, None);
    started.created = epoch(&world);
    world.engine.observe_containers(Some(vec![started]));

    let view = &world.instance(Some("shop"), "api", InstanceRole::Temporal)[0];
    assert_eq!(view.instance_ref, temporal.instance_ref);
    assert_eq!(view.container.as_deref(), Some("happy_nginx"));
    assert_eq!(view.state, InstanceState::Healthy);
}

// --------------------------------------------------------------------- leases
