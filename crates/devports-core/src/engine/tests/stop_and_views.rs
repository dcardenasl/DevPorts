//! Project stops and the ports shown in instance views.

use super::*;

// covers: UXR-2.2
#[test]
pub(super) fn project_stop_excludes_data_shared_and_non_running_instances_in_the_core() {
    let (mut world, temporal, _, _) = bound_temporal();
    world.engine.observe_containers(Some(vec![
        container("cache-id", "shop-redis", "redis:7", 6379, None),
        container("mysql-id", "mysql", "mysql:8", 3306, None),
    ]));
    let preview = world.engine.preview_project_stop("shop");
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].instance_ref, temporal);
    assert_eq!(preview[0].role, InstanceRole::Temporal);
    assert!(world.engine.preview_project_stop("absent").is_empty());
    let forbidden: Vec<_> = world
        .engine
        .views()
        .iter()
        .filter(|view| view.kind == "data" || view.shared)
        .cloned()
        .collect();
    for view in forbidden {
        assert!(
            world
                .engine
                .stop_project_confirmed(&view.instance_ref, &StopExpectation::from_view(&view))
                .is_err()
        );
    }
    assert!(
        world
            .engine
            .stop_project_confirmed(&temporal, &StopExpectation::from_view(&preview[0]))
            .is_ok()
    );
}

// covers: UXR-2.2
#[test]
pub(super) fn project_stop_uses_only_confirmed_targets_and_revalidates_each_member() {
    let mut world = world();
    let now = epoch(&world) - 100;
    let first = proc(
        77,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    );
    world.scan(vec![first.clone()]);
    let preview = world.engine.preview_project_stop("shop");
    assert_eq!(preview.len(), 1);
    let view = &preview[0];
    let expected = StopExpectation::from_view(view);
    // A later arrival is deliberately outside the confirmed set.
    world.scan(vec![
        first,
        proc(
            78,
            1,
            now,
            "/work/shop/admin",
            Some(8081),
            &["node", "server.js"],
        ),
    ]);
    assert_eq!(world.engine.preview_project_stop("shop").len(), 2);
    for field in ["project", "identity", "kind", "shared"] {
        let mut changed = expected.clone();
        match field {
            "project" => changed.project = Some("another-project".to_owned()),
            "identity" => changed.target_identity = Some("replacement".to_owned()),
            "kind" => changed.kind = "data".to_owned(),
            _ => changed.shared = true,
        }
        assert!(
            world
                .engine
                .stop_project_confirmed(&view.instance_ref, &changed)
                .is_err()
        );
    }
    let effects = world
        .engine
        .stop_project_confirmed(&view.instance_ref, &expected)
        .unwrap();
    assert_eq!(effects.len(), 1);
    assert!(
        matches!(&effects[0], Effect::Terminate(request) if request.reference == view.instance_ref)
    );
    // Removing membership after preview also invalidates a previously correct expectation.
    world
        .engine
        .views
        .iter_mut()
        .find(|current| current.pid == Some(78))
        .unwrap()
        .project = None;
    let later = world
        .engine
        .views()
        .iter()
        .find(|current| current.pid == Some(78))
        .unwrap()
        .clone();
    let mut old_membership = StopExpectation::from_view(&later);
    old_membership.project = Some("shop".to_owned());
    assert!(
        world
            .engine
            .stop_project_confirmed(&later.instance_ref, &old_membership)
            .is_err()
    );
}

// covers: UXR-2.3
#[test]
pub(super) fn instance_views_separate_all_observed_ports_from_a_declared_port() {
    let mut world = world();
    let now = epoch(&world) - 100;
    let mut process = proc(
        77,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    );
    process.ports = vec![9090, 8080, 9090];
    world.scan(vec![process]);
    let view = world
        .engine
        .views()
        .iter()
        .find(|view| view.pid == Some(77))
        .unwrap();
    assert_eq!(view.ports, vec![8080, 9090]);
    world.scan(Vec::new());
    let down = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert_eq!(down.state, InstanceState::Down);
    assert_eq!(down.port, Some(8080));
    assert!(
        down.ports.is_empty(),
        "a preferred port is not evidence of a listener"
    );
    let mut mysql = container("mysql-ports", "mysql", "mysql:8", 3306, None);
    mysql.ports.push(PublishedPort {
        host_port: 33060,
        container_port: 33060,
    });
    world.engine.observe_containers(Some(vec![mysql]));
    assert_eq!(
        world.instance(None, "mysql", InstanceRole::Principal)[0].ports,
        vec![3306, 33060]
    );
    world.engine.observe_containers(None);
    let unknown = &world.instance(None, "mysql", InstanceRole::Principal)[0];
    assert_eq!(unknown.state, InstanceState::Unknown);
    assert!(unknown.ports.is_empty());
}

// covers: UXR-2.3
#[test]
pub(super) fn a_registered_temporal_reports_observed_ports_instead_of_just_its_reservation() {
    let (mut world, reference, _, port) = bound_temporal();
    let mut process = proc(
        700,
        1,
        epoch(&world),
        "/work/shop/api",
        Some(port),
        &["php", "spark", "serve"],
    );
    process.ports.push(65534);
    world.scan(vec![process]);
    let view = world
        .engine
        .views()
        .iter()
        .find(|view| view.instance_ref == reference)
        .unwrap();
    let expected: Vec<u16> = [port, 65534]
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(view.ports, expected);
}

// covers: UXR-2.3
#[test]
pub(super) fn docker_observation_loss_does_not_claim_a_forgotten_container_is_still_alive() {
    let mut world = world();
    let registered = accepted(&world.run(&request("cache", RunRole::Principal))).clone();
    let mysql = container("cache-observation", "shop-redis", "redis:7", 6379, None);
    world.engine.observe_containers(Some(vec![mysql.clone()]));
    world.engine.termination_finished(
        &registered.instance_ref,
        StopReason::PersonRequest,
        &Terminated::StillAlive,
    );
    assert_eq!(
        world.instance(Some("shop"), "cache", InstanceRole::Principal)[0].state,
        InstanceState::Forgotten
    );
    world.engine.observe_containers(None);
    let view = &world.instance(Some("shop"), "cache", InstanceRole::Principal)[0];
    assert_eq!(view.state, InstanceState::Unknown);
    assert!(view.ports.is_empty());
    world.engine.observe_containers(Some(vec![mysql]));
    assert_eq!(
        world.instance(Some("shop"), "cache", InstanceRole::Principal)[0].state,
        InstanceState::Forgotten
    );
}

// covers: UXR-4.2
#[test]
pub(super) fn shared_summaries_expose_only_what_shared_yml_declares() {
    let mut world = world();
    let text = "\
version: 1
services:
  mysql:
    docker: mysql
    kind: data
    port: 3306
    start: docker start mysql
  admin:
    docker: admin
    port: 8025
    health: http:/ready
  bare:
    docker: bare
    port: 9000
  nohost:
    docker: nohost
    health: http:/ready
";
    let (shared, _) = SharedServices::parse(text, Path::new("s.yml")).unwrap();
    world.engine.set_catalog(Catalog::new(Vec::new()), shared);
    let summaries = world.engine.shared_summaries();
    let find = |name: &str| summaries.iter().find(|item| item.name == name).unwrap();
    assert_eq!(find("mysql").start.as_deref(), Some("docker start mysql"));
    assert_eq!(
        find("mysql").endpoint,
        None,
        "a port alone is not an HTTP endpoint"
    );
    assert_eq!(
        find("admin").endpoint.as_deref(),
        Some("http://localhost:8025/ready")
    );
    assert_eq!(find("admin").start, None);
    assert_eq!(find("bare").endpoint, None);
    assert_eq!(find("nohost").endpoint, None, "no port, no address");
}
