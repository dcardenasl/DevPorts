//! In-memory [`ContainerSource`] for tests.

use std::sync::{Mutex, PoisonError};

use futures_util::Stream;
use futures_util::stream;
use tokio::sync::broadcast;

use super::{Container, ContainerEvent, ContainerSource, ContainerSourceError};

/// How many events the fake buffers for a slow reader.
const EVENT_BUFFER: usize = 64;

/// A [`ContainerSource`] whose containers, events and availability the test
/// controls.
#[derive(Debug)]
pub struct FakeContainerSource {
    containers: Mutex<Vec<Container>>,
    unreachable: Mutex<bool>,
    events: broadcast::Sender<ContainerEvent>,
}

impl Default for FakeContainerSource {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl FakeContainerSource {
    /// Creates a source that reports `containers`.
    #[must_use]
    pub fn new(containers: Vec<Container>) -> Self {
        Self {
            containers: Mutex::new(containers),
            unreachable: Mutex::new(false),
            events: broadcast::channel(EVENT_BUFFER).0,
        }
    }

    /// Replaces the containers the source reports.
    pub fn set(&self, containers: Vec<Container>) {
        *self
            .containers
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = containers;
    }

    /// Makes the daemon look down (`true`) or back up (`false`).
    pub fn set_unreachable(&self, unreachable: bool) {
        *self
            .unreachable
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = unreachable;
    }

    /// Delivers `event` to every open [`ContainerSource::events`] stream.
    pub fn emit(&self, event: ContainerEvent) {
        // No receiver simply means nobody is listening yet.
        let _ = self.events.send(event);
    }

    fn check_reachable(&self) -> Result<(), ContainerSourceError> {
        if *self
            .unreachable
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
        {
            Err(ContainerSourceError::Unreachable(
                "the fake daemon is down".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

impl ContainerSource for FakeContainerSource {
    async fn list(&self) -> Result<Vec<Container>, ContainerSourceError> {
        self.check_reachable()?;
        Ok(self
            .containers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn events(&self) -> impl Stream<Item = Result<ContainerEvent, ContainerSourceError>> + Send {
        let receiver = self.events.subscribe();
        stream::unfold(receiver, |mut receiver| async move {
            match receiver.recv().await {
                Ok(event) => Some((Ok(event), receiver)),
                Err(broadcast::error::RecvError::Lagged(missed)) => Some((
                    Err(ContainerSourceError::Request(format!(
                        "missed {missed} events"
                    ))),
                    receiver,
                )),
                Err(broadcast::error::RecvError::Closed) => None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::{ContainerEventKind, ContainerState};
    use futures_util::StreamExt;

    fn container(name: &str) -> Container {
        Container {
            id: format!("id-{name}"),
            name: name.to_owned(),
            image: "mysql:8".to_owned(),
            state: ContainerState::Running,
            ports: Vec::new(),
            compose: None,
            created: 1,
        }
    }

    fn event(kind: ContainerEventKind, name: &str) -> ContainerEvent {
        ContainerEvent {
            kind,
            container_id: format!("id-{name}"),
            name: Some(name.to_owned()),
            at: 2,
        }
    }

    #[tokio::test]
    async fn lists_what_it_was_given_and_what_it_is_set_to() {
        let source = FakeContainerSource::new(vec![container("a")]);
        assert_eq!(source.list().await.unwrap().len(), 1);
        source.set(vec![container("b"), container("c")]);
        assert_eq!(source.list().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_down_daemon_makes_list_fail_until_it_returns() {
        let source = FakeContainerSource::new(vec![container("a")]);
        source.set_unreachable(true);
        assert!(matches!(
            source.list().await,
            Err(ContainerSourceError::Unreachable(_))
        ));
        source.set_unreachable(false);
        assert!(source.list().await.is_ok());
    }

    #[tokio::test]
    async fn events_reach_every_open_stream_in_order() {
        let source = FakeContainerSource::default();
        let mut first = Box::pin(source.events());
        let mut second = Box::pin(source.events());

        source.emit(event(ContainerEventKind::Start, "a"));
        source.emit(event(ContainerEventKind::Die, "a"));

        for stream in [&mut first, &mut second] {
            assert_eq!(
                stream.next().await.unwrap().unwrap().kind,
                ContainerEventKind::Start
            );
            assert_eq!(
                stream.next().await.unwrap().unwrap().kind,
                ContainerEventKind::Die
            );
        }
    }
}
