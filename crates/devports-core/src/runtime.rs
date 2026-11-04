//! The running core: the engine plus everything that feeds it and carries out
//! what it decides.
//!
//! One actor task owns the [`Engine`]; every other task talks to it through a
//! channel (ADR-025, point 1). Around it run small, independent tasks:
//!
//! * the **host scanner** (`observation.host_interval`) reads the process table
//!   on a blocking thread;
//! * the **Docker supervisor** lists containers, follows events and reconnects
//!   with growing waits, reporting "unknown" (never "gone") while it is away;
//! * the **catalog watcher** rediscovers manifests every few seconds;
//! * the **health prober** checks running instances every five seconds;
//! * the **IPC server** turns socket requests into actor messages.
//!
//! The interface reads a [`Snapshot`] from a `watch` channel and receives
//! [`Notice`]s from a `broadcast` channel; it never touches the engine.

mod actor;
mod adoption;
mod catalog_watch;
mod docker_supervisor;
mod folders;
mod handle;
mod health;
mod preferences;
mod probe;
mod rules;
mod scan;
mod uses;

use actor::{Actor, DockerState};
use catalog_watch::watch_catalog;
use docker_supervisor::supervise_docker;
pub use handle::CoreHandle;
use handle::{Dispatcher, gone};
use health::probe_health;
use scan::scan_host;

use crate::adopt::review::AdoptionBook;
pub use crate::adopt::review::AdoptionTarget as AdoptTarget;
use adoption::AdoptionMsg;
use preferences::PreferencesMsg;
use rules::RulesMsg;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use devports_proto::views::{
    AttentionItem, AttentionKind, InstanceRole, InstanceState, InstanceView, ProjectSummary,
    SharedSummary,
};
use devports_proto::{LogArgs, LogResult, Request, Response};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::adopt::apply::{self, Applied, ApplyError};
use crate::catalog::Catalog;
use crate::classify::Classification;
use crate::clock::{Clock, SystemClock};
use crate::config::{Config, SharedServices, config_dir};
use crate::discovery::{self, DiscoveryIssue, UnscannedProject};
use crate::docker::engine::EngineContainerSource;
use crate::docker::{Container, ContainerEventKind, ContainerSource};
use crate::engine::{
    ActionError, Effect, Engine, EngineError, HealthTarget, LogError, Notice, Parts,
    StopExpectation, StopReason, Target, Terminated, TerminationRequest,
};
use crate::ipc::{IpcError, IpcServer, Limits, PeerInfo, RequestHandler};
use crate::ports::{PortProbe, SystemPorts};
use crate::process::terminate::{OsSignaler, Outcome, Refusal, Timings, protected_pids, terminate};
use crate::process::{ListeningProcess, ProcessSource};
use crate::reasons::{DockerProblem, Problem, StopFailure, Unsuitability};
use crate::store::{Store, StoreError};

/// How often the engine's clock-driven rules run.
const TICK: Duration = Duration::from_secs(1);
/// How often running instances are health-checked.
const HEALTH_EVERY: Duration = Duration::from_secs(5);
/// How often manifests are looked for again.
const CATALOG_EVERY: Duration = Duration::from_secs(5);
/// Docker is re-listed at least this often even without events.
const DOCKER_RELIST: Duration = Duration::from_secs(10);
/// Waits between attempts to reach Docker, growing to the last.
const DOCKER_BACKOFF: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
];

/// Where the core keeps and finds things.
#[derive(Debug, Clone)]
pub struct CoreOptions {
    /// The user's home folder.
    pub home: PathBuf,
    /// Where the database lives.
    pub database: PathBuf,
    /// Where the socket lives.
    pub socket: PathBuf,
}

impl CoreOptions {
    /// The standard locations for the current user (`DEVPORTS_SOCKET` and
    /// `DEVPORTS_DB` override the defaults).
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        let home = PathBuf::from(var("HOME")?);
        let socket = devports_proto::paths::socket_path_from_env()?;
        let database = var("DEVPORTS_DB").map_or_else(
            || {
                home.join("Library")
                    .join("Application Support")
                    .join("devports")
                    .join("devports.db")
            },
            PathBuf::from,
        );
        Some(Self {
            home,
            database,
            socket,
        })
    }
}

/// Why the core could not start.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// `config.yml` or `shared.yml` has errors.
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    /// The database could not be opened.
    #[error("no se pudo abrir la base de datos: {0}")]
    Store(#[from] StoreError),
    /// The engine could not start.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// The socket could not be served.
    #[error(transparent)]
    Ipc(#[from] IpcError),
    /// The process source could not be created.
    #[error("{0}")]
    Source(String),
    /// A folder could not be created.
    #[error("no se pudo crear {path}: {reason}")]
    Folder {
        /// The folder.
        path: String,
        /// Why.
        reason: String,
    },
}

/// What the interface shows: the current instances and the state of the sources.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
pub struct Snapshot {
    /// Changes with every update, so a listener can tell it apart.
    pub revision: u32,
    /// Every instance worth showing.
    pub instances: Vec<InstanceView>,
    /// Whether Docker answered the last time it was asked.
    pub docker_reachable: bool,
    /// Why Docker is not reachable, if it is not.
    pub docker_error: Option<DockerProblem>,
    /// Problems the core wants the person to know about (invalid manifests,
    /// failed writes).
    pub problems: Vec<Problem>,
    /// The projects, laid out as repos with services.
    pub projects: Vec<ProjectSummary>,
    /// What each loose host process looks like, with the evidence and any correction.
    pub classifications: Vec<Classification>,
    /// Loose servers that run inside a project the search does not reach.
    pub unscanned: Vec<UnscannedProject>,
    /// The services `shared.yml` defines, with their declared start command and endpoint.
    pub shared: Vec<SharedSummary>,
    /// What waits for a decision; its length is the tray counter.
    pub attention: Vec<AttentionItem>,
    /// Minutes an "extend" adds, for the button's label.
    pub extend_minutes: u32,
}

/// A Temporal with at most this long left asks for attention.
const EXPIRING_WITHIN: Duration = Duration::from_mins(5);
/// Most engine failures kept for the interface.
const MAX_ENGINE_PROBLEMS: usize = 10;
/// How often the repositories' branches are read again.
const SUMMARY_EVERY_SECONDS: i64 = 10;

/// What waits for a decision, in display order: hung services, Temporals about
/// to expire (soonest first) and unregistered duplicates that were not kept.
#[must_use]
pub fn attention_items(instances: &[InstanceView], now: i64) -> Vec<AttentionItem> {
    let window = i64::try_from(EXPIRING_WITHIN.as_secs()).unwrap_or(i64::MAX);
    let mut hung: Vec<AttentionItem> = Vec::new();
    let mut expiring: Vec<(i64, AttentionItem)> = Vec::new();
    let mut duplicates: Vec<AttentionItem> = Vec::new();
    for view in instances {
        if view.state == InstanceState::Unhealthy && view.role == InstanceRole::Principal {
            hung.push(AttentionItem {
                kind: AttentionKind::Hung,
                instance_ref: view.instance_ref.clone(),
            });
        }
        if view.role == InstanceRole::Temporal
            && let Some(end) = view
                .expires_at
                .as_deref()
                .and_then(|text| text.parse::<jiff::Timestamp>().ok())
                .map(jiff::Timestamp::as_second)
            && end.saturating_sub(now) <= window
        {
            expiring.push((
                end,
                AttentionItem {
                    kind: AttentionKind::Expiring,
                    instance_ref: view.instance_ref.clone(),
                },
            ));
        }
        if view.role == InstanceRole::Unregistered && view.duplicate_of.is_some() && !view.ignored {
            duplicates.push(AttentionItem {
                kind: AttentionKind::Duplicate,
                instance_ref: view.instance_ref.clone(),
            });
        }
    }
    expiring.sort_by_key(|(end, _)| *end);
    hung.into_iter()
        .chain(expiring.into_iter().map(|(_, item)| item))
        .chain(duplicates)
        .collect()
}

#[derive(Clone, Copy)]
enum StopScope {
    Instance,
    Project,
}

/// A message to the actor.
enum Msg {
    Request(Request, oneshot::Sender<Response>),
    Stop(
        String,
        StopExpectation,
        StopScope,
        oneshot::Sender<Result<(), ActionError>>,
    ),
    PreviewProjectStop(String, oneshot::Sender<Vec<InstanceView>>),
    Extend(String, oneshot::Sender<Result<(), ActionError>>),
    Host(Vec<ListeningProcess>, BTreeMap<PathBuf, PathBuf>),
    Containers(Result<Vec<Container>, DockerProblem>),
    Health(Vec<(String, bool)>),
    Catalog(Box<CatalogUpdate>),
    Terminated(String, StopReason, Terminated),
    HealthTargets(oneshot::Sender<Vec<HealthTarget>>),
    Ignore(String, oneshot::Sender<Result<(), ActionError>>),
    Log(LogArgs, oneshot::Sender<Result<LogResult, LogError>>),
    Describe(String, oneshot::Sender<Option<Description>>),
    Adopted(Applied),
    Adoption(AdoptionMsg),
    UsesContext(oneshot::Sender<uses::UsesContext>),
    Rules(RulesMsg),
    Preferences(PreferencesMsg),
}

/// What an adoption needs to know about the thing being adopted.
struct Description {
    view: InstanceView,
    projects: Vec<(String, PathBuf)>,
    shared: std::collections::BTreeSet<String>,
}

struct CatalogUpdate {
    catalog: Catalog,
    shared: SharedServices,
    problems: Vec<Problem>,
}

/// A started core. Dropping it does not stop it; call [`CoreHandle::shutdown`]
/// and then [`Core::wait`].
pub struct Core {
    handle: CoreHandle,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Core {
    /// The handle for the interface and tests.
    #[must_use]
    pub fn handle(&self) -> CoreHandle {
        self.handle.clone()
    }

    /// Waits until every task has finished (after a shutdown).
    pub async fn wait(self) {
        for task in self.tasks {
            let _ = task.await;
        }
    }

    /// Starts the core against the real host.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] if a file is invalid, the database cannot be opened or
    /// another core already owns the socket.
    #[cfg(target_os = "macos")]
    pub async fn start_system(options: CoreOptions) -> Result<Self, RuntimeError> {
        let source = crate::platform::macos::MacProcessSource::new()
            .map_err(|error| RuntimeError::Source(error.to_string()))?
            .skipping_system_programs();
        Self::start(options, source).await
    }

    /// Starts the core over any process source.
    ///
    /// # Errors
    ///
    /// As [`Self::start_system`].
    pub async fn start<S: ProcessSource + 'static>(
        options: CoreOptions,
        source: S,
    ) -> Result<Self, RuntimeError> {
        let source = Arc::new(source);
        let (config, shared) = load_settings(&options.home)?;
        let discovery = discover_all(&config, &shared);
        let problems = discovery_problems(&discovery.issues);

        if let Some(folder) = options.database.parent() {
            std::fs::create_dir_all(folder).map_err(|error| RuntimeError::Folder {
                path: folder.display().to_string(),
                reason: error.to_string(),
            })?;
        }
        let store = Store::open(&options.database)?;
        // Bind first: a second core fails here before it touches anything.
        let server = IpcServer::bind(&options.socket, Limits::default()).await?;

        let observation = config.observation;
        let own_pid = std::process::id();
        let engine = Engine::new(Parts {
            clock: SystemClock,
            source: Arc::clone(&source),
            ports: SystemPorts,
            config,
            catalog: discovery.catalog,
            shared,
            store,
            entropy: Box::new(random_bytes),
            own_pid,
        })?;

        let (messages, inbox) = mpsc::channel(256);
        let (snapshot_tx, snapshot_rx) = watch::channel(Arc::new(Snapshot::default()));
        let (notices, _) = broadcast::channel(64);
        let (shutdown, shutdown_rx) = watch::channel(false);
        let (docker_tx, docker_rx) = watch::channel(None);
        let handle = CoreHandle {
            messages: messages.clone(),
            snapshot: snapshot_rx,
            notices: notices.clone(),
            shutdown,
            home: Arc::new(options.home.clone()),
        };

        let mut tasks = vec![tokio::spawn(
            Actor {
                engine,
                inbox,
                messages: messages.clone(),
                snapshot: snapshot_tx,
                notices,
                docker: docker_rx,
                source: Arc::clone(&source),
                own_pid,
                revision: 0,
                docker_state: DockerState::default(),
                catalog_problems: problems,
                shutdown: shutdown_rx.clone(),
                projects: Vec::new(),
                shared: Vec::new(),
                projects_at: 0,
                published_views: u64::MAX,
                dirty: true,
                engine_problems: Vec::new(),
                pending_stops: std::collections::BTreeMap::new(),
                adoptions: AdoptionBook::default(),
                adoption_waiters: std::collections::BTreeMap::new(),
                home: Arc::clone(&handle.home),
            }
            .run(),
        )];
        tasks.push(tokio::spawn(scan_host(
            Arc::clone(&source),
            options.home.clone(),
            observation.host_interval,
            messages.clone(),
            shutdown_rx.clone(),
        )));
        tasks.push(tokio::spawn(supervise_docker(
            docker_tx,
            messages.clone(),
            shutdown_rx.clone(),
        )));
        tasks.push(tokio::spawn(watch_catalog(
            options.home.clone(),
            messages.clone(),
            shutdown_rx.clone(),
        )));
        tasks.push(tokio::spawn(probe_health(
            messages.clone(),
            shutdown_rx.clone(),
        )));
        let dispatcher = Arc::new(Dispatcher(handle.clone()));
        let mut stop = shutdown_rx;
        tasks.push(tokio::spawn(async move {
            server
                .serve(dispatcher, async move {
                    let _ = stop.wait_for(|stopped| *stopped).await;
                })
                .await;
        }));
        Ok(Self { handle, tasks })
    }
}

fn random_bytes() -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    // The operating system's generator does not fail in practice; if it did,
    // references and tokens would be guessable, so keep asking is not an option:
    // a zeroed buffer would be accepted by the uniqueness checks but the token
    // would be public. Fall back to the clock mixed with the pid instead of
    // panicking; the engine checks references for uniqueness either way.
    if getrandom::fill(&mut bytes).is_err() {
        let nanos = jiff::Timestamp::now().as_nanosecond().to_le_bytes();
        bytes.copy_from_slice(&nanos);
    }
    bytes
}

fn load_settings(home: &std::path::Path) -> Result<(Config, SharedServices), RuntimeError> {
    let dir = config_dir(home);
    let (config, _) = Config::load(&dir.join("config.yml"), home)?;
    let (shared, _) = SharedServices::load(&dir.join("shared.yml"))?;
    Ok((config, shared))
}

fn discover_all(config: &Config, shared: &SharedServices) -> discovery::Discovery {
    discovery::discover_with(&config.roots, config.max_depth, Some(&shared.names()))
}

fn discovery_problems(issues: &[DiscoveryIssue]) -> Vec<Problem> {
    issues.iter().filter_map(DiscoveryIssue::problem).collect()
}
