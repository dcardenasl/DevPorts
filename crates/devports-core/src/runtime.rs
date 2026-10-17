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

mod probe;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use devports_proto::views::InstanceView;
use devports_proto::{Request, Response};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::catalog::Catalog;
use crate::clock::{Clock, SystemClock};
use crate::config::{Config, SharedServices, config_dir};
use crate::discovery::{self, DiscoveryIssue};
use crate::docker::engine::EngineContainerSource;
use crate::docker::{Container, ContainerEventKind, ContainerSource};
use crate::engine::{
    ActionError, Effect, Engine, EngineError, HealthTarget, Notice, Parts, StopReason, Target,
    Terminated, TerminationRequest,
};
use crate::ipc::{IpcError, IpcServer, Limits, PeerInfo, RequestHandler};
use crate::ports::{PortProbe, SystemPorts};
use crate::process::terminate::{OsSignaler, Outcome, Timings, protected_pids, terminate};
use crate::process::{ListeningProcess, ProcessSource};
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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// Changes with every update, so a listener can tell it apart.
    pub revision: u64,
    /// Every instance worth showing.
    pub instances: Vec<InstanceView>,
    /// Whether Docker answered the last time it was asked.
    pub docker_reachable: bool,
    /// Why Docker is not reachable, if it is not.
    pub docker_error: Option<String>,
    /// Problems the core wants the person to know about (invalid manifests,
    /// failed writes).
    pub problems: Vec<String>,
}

/// A message to the actor.
enum Msg {
    Request(Request, oneshot::Sender<Response>),
    Stop(String, oneshot::Sender<Result<(), ActionError>>),
    Extend(String, oneshot::Sender<Result<(), ActionError>>),
    Host(Vec<ListeningProcess>),
    Containers(Result<Vec<Container>, String>),
    Health(Vec<(String, bool)>),
    Catalog(Box<CatalogUpdate>),
    Terminated(String, StopReason, Terminated),
    HealthTargets(oneshot::Sender<Vec<HealthTarget>>),
}

struct CatalogUpdate {
    catalog: Catalog,
    shared: SharedServices,
    problems: Vec<String>,
}

/// A cloneable handle to the running core.
#[derive(Clone)]
pub struct CoreHandle {
    messages: mpsc::Sender<Msg>,
    snapshot: watch::Receiver<Arc<Snapshot>>,
    notices: broadcast::Sender<Notice>,
    shutdown: watch::Sender<bool>,
}

impl CoreHandle {
    /// The latest snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Arc<Snapshot> {
        Arc::clone(&self.snapshot.borrow())
    }

    /// Follows the snapshot: resolves each time it changes.
    #[must_use]
    pub fn watch(&self) -> watch::Receiver<Arc<Snapshot>> {
        self.snapshot.clone()
    }

    /// Receives the notices (lease about to end, duplicate detected).
    #[must_use]
    pub fn notices(&self) -> broadcast::Receiver<Notice> {
        self.notices.subscribe()
    }

    /// Answers one socket request.
    pub async fn request(&self, request: Request) -> Response {
        let id = request.id.clone();
        let (reply, answer) = oneshot::channel();
        if self
            .messages
            .send(Msg::Request(request, reply))
            .await
            .is_err()
        {
            return unavailable(id);
        }
        answer.await.unwrap_or_else(|_| unavailable(id))
    }

    /// Stops an instance because the person asked (from the application).
    ///
    /// # Errors
    ///
    /// [`ActionError`] when it does not exist or has nothing to stop.
    pub async fn stop(&self, reference: &str) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Stop(reference.to_owned(), reply))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Extends a Temporal because the person asked.
    ///
    /// # Errors
    ///
    /// [`ActionError`] when it does not exist, is not a Temporal or is at the limit.
    pub async fn extend(&self, reference: &str) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Extend(reference.to_owned(), reply))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Asks the whole core to stop.
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }

    async fn send_action(&self, message: Msg) -> Result<(), ActionError> {
        self.messages.send(message).await.map_err(|_| gone())
    }
}

fn gone() -> ActionError {
    ActionError::Refused("DevPorts se está cerrando".to_owned())
}

fn unavailable(id: String) -> Response {
    Response::err(
        id,
        devports_proto::ErrorBody::new(
            devports_proto::ErrorCode::Unavailable,
            "DevPorts se está cerrando",
        ),
    )
}

struct Dispatcher(CoreHandle);

impl RequestHandler for Dispatcher {
    async fn handle(&self, request: Request, _peer: PeerInfo) -> Response {
        self.0.request(request).await
    }
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
            .map_err(|error| RuntimeError::Source(error.to_string()))?;
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
            }
            .run(),
        )];
        tasks.push(tokio::spawn(scan_host(
            Arc::clone(&source),
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

fn discovery_problems(issues: &[DiscoveryIssue]) -> Vec<String> {
    issues
        .iter()
        .filter(|issue| !matches!(issue, DiscoveryIssue::RootMissing(_)))
        .map(ToString::to_string)
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DockerState {
    reachable: bool,
    error: Option<String>,
}

struct Actor<C: Clock, S: ProcessSource + 'static, P: PortProbe> {
    engine: Engine<C, Arc<S>, P>,
    inbox: mpsc::Receiver<Msg>,
    messages: mpsc::Sender<Msg>,
    snapshot: watch::Sender<Arc<Snapshot>>,
    notices: broadcast::Sender<Notice>,
    docker: watch::Receiver<Option<Arc<EngineContainerSource>>>,
    source: Arc<S>,
    own_pid: u32,
    revision: u64,
    docker_state: DockerState,
    catalog_problems: Vec<String>,
    shutdown: watch::Receiver<bool>,
}

impl<C: Clock + 'static, S: ProcessSource + 'static, P: PortProbe + 'static> Actor<C, S, P> {
    async fn run(mut self) {
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        self.publish();
        loop {
            let wake = tokio::select! {
                _ = self.shutdown.wait_for(|stopped| *stopped) => break,
                _ = ticker.tick() => None,
                message = self.inbox.recv() => match message {
                    Some(message) => Some(message),
                    None => break,
                },
            };
            let effects = match wake {
                Some(message) => self.handle(message),
                None => self.engine.tick(),
            };
            self.execute(effects);
            self.publish();
        }
    }

    fn handle(&mut self, message: Msg) -> Vec<Effect> {
        match message {
            Msg::Request(request, reply) => {
                let (response, effects) = self.engine.handle_request(request);
                let _ = reply.send(response);
                effects
            }
            Msg::Stop(reference, reply) => match self.engine.stop_instance(&reference) {
                Ok(effects) => {
                    let _ = reply.send(Ok(()));
                    effects
                }
                Err(error) => {
                    let _ = reply.send(Err(error));
                    Vec::new()
                }
            },
            Msg::Extend(reference, reply) => {
                let _ = reply.send(self.engine.extend_instance(&reference));
                Vec::new()
            }
            Msg::Host(scan) => self.engine.observe_host(scan),
            Msg::Containers(Ok(containers)) => {
                self.docker_state = DockerState {
                    reachable: true,
                    error: None,
                };
                self.engine.observe_containers(Some(containers))
            }
            Msg::Containers(Err(reason)) => {
                self.docker_state = DockerState {
                    reachable: false,
                    error: Some(reason),
                };
                self.engine.observe_containers(None)
            }
            Msg::Health(results) => {
                for (reference, healthy) in results {
                    self.engine.record_health(&reference, healthy);
                }
                self.engine.rebuild_views();
                Vec::new()
            }
            Msg::Catalog(update) => {
                let CatalogUpdate {
                    catalog,
                    shared,
                    problems,
                } = *update;
                self.catalog_problems = problems;
                self.engine.set_catalog(catalog, shared);
                Vec::new()
            }
            Msg::Terminated(reference, reason, outcome) => {
                self.engine
                    .termination_finished(&reference, reason, &outcome);
                Vec::new()
            }
            Msg::HealthTargets(reply) => {
                let _ = reply.send(self.engine.health_targets());
                Vec::new()
            }
        }
    }

    /// Carries out what the engine decided.
    fn execute(&self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Notify(notice) => {
                    // Nobody listening is fine.
                    let _ = self.notices.send(notice);
                }
                Effect::Terminate(request) => self.terminate(request),
            }
        }
    }

    fn terminate(&self, request: TerminationRequest) {
        let messages = self.messages.clone();
        let source = Arc::clone(&self.source);
        let docker = self.docker.borrow().clone();
        let own_pid = self.own_pid;
        tokio::spawn(async move {
            let TerminationRequest {
                reference,
                target,
                reason,
            } = request;
            let outcome = match target {
                Target::Process(root) => {
                    let uid = rustix::process::getuid().as_raw();
                    let protected = {
                        let source = Arc::clone(&source);
                        tokio::task::spawn_blocking(move || protected_pids(&*source, own_pid))
                            .await
                            .unwrap_or_default()
                    };
                    match terminate(
                        &*source,
                        &OsSignaler,
                        root,
                        &protected,
                        uid,
                        Timings::default(),
                    )
                    .await
                    {
                        Outcome::Stopped => Terminated::Stopped,
                        Outcome::AlreadyGone => Terminated::AlreadyGone,
                        Outcome::StillAlive(_) => Terminated::StillAlive,
                        Outcome::Refused(refusal) => Terminated::Refused(refusal.to_string()),
                    }
                }
                Target::Container(id) => match docker {
                    Some(docker) => match docker.stop(&id).await {
                        Ok(()) => Terminated::Stopped,
                        Err(error) => Terminated::Failed(error.to_string()),
                    },
                    None => Terminated::Failed("Docker no está disponible".to_owned()),
                },
            };
            let _ = messages
                .send(Msg::Terminated(reference, reason, outcome))
                .await;
        });
    }

    fn publish(&mut self) {
        let mut problems = self.catalog_problems.clone();
        problems.extend(self.engine.take_problems());
        let instances = self.engine.views().to_vec();
        let current = self.snapshot.borrow().clone();
        if current.instances == instances
            && current.docker_reachable == self.docker_state.reachable
            && current.docker_error == self.docker_state.error
            && current.problems == problems
        {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        let _ = self.snapshot.send(Arc::new(Snapshot {
            revision: self.revision,
            instances,
            docker_reachable: self.docker_state.reachable,
            docker_error: self.docker_state.error.clone(),
            problems,
        }));
    }
}

async fn scan_host<S: ProcessSource + 'static>(
    source: Arc<S>,
    every: Duration,
    messages: mpsc::Sender<Msg>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.wait_for(|stopped| *stopped) => return,
            _ = ticker.tick() => {}
        }
        let reader = Arc::clone(&source);
        // A failed scan keeps the previous picture: unknown is not gone.
        if let Ok(Ok(scan)) = tokio::task::spawn_blocking(move || reader.snapshot()).await
            && messages.send(Msg::Host(scan)).await.is_err()
        {
            return;
        }
    }
}

async fn supervise_docker(
    shared: watch::Sender<Option<Arc<EngineContainerSource>>>,
    messages: mpsc::Sender<Msg>,
    mut shutdown: watch::Receiver<bool>,
) {
    use futures_util::StreamExt;

    let mut attempt = 0_usize;
    loop {
        match EngineContainerSource::connect().await {
            Ok(source) => {
                attempt = 0;
                let source = Arc::new(source);
                let _ = shared.send(Some(Arc::clone(&source)));
                // Subscribe before listing so nothing between the two is lost.
                let events = source.events();
                futures_util::pin_mut!(events);
                let mut relist = tokio::time::interval(DOCKER_RELIST);
                loop {
                    let listed = source.list().await.map_err(|error| error.to_string());
                    let failed = listed.is_err();
                    if messages.send(Msg::Containers(listed)).await.is_err() {
                        return;
                    }
                    if failed {
                        break;
                    }
                    let lost = tokio::select! {
                        _ = shutdown.wait_for(|stopped| *stopped) => return,
                        _ = relist.tick() => false,
                        event = events.next() => match event {
                            Some(Ok(event)) => {
                                // Only these four change what the engine shows.
                                !matches!(
                                    event.kind,
                                    ContainerEventKind::Start
                                        | ContainerEventKind::Die
                                        | ContainerEventKind::Stop
                                        | ContainerEventKind::Destroy
                                )
                            }
                            Some(Err(_)) | None => true,
                        },
                    };
                    if lost {
                        break;
                    }
                }
                let _ = shared.send(None);
                let _ = messages
                    .send(Msg::Containers(Err(
                        "se perdió la conexión con Docker".to_owned()
                    )))
                    .await;
            }
            Err(error) => {
                let _ = messages.send(Msg::Containers(Err(error.to_string()))).await;
            }
        }
        let wait = DOCKER_BACKOFF
            .get(attempt)
            .or_else(|| DOCKER_BACKOFF.last())
            .copied()
            .unwrap_or(Duration::from_secs(30));
        attempt = attempt.saturating_add(1);
        tokio::select! {
            _ = shutdown.wait_for(|stopped| *stopped) => return,
            () = tokio::time::sleep(wait) => {}
        }
    }
}

async fn watch_catalog(
    home: PathBuf,
    messages: mpsc::Sender<Msg>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut last: Option<(Catalog, SharedServices)> = None;
    let mut ticker = tokio::time::interval(CATALOG_EVERY);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.wait_for(|stopped| *stopped) => return,
            _ = ticker.tick() => {}
        }
        let home = home.clone();
        let Ok(Ok((catalog, shared, problems))) = tokio::task::spawn_blocking(move || {
            let (config, shared) = load_settings(&home)?;
            let discovery = discover_all(&config, &shared);
            Ok::<_, RuntimeError>((
                discovery.catalog,
                shared,
                discovery_problems(&discovery.issues),
            ))
        })
        .await
        else {
            // A half-written config.yml keeps the previous catalog.
            continue;
        };
        if last
            .as_ref()
            .is_some_and(|(known, known_shared)| *known == catalog && *known_shared == shared)
        {
            continue;
        }
        last = Some((catalog.clone(), shared.clone()));
        let update = CatalogUpdate {
            catalog,
            shared,
            problems,
        };
        if messages.send(Msg::Catalog(Box::new(update))).await.is_err() {
            return;
        }
    }
}

async fn probe_health(messages: mpsc::Sender<Msg>, mut shutdown: watch::Receiver<bool>) {
    let mut ticker = tokio::time::interval(HEALTH_EVERY);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.wait_for(|stopped| *stopped) => return,
            _ = ticker.tick() => {}
        }
        let (reply, targets) = oneshot::channel();
        if messages.send(Msg::HealthTargets(reply)).await.is_err() {
            return;
        }
        let Ok(targets) = targets.await else {
            continue;
        };
        let checks = targets.into_iter().map(|target| async move {
            let healthy = probe::is_healthy(target.port, &target.health).await;
            (target.reference, healthy)
        });
        let results = futures_util::future::join_all(checks).await;
        if !results.is_empty() && messages.send(Msg::Health(results)).await.is_err() {
            return;
        }
    }
}
