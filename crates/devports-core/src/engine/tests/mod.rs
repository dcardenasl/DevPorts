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

#[allow(
    clippy::similar_names,
    reason = "pid and ppid are the operating system's own names"
)]
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
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.kind)
            .collect()
    }
}

// ---------------------------------------------------------------- run: accept

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

pub(super) fn bound_temporal() -> (World, String, String, u16) {
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

pub(super) fn epoch_of_start() -> i64 {
    START.parse::<Timestamp>().unwrap().as_second() - 5
}

pub(super) fn principal_world() -> (World, String) {
    let mut world = world();
    let reference = accepted(&world.run(&request("api", RunRole::Principal)))
        .instance_ref
        .clone();
    (world, reference)
}

mod actions;
mod binding;
mod doctor;
mod ignore;
mod leases;
mod log;
mod run;
mod stop_and_views;
mod views;
