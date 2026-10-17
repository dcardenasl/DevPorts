//! [`ContainerSource`] backed by the Docker Engine API (`bollard`).

use std::collections::HashMap;
use std::path::PathBuf;

use bollard::API_DEFAULT_VERSION;
use bollard::Docker;
use bollard::models::{
    ContainerSummary, ContainerSummaryStateEnum, EventMessage, PortSummary, PortSummaryTypeEnum,
};
use bollard::query_parameters::{EventsOptionsBuilder, ListContainersOptionsBuilder};
use futures_util::{Stream, StreamExt};

use super::context::{ContextEnv, Endpoint, ResolvedContext, resolve};
use super::{
    ComposeLabels, Container, ContainerEvent, ContainerEventKind, ContainerSource,
    ContainerSourceError, ContainerState, PublishedPort,
};

/// Seconds a request may take before it is abandoned.
const REQUEST_TIMEOUT_SECONDS: u64 = 30;

const COMPOSE_PROJECT_LABEL: &str = "com.docker.compose.project";
const COMPOSE_WORKING_DIR_LABEL: &str = "com.docker.compose.project.working_dir";

/// The Docker Engine reached through the active Docker context.
#[derive(Debug, Clone)]
pub struct EngineContainerSource {
    docker: Docker,
    context: ResolvedContext,
}

impl EngineContainerSource {
    /// Resolves the active context from the process environment and connects.
    ///
    /// # Errors
    ///
    /// Fails if the context cannot be resolved, uses an unsupported transport,
    /// or the daemon is not reachable.
    pub async fn connect() -> Result<Self, ContainerSourceError> {
        let context = resolve(&ContextEnv::from_process())
            .map_err(|error| ContainerSourceError::Context(error.to_string()))?;
        Self::connect_to(context).await
    }

    /// Connects to an already resolved context.
    ///
    /// # Errors
    ///
    /// As [`Self::connect`], except for context resolution.
    pub async fn connect_to(context: ResolvedContext) -> Result<Self, ContainerSourceError> {
        let path = match &context.endpoint {
            Endpoint::Unix(path) => path,
            Endpoint::Other(host) => {
                return Err(ContainerSourceError::UnsupportedEndpoint(host.clone()));
            }
        };
        let docker = Docker::connect_with_unix(
            &path.to_string_lossy(),
            REQUEST_TIMEOUT_SECONDS,
            API_DEFAULT_VERSION,
        )
        .map_err(|error| ContainerSourceError::Unreachable(error.to_string()))?
        .negotiate_version()
        .await
        .map_err(|error| ContainerSourceError::Unreachable(error.to_string()))?;
        Ok(Self { docker, context })
    }

    /// The context this source is connected through.
    #[must_use]
    pub fn context(&self) -> &ResolvedContext {
        &self.context
    }
}

impl ContainerSource for EngineContainerSource {
    async fn list(&self) -> Result<Vec<Container>, ContainerSourceError> {
        let options = ListContainersOptionsBuilder::new().all(true).build();
        let summaries = self
            .docker
            .list_containers(Some(options))
            .await
            .map_err(|error| ContainerSourceError::Request(error.to_string()))?;
        Ok(summaries.into_iter().filter_map(container_from).collect())
    }

    fn events(&self) -> impl Stream<Item = Result<ContainerEvent, ContainerSourceError>> + Send {
        let filters: HashMap<String, Vec<String>> = HashMap::from([
            ("type".to_owned(), vec!["container".to_owned()]),
            (
                "event".to_owned(),
                ["start", "die", "stop", "destroy"]
                    .map(str::to_owned)
                    .to_vec(),
            ),
        ]);
        // The engine only starts the subscription when the stream is first polled.
        // `since` makes it replay anything that happened between this call and that
        // poll, so no event is lost in the gap; at most the previous second repeats,
        // which is harmless because consumers reconcile against `list`.
        let since = jiff::Timestamp::now().as_second().to_string();
        let options = EventsOptionsBuilder::new()
            .since(&since)
            .filters(&filters)
            .build();
        self.docker
            .events(Some(options))
            .filter_map(|message| async {
                match message {
                    Ok(message) => event_from(&message).map(Ok),
                    Err(error) => Some(Err(ContainerSourceError::Request(error.to_string()))),
                }
            })
    }
}

/// Converts the engine's summary. A summary without an id cannot be addressed
/// and is dropped.
fn container_from(summary: ContainerSummary) -> Option<Container> {
    let id = summary.id?;
    let name = summary
        .names
        .and_then(|names| names.into_iter().next())
        .map(|name| name.trim_start_matches('/').to_owned())
        .unwrap_or_default();
    let labels = summary.labels.unwrap_or_default();
    let mut ports: Vec<PublishedPort> = summary
        .ports
        .unwrap_or_default()
        .iter()
        .filter_map(published_port)
        .collect();
    ports.sort_by_key(|port| (port.host_port, port.container_port));
    ports.dedup();
    Some(Container {
        id,
        name,
        image: summary.image.unwrap_or_default(),
        state: state_from(summary.state),
        ports,
        compose: compose_from(&labels),
        created: summary.created.unwrap_or_default(),
    })
}

/// A TCP port that is published on the host.
fn published_port(port: &PortSummary) -> Option<PublishedPort> {
    let is_tcp = matches!(port.typ, Some(PortSummaryTypeEnum::TCP) | None);
    let host_port = port.public_port?;
    is_tcp.then_some(PublishedPort {
        host_port,
        container_port: port.private_port,
    })
}

fn compose_from(labels: &HashMap<String, String>) -> Option<ComposeLabels> {
    let project = labels
        .get(COMPOSE_PROJECT_LABEL)
        .filter(|project| !project.is_empty())?;
    Some(ComposeLabels {
        project: project.clone(),
        working_dir: labels
            .get(COMPOSE_WORKING_DIR_LABEL)
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from),
    })
}

fn state_from(state: Option<ContainerSummaryStateEnum>) -> ContainerState {
    match state {
        Some(ContainerSummaryStateEnum::CREATED) => ContainerState::Created,
        Some(ContainerSummaryStateEnum::RUNNING) => ContainerState::Running,
        Some(ContainerSummaryStateEnum::PAUSED) => ContainerState::Paused,
        Some(ContainerSummaryStateEnum::RESTARTING) => ContainerState::Restarting,
        Some(ContainerSummaryStateEnum::EXITED) => ContainerState::Exited,
        Some(ContainerSummaryStateEnum::REMOVING) => ContainerState::Removing,
        Some(ContainerSummaryStateEnum::DEAD) => ContainerState::Dead,
        Some(ContainerSummaryStateEnum::STOPPING) => ContainerState::Stopping,
        Some(ContainerSummaryStateEnum::EMPTY) | None => ContainerState::Unknown,
    }
}

/// Converts an engine event; `None` for kinds DevPorts does not track.
fn event_from(message: &EventMessage) -> Option<ContainerEvent> {
    let kind = match message.action.as_deref()? {
        "start" => ContainerEventKind::Start,
        "die" => ContainerEventKind::Die,
        "stop" => ContainerEventKind::Stop,
        "destroy" => ContainerEventKind::Destroy,
        _ => return None,
    };
    let actor = message.actor.as_ref()?;
    Some(ContainerEvent {
        kind,
        container_id: actor.id.clone()?,
        name: actor
            .attributes
            .as_ref()
            .and_then(|attributes| attributes.get("name"))
            .cloned(),
        at: message.time.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::models::EventActor;

    fn summary() -> ContainerSummary {
        ContainerSummary {
            id: Some("abc123".to_owned()),
            names: Some(vec!["/mysql".to_owned(), "/alias".to_owned()]),
            image: Some("mysql:8.0".to_owned()),
            created: Some(1_700_000_000),
            state: Some(ContainerSummaryStateEnum::RUNNING),
            ports: Some(vec![
                PortSummary {
                    ip: Some("0.0.0.0".to_owned()),
                    private_port: 3306,
                    public_port: Some(3306),
                    typ: Some(PortSummaryTypeEnum::TCP),
                },
                PortSummary {
                    ip: Some("::".to_owned()),
                    private_port: 3306,
                    public_port: Some(3306),
                    typ: Some(PortSummaryTypeEnum::TCP),
                },
                PortSummary {
                    ip: None,
                    private_port: 33060,
                    public_port: None,
                    typ: Some(PortSummaryTypeEnum::TCP),
                },
                PortSummary {
                    ip: Some("0.0.0.0".to_owned()),
                    private_port: 53,
                    public_port: Some(5353),
                    typ: Some(PortSummaryTypeEnum::UDP),
                },
            ]),
            labels: Some(HashMap::from([
                (
                    COMPOSE_PROJECT_LABEL.to_owned(),
                    "docker-services".to_owned(),
                ),
                (
                    COMPOSE_WORKING_DIR_LABEL.to_owned(),
                    "/Users/me/docker-services".to_owned(),
                ),
            ])),
            ..ContainerSummary::default()
        }
    }

    #[test]
    fn a_summary_becomes_a_container() {
        let container = container_from(summary()).unwrap();
        assert_eq!(container.id, "abc123");
        assert_eq!(container.name, "mysql");
        assert_eq!(container.image, "mysql:8.0");
        assert_eq!(container.state, ContainerState::Running);
        assert_eq!(container.created, 1_700_000_000);
    }

    #[test]
    fn only_published_tcp_ports_are_kept_once_each() {
        let container = container_from(summary()).unwrap();
        assert_eq!(
            container.ports,
            [PublishedPort {
                host_port: 3306,
                container_port: 3306
            }]
        );
    }

    #[test]
    fn compose_labels_are_read() {
        let container = container_from(summary()).unwrap();
        assert_eq!(
            container.compose,
            Some(ComposeLabels {
                project: "docker-services".to_owned(),
                working_dir: Some("/Users/me/docker-services".into()),
            })
        );
    }

    #[test]
    fn a_container_without_compose_labels_has_none() {
        let mut plain = summary();
        plain.labels = None;
        assert_eq!(container_from(plain).unwrap().compose, None);
    }

    #[test]
    fn a_summary_without_an_id_is_dropped() {
        let mut anonymous = summary();
        anonymous.id = None;
        assert!(container_from(anonymous).is_none());
    }

    #[test]
    fn every_engine_state_has_a_mapping() {
        assert_eq!(state_from(None), ContainerState::Unknown);
        assert_eq!(
            state_from(Some(ContainerSummaryStateEnum::EXITED)),
            ContainerState::Exited
        );
        assert!(state_from(Some(ContainerSummaryStateEnum::RESTARTING)).is_running());
        assert!(!state_from(Some(ContainerSummaryStateEnum::PAUSED)).is_running());
    }

    fn message(action: &str) -> EventMessage {
        EventMessage {
            action: Some(action.to_owned()),
            actor: Some(EventActor {
                id: Some("abc123".to_owned()),
                attributes: Some(HashMap::from([("name".to_owned(), "mysql".to_owned())])),
            }),
            time: Some(1_700_000_001),
            ..EventMessage::default()
        }
    }

    #[test]
    fn the_four_tracked_actions_become_events() {
        for (action, kind) in [
            ("start", ContainerEventKind::Start),
            ("die", ContainerEventKind::Die),
            ("stop", ContainerEventKind::Stop),
            ("destroy", ContainerEventKind::Destroy),
        ] {
            let event = event_from(&message(action)).unwrap();
            assert_eq!(event.kind, kind);
            assert_eq!(event.container_id, "abc123");
            assert_eq!(event.name.as_deref(), Some("mysql"));
            assert_eq!(event.at, 1_700_000_001);
        }
    }

    #[test]
    fn other_actions_and_incomplete_messages_are_ignored() {
        assert!(event_from(&message("exec_start")).is_none());
        let mut no_actor = message("start");
        no_actor.actor = None;
        assert!(event_from(&no_actor).is_none());
        let mut no_action = message("start");
        no_action.action = None;
        assert!(event_from(&no_action).is_none());
    }
}
