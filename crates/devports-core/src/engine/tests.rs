//! The engine's policy, deterministically: fake clock, fake process table, fake
//! ports, in-memory store (`PRUEBAS.md`: U-04, U-05, U-06, U-10 and the in-engine
//! half of I-01, I-02, I-05 and I-10).

#![allow(clippy::arithmetic_side_effects)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use devports_proto::run::{LauncherInfo, RejectKind, RunOutcome, RunRegister, RunRole};
use devports_proto::views::{InstanceRole, InstanceState};
use devports_proto::{ErrorCode, LogArgs, LsArgs};
use jiff::Timestamp;

use super::*;
use crate::catalog::Project;
use crate::clock::ManualClock;
use crate::docker::{ComposeLabels, ContainerState, PublishedPort};
use crate::manifest;
use crate::process::fake::FakeProcessSource;
use crate::process::{ProcessInfo, ProcessStart};

pub(super) const START: &str = "2026-10-17T12:00:00Z";

/// A port probe where only the listed ports are busy.
#[derive(Default)]
pub(super) struct FakePorts(Mutex<BTreeSet<u16>>);

impl FakePorts {
    fn occupy(&self, port: u16) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(port);
    }
}

impl PortProbe for FakePorts {
    fn is_free(&self, port: u16) -> bool {
        !self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&port)
    }
}

pub(super) type TestEngine = Engine<Arc<ManualClock>, Arc<FakeProcessSource>, Arc<FakePorts>>;

pub(super) struct World {
    pub(super) engine: TestEngine,
    pub(super) clock: Arc<ManualClock>,
    pub(super) source: Arc<FakeProcessSource>,
    pub(super) ports: Arc<FakePorts>,
}

const SHOP: &str = "\
version: 1
project: shop
uses: [mysql]
services:
  api:
    cwd: ./api
    port: 8080
    start: php spark serve --port ${PORT}
    match: php spark serve
  admin:
    cwd: ./admin
    port: 8081
  cache:
    docker: shop-redis
    kind: data
    port: 6379
    start: docker start shop-redis
";

const SHARED: &str = "\
version: 1
services:
  mysql:
    docker: mysql
    kind: data
    port: 3306
    start: docker start mysql
";

pub(super) fn world_with(store: Store, clock: Arc<ManualClock>) -> World {
    let manifest = manifest::parse(SHOP, Path::new("m.yml")).unwrap();
    let catalog = Catalog::new(vec![Project::from_manifest(
        Path::new("/work/shop"),
        &manifest,
    )]);
    let (shared, _) = SharedServices::parse(SHARED, Path::new("s.yml")).unwrap();
    let source = Arc::new(FakeProcessSource::default());
    let ports = Arc::new(FakePorts::default());
    let mut counter = 0_u8;
    let entropy: Box<dyn FnMut() -> [u8; 16] + Send> = Box::new(move || {
        counter = counter.wrapping_add(1);
        let mut bytes = [counter; 16];
        bytes[8] = counter.wrapping_mul(31);
        bytes
    });
    let engine = Engine::new(Parts {
        clock: Arc::clone(&clock),
        source: Arc::clone(&source),
        ports: Arc::clone(&ports),
        config: Config::defaults(Path::new("/Users/me")),
        catalog,
        shared,
        store,
        entropy,
        own_pid: 99,
    })
    .unwrap();
    World {
        engine,
        clock,
        source,
        ports,
    }
}

pub(super) fn world() -> World {
    world_with(
        Store::open_in_memory().unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    )
}

pub(super) fn launcher(pid: u32) -> LauncherInfo {
    LauncherInfo {
        pid,
        chain: vec![
            ChainLink {
                pid: 4000,
                name: "zsh".to_owned(),
                label: None,
            },
            ChainLink {
                pid: 3999,
                name: "cursor".to_owned(),
                label: None,
            },
        ],
    }
}

pub(super) fn request(service: &str, role: RunRole) -> RunRegister {
    RunRegister {
        project: None,
        service: service.to_owned(),
        cwd: "/work/shop/api".to_owned(),
        role,
        reason: (role == RunRole::Test).then(|| "QA login".to_owned()),
        ttl_seconds: None,
        reuse: false,
        command: None,
        launcher: launcher(500),
    }
}

pub(super) fn accepted(outcome: &RunOutcome) -> &devports_proto::run::Accepted {
    match outcome {
        RunOutcome::Accepted(accepted) => accepted,
        other => panic!("expected an accepted run, got {other:?}"),
    }
}

pub(super) fn rejected(outcome: &RunOutcome) -> &devports_proto::run::Rejected {
    match outcome {
        RunOutcome::Rejected(rejected) => rejected,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

pub(super) fn proc(
    pid: u32,
    ppid: u32,
    start_secs: i64,
    cwd: &str,
    port: Option<u16>,
    argv: &[&str],
) -> ListeningProcess {
    ListeningProcess {
        info: ProcessInfo {
            id: ProcessId {
                pid,
                start: ProcessStart(u64::try_from(start_secs).unwrap() * 1_000_000),
            },
            ppid,
            pgid: pid,
            uid: 501,
            argv: argv.iter().map(|part| (*part).to_owned()).collect(),
            cwd: Some(PathBuf::from(cwd)),
        },
        ports: port.into_iter().collect(),
    }
}

pub(super) fn epoch(world: &World) -> i64 {
    world.clock.wall().as_second()
}

impl World {
    /// Publishes a process table as both the fake source and the engine's scan.
    pub(super) fn scan(&mut self, table: Vec<ListeningProcess>) -> Vec<Effect> {
        self.source.set(table.clone());
        let listening: Vec<ListeningProcess> =
            table.into_iter().filter(|p| !p.ports.is_empty()).collect();
        self.engine.observe_host(listening)
    }

    pub(super) fn run(&mut self, request: &RunRegister) -> RunOutcome {
        self.engine.run_register(request).0
    }

    pub(super) fn instance(
        &self,
        project: Option<&str>,
        service: &str,
        role: InstanceRole,
    ) -> Vec<InstanceView> {
        self.engine
            .views()
            .iter()
            .filter(|view| {
                view.project.as_deref() == project
                    && view.service.as_deref() == Some(service)
                    && view.role == role
            })
            .cloned()
            .collect()
    }

    pub(super) fn events(&self) -> Vec<String> {
        self.engine
            .log(&LogArgs {
                since_seconds: Some(86_400),
                project: None,
            })
            .entries
            .into_iter()
            .map(|entry| entry.kind)
            .collect()
    }
}

// ---------------------------------------------------------------- run: accept

#[test]
fn the_first_run_becomes_the_principal_on_its_preferred_port() {
    let mut world = world();
    let outcome = world.run(&request("api", RunRole::Principal));
    let accepted = accepted(&outcome);

    assert_eq!(accepted.port, 8080);
    assert_eq!(accepted.role, InstanceRole::Principal);
    assert_eq!(accepted.project, "shop");
    assert_eq!(accepted.url, "http://localhost:8080");
    assert_eq!(accepted.shell, "exec php spark serve --port 8080");
    assert!(accepted.token.is_none(), "a Principal has no stop token");
    assert!(accepted.run_dir.ends_with("/work/shop/api"));
    assert_eq!(accepted.env["PORT"], "8080");
    assert_eq!(accepted.env["DEVPORTS_PORT"], "8080");
    assert_eq!(accepted.env["DEVPORTS_ROLE"], "principal");
    assert_eq!(accepted.env["DEVPORTS_SERVICE"], "api");
    assert_eq!(accepted.env["DEVPORTS_INSTANCE_REF"], accepted.instance_ref);
    assert!(!accepted.env.contains_key("DEVPORTS_STOP_TOKEN"));
    assert!(accepted.banner[0].contains("como PRINCIPAL en http://localhost:8080"));

    let principal = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert_eq!(principal.state, InstanceState::Starting);
    assert!(principal.registered);
}

#[test]
fn the_project_is_resolved_from_the_folder_or_named_explicitly() {
    let mut world = world();
    let mut elsewhere = request("api", RunRole::Principal);
    elsewhere.cwd = "/tmp".to_owned();
    let refused = world.run(&elsewhere);
    assert_eq!(rejected(&refused).error, RejectKind::NotFound);
    assert!(rejected(&refused).message.contains("`--project`"));

    elsewhere.project = Some("shop".to_owned());
    assert_eq!(accepted(&world.run(&elsewhere)).project, "shop");

    let mut ghost = request("api", RunRole::Principal);
    ghost.project = Some("ghost".to_owned());
    assert_eq!(rejected(&world.run(&ghost)).error, RejectKind::NotFound);
}

// covers: U-04
#[test]
fn an_unknown_service_is_refused_with_code_14() {
    let mut world = world();
    let outcome = world.run(&request("nope", RunRole::Principal));
    assert_eq!(rejected(&outcome).error, RejectKind::NotFound);
    assert_eq!(rejected(&outcome).error.exit_code(), 14);
}

// covers: U-04
#[test]
fn without_a_command_the_run_is_refused_with_code_13() {
    let mut world = world();
    // `admin` has no `start`.
    let outcome = world.run(&request("admin", RunRole::Principal));
    assert_eq!(rejected(&outcome).error, RejectKind::NoCommand);
    // Giving a command after `--` fixes it.
    let mut with_command = request("admin", RunRole::Principal);
    with_command.command = Some(vec![
        "node".to_owned(),
        "server.js".to_owned(),
        "--port".to_owned(),
        "{PORT}".to_owned(),
    ]);
    assert_eq!(
        accepted(&world.run(&with_command)).shell,
        "exec node server.js --port 8081"
    );
}

#[test]
fn arguments_are_quoted_so_they_reach_the_command_unchanged() {
    let mut world = world();
    let mut tricky = request("admin", RunRole::Principal);
    tricky.command = Some(vec![
        "echo".to_owned(),
        "it's $HOME; rm -rf *".to_owned(),
        "{PORT}".to_owned(),
        String::new(),
    ]);
    assert_eq!(
        accepted(&world.run(&tricky)).shell,
        "exec echo 'it'\\''s $HOME; rm -rf *' 8081 ''"
    );
}

#[test]
fn a_shared_service_can_be_started_from_a_project_that_uses_it() {
    let mut world = world();
    let mut mysql = request("mysql", RunRole::Principal);
    mysql.command = None;
    let outcome = world.run(&mysql);
    let accepted = accepted(&outcome);
    assert_eq!(accepted.shell, "exec docker start mysql");
    assert_eq!(accepted.port, 3306);
    assert_eq!(accepted.project, "compartidos");
    let view = &world.instance(None, "mysql", InstanceRole::Principal)[0];
    assert!(view.shared);
}

// -------------------------------------------------------------- run: conflicts

// covers: U-04, I-01
#[test]
fn a_second_run_is_refused_while_the_first_is_starting_i_01() {
    let mut world = world();
    assert!(matches!(
        world.run(&request("api", RunRole::Principal)),
        RunOutcome::Accepted(_)
    ));
    // Seven agents racing: one Principal, six refusals, nothing else created.
    for _ in 0..6 {
        let outcome = world.run(&request("api", RunRole::Principal));
        assert_eq!(rejected(&outcome).error, RejectKind::Conflict);
        assert_eq!(rejected(&outcome).error.exit_code(), 10);
    }
    assert_eq!(
        world
            .instance(Some("shop"), "api", InstanceRole::Principal)
            .len(),
        1
    );
    assert_eq!(world.engine.tracked.len(), 1);
    let kinds = world.events();
    assert_eq!(
        kinds.iter().filter(|kind| *kind == "run_rejected").count(),
        6
    );
}

#[test]
fn the_refusal_names_what_exists_and_the_ways_out() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    let outcome = world.run(&request("api", RunRole::Principal));
    let refusal = rejected(&outcome);
    assert!(refusal.message.contains("conflicto en api (proyecto shop)"));
    assert!(refusal.message.contains("http://localhost:8080"));
    assert!(refusal.message.contains("devports run api --reuse"));
    assert!(refusal.message.contains("detén la instancia desde la app"));
    assert_eq!(
        refusal.existing.as_ref().unwrap().service.as_deref(),
        Some("api")
    );
    assert_eq!(refusal.alternatives.len(), 3);
    assert!(refusal.alternatives.iter().any(|alt| alt.needs_human));
}

// covers: U-04
#[test]
fn reuse_starts_nothing_and_reports_the_existing_instance() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    let mut again = request("api", RunRole::Principal);
    again.reuse = true;
    let outcome = world.run(&again);
    let RunOutcome::Reused(reused) = outcome else {
        panic!("expected a reuse");
    };
    assert!(
        reused
            .message
            .contains("api ya corre (Principal) en http://localhost:8080")
    );
    assert_eq!(world.engine.tracked.len(), 1);
    assert!(world.events().contains(&"reuse".to_owned()));
}

// covers: U-04
#[test]
fn a_hung_principal_is_refused_with_code_11_even_with_reuse() {
    let mut world = world();
    let accepted = accepted(&world.run(&request("api", RunRole::Principal))).clone();
    // The process is bound and then stops answering.
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    world.scan(vec![proc(
        500,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    for _ in 0..3 {
        world.engine.record_health(&accepted.instance_ref, false);
    }
    let mut again = request("api", RunRole::Principal);
    again.reuse = true;
    let outcome = world.run(&again);
    assert_eq!(rejected(&outcome).error, RejectKind::Unhealthy);
    assert_eq!(rejected(&outcome).error.exit_code(), 11);
    assert!(rejected(&outcome).message.contains("no responde"));

    // One good check clears it.
    world.engine.record_health(&accepted.instance_ref, true);
    assert!(matches!(world.run(&again), RunOutcome::Reused(_)));
}

// covers: U-04
#[test]
fn a_preferred_port_held_by_another_program_is_refused_with_code_18() {
    let mut world = world();
    world.ports.occupy(8080);
    world.scan(vec![proc(
        77,
        1,
        100,
        "/somewhere/else",
        Some(8080),
        &["/usr/bin/node", "server.js"],
    )]);
    let outcome = world.run(&request("api", RunRole::Principal));
    let refusal = rejected(&outcome);
    assert_eq!(refusal.error, RejectKind::PortBusy);
    assert_eq!(refusal.error.exit_code(), 18);
    assert!(
        refusal.message.contains("node (pid 77)"),
        "{}",
        refusal.message
    );
}

// ------------------------------------------------------------------- temporals

// covers: U-04
#[test]
fn a_test_run_needs_a_reason() {
    let mut world = world();
    let mut no_reason = request("api", RunRole::Test);
    no_reason.reason = None;
    assert_eq!(rejected(&world.run(&no_reason)).error, RejectKind::BadArgs);
    no_reason.reason = Some("   ".to_owned());
    assert_eq!(rejected(&world.run(&no_reason)).error.exit_code(), 16);
}

// covers: U-04
#[test]
fn a_temporal_gets_a_free_port_a_token_and_a_lease() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    let outcome = world.run(&request("api", RunRole::Test));
    let temporal = accepted(&outcome);

    assert_eq!(temporal.role, InstanceRole::Temporal);
    assert!((49152..=65535).contains(&temporal.port));
    assert_ne!(temporal.port, 8080);
    let token = temporal.token.as_deref().unwrap();
    assert_eq!(token.len(), 32);
    assert_eq!(temporal.env["DEVPORTS_STOP_TOKEN"], token);
    assert_eq!(temporal.env["DEVPORTS_ROLE"], "temporal");
    assert!(temporal.env.contains_key("DEVPORTS_EXPIRES_AT"));
    assert!(temporal.banner[1].starts_with("DEVPORTS_STOP_TOKEN="));
    assert!(temporal.banner[0].contains("como TEMPORAL"));
    assert!(temporal.banner[0].contains("(30 min)"));

    // It lives next to the Principal, which is not a duplicate of it.
    assert_eq!(
        world
            .instance(Some("shop"), "api", InstanceRole::Temporal)
            .len(),
        1
    );
    assert_eq!(
        world
            .instance(Some("shop"), "api", InstanceRole::Principal)
            .len(),
        1
    );
}

// covers: U-08, I-08
#[test]
fn the_token_is_never_stored_or_shown() {
    let mut world = world();
    let outcome = world.run(&request("api", RunRole::Test));
    let token = accepted(&outcome).token.clone().unwrap();
    let stored = world.engine.store.live_instances().unwrap();
    let json = serde_json::to_string(&stored.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>())
        .unwrap();
    assert!(!json.contains(&token), "the plain token is in the store");
    assert_eq!(
        stored[0].token_hash.as_deref(),
        Some(crate::ids::token_hash(&token).as_str())
    );
    let listed = serde_json::to_string(&world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    }))
    .unwrap();
    assert!(!listed.contains(&token));
    let log = serde_json::to_string(&world.engine.log(&LogArgs::default())).unwrap();
    assert!(!log.contains(&token));
}

// covers: U-04
#[test]
fn at_most_two_temporals_per_service() {
    let mut world = world();
    assert!(matches!(
        world.run(&request("api", RunRole::Test)),
        RunOutcome::Accepted(_)
    ));
    assert!(matches!(
        world.run(&request("api", RunRole::Test)),
        RunOutcome::Accepted(_)
    ));
    let third = world.run(&request("api", RunRole::Test));
    assert_eq!(rejected(&third).error, RejectKind::Limit);
    assert_eq!(rejected(&third).error.exit_code(), 12);
    // Another service has its own allowance.
    let mut other = request("admin", RunRole::Test);
    other.command = Some(vec!["true".to_owned()]);
    assert!(matches!(world.run(&other), RunOutcome::Accepted(_)));
}

// covers: U-04
#[test]
fn a_ttl_above_the_maximum_is_refused() {
    let mut world = world();
    let mut long = request("api", RunRole::Test);
    long.ttl_seconds = Some(5 * 3600);
    assert_eq!(rejected(&world.run(&long)).error, RejectKind::BadArgs);
    long.ttl_seconds = Some(0);
    assert_eq!(rejected(&world.run(&long)).error, RejectKind::BadArgs);
    long.ttl_seconds = Some(600);
    assert!(matches!(world.run(&long), RunOutcome::Accepted(_)));
}

// covers: U-07
#[test]
fn temporals_never_get_the_same_port() {
    let mut world = world();
    let first = accepted(&world.run(&request("api", RunRole::Test))).port;
    let second = accepted(&world.run(&request("api", RunRole::Test))).port;
    assert_ne!(first, second);
}

// ------------------------------------------------------------------- binding

fn principal_world() -> (World, String) {
    let mut world = world();
    let reference = accepted(&world.run(&request("api", RunRole::Principal)))
        .instance_ref
        .clone();
    (world, reference)
}

// covers: U-10
#[test]
fn the_launcher_itself_is_bound_after_exec() {
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

fn world_with_launcher(pid: u32, start_secs: i64) -> World {
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
fn a_child_of_the_launcher_is_bound() {
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

fn epoch_of_start() -> i64 {
    START.parse::<Timestamp>().unwrap().as_second() - 5
}

// covers: U-10
#[test]
fn a_detached_server_inside_the_project_is_bound_by_directory() {
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
fn a_stranger_on_the_reserved_port_is_not_bound_i_05() {
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
fn a_process_that_started_before_the_reservation_is_not_bound() {
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
fn an_unused_reservation_is_released_after_the_grace_period() {
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
fn when_the_process_disappears_the_instance_ends_and_the_service_shows_as_down() {
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

// covers: U-05
#[test]
fn a_server_started_by_hand_is_the_principal_de_hecho() {
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
fn seven_servers_for_one_service_give_one_principal_and_six_duplicates_i_02() {
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
fn without_the_preferred_port_the_oldest_is_the_principal() {
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
fn a_registered_principal_wins_over_observed_servers() {
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
fn duplicates_are_announced_once_and_again_if_they_come_back() {
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
fn processes_outside_any_project_are_listed_only_with_all() {
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
fn declared_services_with_nothing_running_appear_as_down() {
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
fn container_services_are_unknown_until_docker_has_answered() {
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

pub(super) fn container(
    id: &str,
    name: &str,
    image: &str,
    port: u16,
    compose: Option<(&str, &str)>,
) -> Container {
    Container {
        id: id.to_owned(),
        name: name.to_owned(),
        image: image.to_owned(),
        state: ContainerState::Running,
        ports: vec![PublishedPort {
            host_port: port,
            container_port: port,
        }],
        compose: compose.map(|(project, dir)| ComposeLabels {
            project: project.to_owned(),
            working_dir: Some(PathBuf::from(dir)),
        }),
        created: epoch_of_start() - 1_000,
    }
}

#[test]
fn a_declared_container_is_the_principal_of_its_service() {
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
fn a_container_named_in_shared_yml_is_the_shared_service() {
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
fn a_loose_container_is_listed_without_a_project_and_a_compose_one_by_group() {
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
fn when_docker_is_unreachable_containers_are_unknown_not_gone() {
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
fn a_container_published_after_the_reservation_binds_a_temporal() {
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

fn bound_temporal() -> (World, String, String, u16) {
    let mut world = world();
    let outcome = world.run(&request("api", RunRole::Test));
    let temporal = accepted(&outcome).clone();
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    world.scan(vec![proc(
        700,
        1,
        now,
        "/work/shop/api",
        Some(temporal.port),
        &["php", "spark", "serve"],
    )]);
    (
        world,
        temporal.instance_ref,
        temporal.token.unwrap(),
        temporal.port,
    )
}

// covers: U-06
#[test]
fn the_warning_comes_once_a_minute_before_the_end_u_06() {
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
fn at_the_end_of_the_lease_the_instance_is_stopped_exactly_once() {
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
fn what_survives_is_forgotten_and_never_retried() {
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
fn extending_adds_time_up_to_the_maximum_and_rearms_the_warning() {
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
fn a_token_that_matches_nothing_is_refused() {
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
fn a_token_holder_stops_only_its_own_temporal() {
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
fn stopping_twice_or_after_the_end_is_not_an_error() {
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
fn a_temporal_that_never_bound_is_released_on_stop_without_effects() {
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
fn a_lease_that_expired_while_the_core_was_down_is_stopped_at_once_i_10() {
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

#[test]
fn a_person_can_stop_a_principal_and_a_process_nobody_registered() {
    let mut world = world();
    let now = epoch(&world) - 100;
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
            now + 5,
            "/work/shop/api",
            Some(8090),
            &["php", "spark", "serve"],
        ),
    ]);
    let duplicate = world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0].clone();
    let effects = world.engine.stop_instance(&duplicate.instance_ref).unwrap();
    assert!(
        matches!(&effects[0], Effect::Terminate(request) if request.reason == StopReason::PersonRequest)
    );
    assert_eq!(
        world.engine.stop_instance("ffff").unwrap_err(),
        ActionError::UnknownInstance("ffff".to_owned())
    );
    world.engine.termination_finished(
        &duplicate.instance_ref,
        StopReason::PersonRequest,
        &Terminated::Stopped,
    );
    assert!(world.events().contains(&"stopped".to_owned()));
}

// covers: UXR-2.1
#[test]
fn a_manual_stop_rejects_a_changed_or_disappeared_confirmation_target() {
    let mut changed_world = world();
    let now = epoch(&changed_world) - 100;
    changed_world.scan(vec![proc(
        77,
        1,
        now,
        "/work/loose/server",
        Some(8080),
        &["node", "server.js"],
    )]);
    let original = changed_world
        .engine
        .views()
        .iter()
        .find(|view| view.pid == Some(77))
        .cloned()
        .expect("observed target");
    let expectation = StopExpectation::from_view(&original);
    let mut changed = expectation.clone();
    changed.port = Some(9090);
    assert_eq!(
        changed_world
            .engine
            .stop_confirmed(&original.instance_ref, &changed)
            .unwrap_err(),
        ActionError::TargetChanged
    );
    let mut reused_identity = expectation.clone();
    reused_identity.target_identity = Some("new-process-generation".to_owned());
    assert_eq!(
        changed_world
            .engine
            .stop_confirmed(&original.instance_ref, &reused_identity)
            .unwrap_err(),
        ActionError::TargetChanged
    );
    assert!(
        changed_world
            .engine
            .stop_confirmed(&original.instance_ref, &expectation)
            .is_ok()
    );

    let mut gone_world = world();
    gone_world.scan(vec![proc(
        78,
        1,
        now,
        "/work/loose/vanished",
        Some(8081),
        &["node", "server.js"],
    )]);
    let vanished = gone_world
        .engine
        .views()
        .iter()
        .find(|view| view.pid == Some(78))
        .cloned()
        .expect("observed target");
    let expectation = StopExpectation::from_view(&vanished);
    let mut replacement = proc(
        78,
        1,
        now,
        "/work/loose/vanished",
        Some(8081),
        &["node", "server.js"],
    );
    replacement.info.id.start =
        crate::process::ProcessStart(u64::try_from(now).unwrap() * 1_000_000 + 1);
    gone_world.scan(vec![replacement]);
    assert_eq!(
        gone_world
            .engine
            .stop_confirmed(&vanished.instance_ref, &expectation)
            .unwrap_err(),
        ActionError::UnknownInstance(vanished.instance_ref)
    );
}

#[test]
fn a_person_can_extend_a_running_temporal_from_the_application() {
    let (mut world, reference, _, _) = bound_temporal();
    world.engine.extend_instance(&reference).unwrap();
    let view = &world.instance(Some("shop"), "api", InstanceRole::Temporal)[0];
    assert!(view.expires_at.is_some());
    assert!(world.engine.extend_instance("nope").is_err());
}

#[test]
fn the_log_tells_the_story_in_spanish() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    world.run(&request("api", RunRole::Principal));
    let log = world.engine.log(&LogArgs::default());
    let summaries: Vec<&str> = log
        .entries
        .iter()
        .map(|entry| entry.summary.as_str())
        .collect();
    assert!(
        summaries
            .iter()
            .any(|summary| summary.contains("DevPorts se inició"))
    );
    assert!(
        summaries
            .iter()
            .any(|summary| summary.contains("registrada como Principal :8080"))
    );
    assert!(
        summaries.iter().any(|summary| summary
            .contains("un agente (cursor > zsh) intentó levantar otra instancia de api")),
        "{summaries:?}"
    );
    let filtered = world.engine.log(&LogArgs {
        project: Some("other".to_owned()),
        since_seconds: None,
    });
    assert!(filtered.entries.is_empty());
}

#[test]
fn the_socket_dispatcher_answers_each_operation() {
    use devports_proto::{ExtendArgs, LsResult, Op, PingResult, Request, TokenArgs};
    let (mut world, _, token, _) = bound_temporal();
    let ping = world.engine.handle_request(Request::new("1", Op::Ping)).0;
    assert_eq!(ping.into_result::<PingResult>().unwrap().pid, 99);
    let ls = world
        .engine
        .handle_request(Request::new(
            "2",
            Op::Ls(LsArgs {
                all: true,
                ..LsArgs::default()
            }),
        ))
        .0;
    assert!(!ls.into_result::<LsResult>().unwrap().instances.is_empty());
    let extended = world
        .engine
        .handle_request(Request::new(
            "3",
            Op::Extend(ExtendArgs {
                token: token.clone(),
                by_seconds: Some(60),
            }),
        ))
        .0;
    assert!(extended.ok);
    let bad = world
        .engine
        .handle_request(Request::new(
            "4",
            Op::Stop(TokenArgs {
                token: "x".to_owned(),
            }),
        ))
        .0;
    assert_eq!(bad.error.unwrap().code, ErrorCode::UnknownToken);
    let adopt = world
        .engine
        .handle_request(Request::new(
            "5",
            Op::AdoptPropose(devports_proto::AdoptArgs {
                dir: "/definitely/not/here".to_owned(),
            }),
        ))
        .0;
    assert_eq!(adopt.error.unwrap().code, ErrorCode::NotFound);
}

// ------------------------------------------------------------------- keeping

#[test]
fn a_kept_duplicate_stops_asking_for_attention_and_the_choice_survives_a_restart() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    let table = |base: i64| {
        vec![
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
                &["php", "spark", "serve", "--port", "8090"],
            ),
        ]
    };
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    let duplicate_ref = {
        let mut world = world_with(
            Store::open(&path).unwrap(),
            Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
        );
        world.scan(table(base));
        let duplicate = world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0].clone();
        assert!(!duplicate.ignored);
        world
            .engine
            .ignore_instance(&duplicate.instance_ref)
            .unwrap();
        let kept = &world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0];
        assert!(kept.ignored);
        assert!(world.events().contains(&"kept".to_owned()));
        duplicate.instance_ref
    };

    // After a restart the same command in the same folder is still kept, and it
    // is not announced again.
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    let effects = world.scan(table(base));
    let kept = &world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0];
    assert!(kept.ignored, "{duplicate_ref}");
    assert!(
        effects.is_empty(),
        "a kept duplicate is not news: {effects:?}"
    );
    assert!(world.engine.ignore_instance("nope").is_err());
}

#[test]
fn programs_that_ship_with_the_system_or_an_app_are_not_listed() {
    let mut world = world();
    let now = epoch(&world) - 10;
    world.scan(vec![
        proc(
            1,
            1,
            now,
            "/",
            Some(5000),
            &["/System/Library/CoreServices/ControlCenter.app/Contents/MacOS/ControlCenter"],
        ),
        proc(
            2,
            1,
            now,
            "/",
            Some(5001),
            &["/Applications/GitKraken.app/Contents/MacOS/GitKraken"],
        ),
        proc(3, 1, now, "/", Some(5002), &["/usr/libexec/rapportd"]),
        proc(
            4,
            1,
            now,
            "/tmp/scratch",
            Some(7000),
            &["/opt/homebrew/bin/node", "server.js"],
        ),
    ]);
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    let pids: Vec<Option<u32>> = all
        .instances
        .iter()
        .filter_map(|view| view.pid.map(Some))
        .collect();
    assert_eq!(pids, vec![Some(4)]);
}

#[test]
fn workers_that_share_their_parents_socket_are_one_server() {
    let mut world = world();
    let now = epoch(&world) - 10;
    world.scan(vec![
        proc(
            100,
            1,
            now,
            "/opt/homebrew/var",
            Some(9000),
            &["php-fpm: master process"],
        ),
        proc(
            101,
            100,
            now,
            "/opt/homebrew/var",
            Some(9000),
            &["php-fpm: pool www"],
        ),
        proc(
            102,
            100,
            now,
            "/opt/homebrew/var",
            Some(9000),
            &["php-fpm: pool www"],
        ),
        proc(200, 1, now, "/tmp/a", Some(7000), &["node", "a.js"]),
        proc(201, 1, now, "/tmp/b", Some(7000), &["node", "b.js"]),
    ]);
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    let mut pids: Vec<u32> = all.instances.iter().filter_map(|view| view.pid).collect();
    pids.sort_unstable();
    // Unrelated processes on the same port (two dev servers) stay separate.
    assert_eq!(pids, vec![100, 200, 201]);
}

// covers: UXR-2.2
#[test]
fn project_stop_excludes_data_shared_and_non_running_instances_in_the_core() {
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
fn project_stop_uses_only_confirmed_targets_and_revalidates_each_member() {
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
fn instance_views_separate_all_observed_ports_from_a_declared_port() {
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
fn a_registered_temporal_reports_observed_ports_instead_of_just_its_reservation() {
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
fn docker_observation_loss_does_not_claim_a_forgotten_container_is_still_alive() {
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
fn shared_summaries_expose_only_what_shared_yml_declares() {
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

fn loose_views(world: &World) -> Vec<InstanceView> {
    world
        .engine
        .views()
        .iter()
        .filter(|view| view.cwd.as_deref() == Some("/tmp/scratch"))
        .cloned()
        .collect()
}

// covers: UXR-5.1
#[test]
fn an_ignored_server_stays_ignored_when_recreated_and_comes_back_when_restored() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    let table = |pid: u32, start: i64, port: u16| {
        vec![proc(
            pid,
            1,
            start,
            "/tmp/scratch",
            Some(port),
            &["node", "server.js"],
        )]
    };
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    world.scan(table(40, base, 5000));
    let loose = loose_views(&world);
    assert_eq!(loose.len(), 1);
    assert!(!loose[0].ignored);
    let preview = world.engine.ignore_preview(&loose[0].instance_ref).unwrap();
    assert_eq!(preview.folder.as_deref(), Some("/tmp/scratch"));
    assert_eq!(preview.command, ["node", "server.js"]);
    assert!(
        world.engine.ignored_rules().unwrap().is_empty(),
        "a preview saves nothing"
    );
    world
        .engine
        .ignore_instance(&loose[0].instance_ref)
        .unwrap();

    // The same command in the same folder, as a new process on another port.
    world.scan(table(41, base + 10, 5001));
    assert!(loose_views(&world)[0].ignored);

    // After a restart the rule is still there, listed with its criterion even with
    // nothing running that matches it.
    drop(world);
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    let rules = world.engine.ignored_rules().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].folder.as_deref(), Some("/tmp/scratch"));
    assert!(!rules[0].legacy && rules[0].created_at.is_some());
    assert!(loose_views(&world).is_empty());
    world.scan(table(42, base + 20, 5002));
    assert!(loose_views(&world)[0].ignored);

    // A different folder or command is not hidden by that rule.
    world.scan(vec![
        proc(
            42,
            1,
            base + 20,
            "/tmp/scratch",
            Some(5002),
            &["node", "server.js"],
        ),
        proc(
            43,
            1,
            base + 30,
            "/tmp/scratch",
            Some(5003),
            &["node", "other.js"],
        ),
        proc(
            44,
            1,
            base + 40,
            "/tmp/elsewhere",
            Some(5004),
            &["node", "server.js"],
        ),
    ]);
    let ignored: Vec<_> = world
        .engine
        .views()
        .iter()
        .filter(|view| view.ignored)
        .map(|view| view.pid)
        .collect();
    assert_eq!(ignored, [Some(42)]);

    // Restoring needs no SQLite edit, is idempotent and is recorded.
    let id = rules[0].id.clone();
    world.engine.restore_ignored(&id).unwrap();
    assert!(world.engine.views().iter().all(|view| !view.ignored));
    assert!(world.engine.ignored_rules().unwrap().is_empty());
    assert!(world.events().contains(&"restored".to_owned()));
    world.engine.restore_ignored(&id).unwrap();
    assert!(world.engine.restore_ignored("nothing-like-it").is_ok());
}

// covers: UXR-5.1
#[test]
fn rules_from_earlier_builds_still_hide_only_what_they_did_and_can_be_restored() {
    let store = Store::open_in_memory().unwrap();
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    store
        .add_ignored("process:/tmp/scratch\u{1f}node\u{1f}node server.js", 7)
        .unwrap();
    let mut world = world_with(
        store,
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    world.scan(vec![
        proc(
            50,
            1,
            base,
            "/tmp/scratch",
            Some(5100),
            &["node", "server.js"],
        ),
        proc(
            51,
            1,
            base,
            "/tmp/scratch",
            Some(5101),
            &["node", "server.js", "--x"],
        ),
    ]);
    let ignored: Vec<_> = world
        .engine
        .views()
        .iter()
        .filter(|view| view.ignored)
        .map(|view| view.pid)
        .collect();
    assert_eq!(ignored, [Some(50)]);
    let rules = world.engine.ignored_rules().unwrap();
    assert!(rules[0].legacy);
    assert_eq!(rules[0].created_at.as_deref(), Some("1970-01-01T00:00:07Z"));
    world.engine.restore_ignored(&rules[0].id).unwrap();
    assert!(world.engine.views().iter().all(|view| !view.ignored));
}

// covers: UXR-5.1
#[test]
fn a_container_rule_belongs_to_its_docker_context() {
    let mut world = world();
    world.engine.set_docker_context("colima");
    world.engine.observe_containers(Some(vec![container(
        "c9",
        "scratch-db",
        "redis:7",
        6390,
        None,
    )]));
    let view = world
        .engine
        .views()
        .iter()
        .find(|view| view.container.as_deref() == Some("scratch-db"))
        .cloned()
        .unwrap();
    let preview = world.engine.ignore_preview(&view.instance_ref).unwrap();
    assert_eq!(preview.context.as_deref(), Some("colima"));
    world.engine.ignore_instance(&view.instance_ref).unwrap();
    let hidden = |world: &World| {
        world
            .engine
            .views()
            .iter()
            .find(|view| view.container.as_deref() == Some("scratch-db"))
            .map(|view| view.ignored)
    };
    assert_eq!(hidden(&world), Some(true));
    world.engine.set_docker_context("desktop-linux");
    assert_eq!(hidden(&world), Some(false), "same name on another context");
    world.engine.set_docker_context("colima");
    assert_eq!(hidden(&world), Some(true));
}
