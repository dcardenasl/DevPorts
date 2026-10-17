//! Observation of Docker containers.
//!
//! Like processes, containers reach the core through a trait, [`ContainerSource`],
//! so everything above it is tested against [`fake::FakeContainerSource`]. The
//! real implementation talks to the Docker Engine API over the socket of the
//! *active* Docker context, which covers Docker Desktop, OrbStack and Colima
//! without hard-coded paths (ADR-012).

pub mod context;
pub mod engine;
pub mod fake;

use std::path::PathBuf;

use futures_util::Stream;

/// Lifecycle state of a container as the Docker Engine reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerState {
    /// Created but never started.
    Created,
    /// Running.
    Running,
    /// Paused.
    Paused,
    /// Being restarted by its restart policy.
    Restarting,
    /// Stopped.
    Exited,
    /// Being removed.
    Removing,
    /// Could not be stopped or removed properly.
    Dead,
    /// Being stopped.
    Stopping,
    /// The engine reported a state this build does not know.
    Unknown,
}

impl ContainerState {
    /// Whether the container is serving, or about to be again.
    #[must_use]
    pub fn is_running(self) -> bool {
        matches!(self, Self::Running | Self::Restarting)
    }
}

/// A TCP port a container publishes on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPort {
    /// Port on the host.
    pub host_port: u16,
    /// Port inside the container.
    pub container_port: u16,
}

/// The `docker compose` labels of a container (ADR-012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeLabels {
    /// `com.docker.compose.project`.
    pub project: String,
    /// `com.docker.compose.project.working_dir`, when present.
    pub working_dir: Option<PathBuf>,
}

/// A container, reduced to what DevPorts uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    /// Full container id.
    pub id: String,
    /// Container name, without Docker's leading slash.
    pub name: String,
    /// Image reference.
    pub image: String,
    /// Lifecycle state.
    pub state: ContainerState,
    /// TCP ports published on the host, sorted by host port.
    pub ports: Vec<PublishedPort>,
    /// Compose labels, if the container belongs to a compose project.
    pub compose: Option<ComposeLabels>,
    /// Creation time, seconds since the Unix epoch.
    pub created: i64,
}

/// What happened to a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerEventKind {
    /// The container started.
    Start,
    /// The main process exited.
    Die,
    /// The container was stopped.
    Stop,
    /// The container was removed.
    Destroy,
}

/// A container lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerEvent {
    /// What happened.
    pub kind: ContainerEventKind,
    /// Id of the container.
    pub container_id: String,
    /// Name of the container, when the engine reported it.
    pub name: Option<String>,
    /// When it happened, seconds since the Unix epoch.
    pub at: i64,
}

/// Why the container source could not answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContainerSourceError {
    /// The active Docker context could not be resolved.
    #[error("could not resolve the Docker context: {0}")]
    Context(String),
    /// The endpoint uses a transport this build does not speak.
    #[error("unsupported Docker endpoint: {0}")]
    UnsupportedEndpoint(String),
    /// The daemon is not reachable (not running, or the socket is gone).
    #[error("cannot reach the Docker daemon: {0}")]
    Unreachable(String),
    /// The daemon answered with an error.
    #[error("Docker request failed: {0}")]
    Request(String),
}

/// Source of truth about the host's containers.
///
/// Uses native `async fn` in traits, so it is used through generics rather than
/// as a trait object.
pub trait ContainerSource: Send + Sync {
    /// Every container, running or not (a stopped declared service must show as
    /// down, not vanish).
    fn list(&self) -> impl Future<Output = Result<Vec<Container>, ContainerSourceError>> + Send;

    /// A stream of lifecycle events, starting from the moment of this call: events
    /// that happen before the stream is first polled are not lost. Up to the
    /// previous second may repeat, so consumers treat events as hints and
    /// reconcile against [`Self::list`]. The stream ends, or yields an error,
    /// when the connection to the daemon is lost; reconnecting is the caller's
    /// policy.
    fn events(&self) -> impl Stream<Item = Result<ContainerEvent, ContainerSourceError>> + Send;
}
