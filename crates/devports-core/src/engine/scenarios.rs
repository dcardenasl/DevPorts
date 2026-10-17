//! Scenario tests tied to `PRUEBAS.md`: attribution (I-16), stale instances
//! (I-17), recycled pids (PT-03) and the Docker rules with a simulated source
//! (D-01 to D-04). Each test names what it covers on a `// covers:` line, which
//! `tools/check_matrix.py` reads.

#![allow(clippy::arithmetic_side_effects)]

use std::path::Path;
use std::time::Duration;

use devports_proto::run::{RejectKind, RunOutcome, RunRole};
use devports_proto::views::{ChainLink, InstanceRole, InstanceState};
use devports_proto::{LogArgs, LsArgs};
use jiff::Timestamp;
use proptest::prelude::*;

use super::tests::{
    START, World, accepted, container, epoch, launcher, proc, rejected, request, world,
};
use super::*;
use crate::docker::ContainerState;

fn link(pid: u32, name: &str) -> ChainLink {
    ChainLink {
        pid,
        name: name.to_owned(),
        label: None,
    }
}

fn with_chain(chain: Vec<ChainLink>) -> devports_proto::run::RunRegister {
    let mut register = request("api", RunRole::Principal);
    register.launcher = devports_proto::run::LauncherInfo { pid: 500, chain };
    register
}

// covers: I-16, RF-19
#[test]
fn a_known_agent_is_named_and_an_unknown_ancestor_is_shown_as_it_is() {
    let mut world = world();
    world.run(&with_chain(vec![link(4000, "zsh"), link(3999, "claude")]));
    let refused = world.run(&with_chain(vec![
        link(4100, "bash"),
        link(4099, "mystery-tool"),
    ]));
    assert_eq!(rejected(&refused).error, RejectKind::Conflict);

    let summaries: Vec<String> = world
        .engine
        .log(&LogArgs::default())
        .entries
        .into_iter()
        .map(|entry| entry.summary)
        .collect();
    let registered = summaries
        .iter()
        .find(|summary| summary.contains("registrada como Principal"))
        .expect("the registration is logged");
    // The chain of the first run carries a recognised name, kept in the view.
    let principal = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert!(
        principal
            .launcher_chain
            .iter()
            .any(|entry| entry.name == "claude" && entry.label.as_deref() == Some("Claude Code")),
        "{registered} {:?}",
        principal.launcher_chain
    );
    // The unknown one appears raw in the refusal's log line, with no label.
    let rejection = summaries
        .iter()
        .find(|summary| summary.contains("intentó levantar otra instancia"))
        .expect("the refusal is logged");
    assert!(rejection.contains("mystery-tool"), "{rejection}");
}

// covers: I-16, RF-19
#[test]
fn attribution_never_changes_what_is_decided() {
    let chains = [
        Vec::new(),
        vec![link(1, "zsh")],
        vec![link(1, "zsh"), link(2, "claude"), link(3, "cursor")],
        vec![link(1, "zsh"), link(2, "totally-unknown")],
    ];
    let outcomes: Vec<(bool, Option<RejectKind>)> = chains
        .into_iter()
        .map(|chain| {
            let mut world = world();
            world.run(&request("api", RunRole::Principal));
            match world.run(&with_chain(chain)) {
                RunOutcome::Rejected(rejection) => (false, Some(rejection.error)),
                RunOutcome::Accepted(_) => (true, None),
                RunOutcome::Reused(_) => (false, None),
            }
        })
        .collect();
    assert!(
        outcomes
            .iter()
            .all(|outcome| *outcome == (false, Some(RejectKind::Conflict)))
    );
}

fn git_repo(dir: &Path, changed_at: SystemTimeSeconds) {
    std::fs::create_dir_all(dir.join(".git").join("logs")).unwrap();
    std::fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
    let log = dir.join(".git").join("logs").join("HEAD");
    std::fs::write(&log, "0 1 someone\n").unwrap();
    let modified = std::time::UNIX_EPOCH + Duration::from_secs(changed_at);
    std::fs::File::options()
        .write(true)
        .open(&log)
        .unwrap()
        .set_modified(modified)
        .unwrap();
}

type SystemTimeSeconds = u64;

// covers: I-17, RF-20
#[test]
fn a_server_that_started_before_the_last_commit_is_marked_as_old_code() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = std::fs::canonicalize(dir.path()).unwrap();
    let mut world = world();
    let started = epoch(&world) - 3_600;
    // The repository changed ten minutes after the server started.
    git_repo(&repo, u64::try_from(started + 600).unwrap());
    world.scan(vec![proc(
        1,
        1,
        started,
        &repo.display().to_string(),
        Some(7000),
        &["node", "server.js"],
    )]);
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    let view = all
        .instances
        .iter()
        .find(|view| view.pid == Some(1))
        .unwrap();
    let note = view.stale.as_deref().expect("marked as stale");
    assert!(note.contains("arrancó antes del último commit"), "{note}");
}

// covers: I-17, RF-20
#[test]
fn a_server_that_started_after_the_last_commit_is_not_marked() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = std::fs::canonicalize(dir.path()).unwrap();
    let mut world = world();
    let started = epoch(&world) - 600;
    git_repo(&repo, u64::try_from(started - 3_600).unwrap());
    world.scan(vec![proc(
        1,
        1,
        started,
        &repo.display().to_string(),
        Some(7000),
        &["node", "server.js"],
    )]);
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    assert!(
        all.instances
            .iter()
            .find(|view| view.pid == Some(1))
            .unwrap()
            .stale
            .is_none()
    );
}

// covers: I-04, RF-08
#[test]
fn a_server_that_opens_its_port_ten_seconds_after_the_reservation_is_still_bound() {
    let mut world = world();
    let temporal = accepted(&world.run(&request("api", RunRole::Test))).clone();
    world.clock.advance(Duration::from_secs(10));
    let now = epoch(&world);
    world.scan(vec![proc(
        900,
        1,
        now,
        "/work/shop/api",
        Some(temporal.port),
        &["php", "spark", "serve"],
    )]);
    let view = &world.instance(Some("shop"), "api", InstanceRole::Temporal)[0];
    assert_eq!(view.instance_ref, temporal.instance_ref);
    assert_eq!(view.pid, Some(900));
    assert_eq!(view.state, InstanceState::Healthy);
}

fn bound_container_temporal(world: &mut World) -> (String, String) {
    let mut docker_run = request("api", RunRole::Test);
    docker_run.command = Some(
        ["docker", "run", "-d", "-p", "{PORT}:3306", "mysql"]
            .map(str::to_owned)
            .to_vec(),
    );
    let temporal = accepted(&world.run(&docker_run)).clone();
    world.clock.advance(Duration::from_secs(2));
    let mut started = container("c9", "happy_mysql", "mysql:8", temporal.port, None);
    started.created = epoch(world);
    world.engine.observe_containers(Some(vec![started]));
    (temporal.instance_ref, "c9".to_owned())
}

// covers: D-02, RF-03, RF-09
#[test]
fn an_expired_container_temporal_is_stopped_never_removed() {
    let mut world = world();
    let (reference, id) = bound_container_temporal(&mut world);
    world.clock.advance(Duration::from_mins(31));
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
    assert_eq!(stops[0].target, Target::Container(id));
    // `Effect` has no way to ask for a removal: the only container action there
    // is is a stop, which keeps the container and its volumes.
    world
        .engine
        .termination_finished(&reference, StopReason::LeaseExpired, &Terminated::Stopped);
    assert!(
        world
            .instance(Some("shop"), "api", InstanceRole::Temporal)
            .is_empty()
    );
}

// covers: D-03, RF-03
#[test]
fn start_and_stop_of_a_container_are_reflected_at_once_and_unreachable_is_not_stopped() {
    let mut world = world();
    let mut redis = container("c1", "shop-redis", "redis:7", 6379, None);
    world.engine.observe_containers(Some(vec![redis.clone()]));
    let cache =
        |world: &World| world.instance(Some("shop"), "cache", InstanceRole::Principal)[0].state;
    assert_eq!(cache(&world), InstanceState::Healthy);

    redis.state = ContainerState::Exited;
    world.engine.observe_containers(Some(vec![redis.clone()]));
    assert_eq!(cache(&world), InstanceState::Down);

    redis.state = ContainerState::Running;
    world.engine.observe_containers(Some(vec![redis]));
    assert_eq!(cache(&world), InstanceState::Healthy);

    // Docker restarts: while it is away the picture is "unknown", never "gone".
    world.engine.observe_containers(None);
    assert_eq!(cache(&world), InstanceState::Unknown);
}

// covers: D-04, RF-09
#[test]
fn a_data_service_is_never_stopped_by_the_core() {
    let mut world = world();
    world.engine.observe_containers(Some(vec![
        container("c1", "shop-redis", "redis:7", 6379, None),
        container(
            "c2",
            "mysql",
            "mysql:8.0",
            3306,
            Some(("docker-services", "/x")),
        ),
        container("c3", "stray-postgres", "postgres:16", 5432, None),
    ]));
    for _ in 0..3 {
        world.clock.advance(Duration::from_hours(8));
        let effects = world.engine.tick();
        assert!(
            effects
                .iter()
                .all(|effect| !matches!(effect, Effect::Terminate(_))),
            "{effects:?}"
        );
    }
    assert!(
        world
            .engine
            .views()
            .iter()
            .any(|view| view.container.as_deref() == Some("mysql"))
    );
}

// covers: D-01, RF-02
#[test]
fn a_compose_container_is_assigned_to_its_project_by_labels() {
    let mut world = world();
    world.engine.observe_containers(Some(vec![container(
        "c4",
        "shop-redis",
        "redis:7",
        6379,
        Some(("shop", "/work/shop")),
    )]));
    let view = &world.instance(Some("shop"), "cache", InstanceRole::Principal)[0];
    assert_eq!(view.container.as_deref(), Some("shop-redis"));
}

proptest! {
    // covers: PT-03, RF-06
    #[test]
    fn a_recycled_pid_is_never_taken_for_the_registered_process(
        other_start in 1_i64..2_000_000_000,
        pid in 2_u32..100_000,
    ) {
        let mut world = world();
        let temporal = accepted(&world.run(&request("api", RunRole::Test))).clone();
        world.clock.advance(Duration::from_secs(1));
        let bound_at = epoch(&world);
        world.scan(vec![proc(pid, 1, bound_at, "/work/shop/api", Some(temporal.port), &["php"])]);
        prop_assert_eq!(
            world.instance(Some("shop"), "api", InstanceRole::Temporal)[0].pid,
            Some(pid)
        );

        // The process dies and the system hands the same pid to another program.
        prop_assume!(other_start != bound_at);
        world.scan(vec![proc(pid, 1, other_start, "/elsewhere", Some(temporal.port + 1), &["node"])]);
        let temporals = world.instance(Some("shop"), "api", InstanceRole::Temporal);
        prop_assert!(
            temporals.iter().all(|view| view.state != InstanceState::Healthy || view.pid != Some(pid)),
            "the new process was taken for the old instance: {temporals:?}"
        );
        // And nothing registered is ever aimed at the newcomer.
        let effects = world.engine.tick();
        let newcomer = Target::Process(ProcessId {
            pid,
            start: crate::process::ProcessStart(u64::try_from(other_start).unwrap() * 1_000_000),
        });
        let aimed_at_newcomer = effects.iter().any(|effect| {
            matches!(effect, Effect::Terminate(request) if request.target == newcomer)
        });
        prop_assert!(!aimed_at_newcomer);
    }
}

#[test]
fn the_launcher_helper_is_used_so_the_import_stays_honest() {
    assert_eq!(launcher(1).pid, 1);
    assert!(START.parse::<Timestamp>().is_ok());
}
