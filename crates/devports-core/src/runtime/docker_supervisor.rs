//! The Docker supervisor task: reports "unknown", never "gone", when the daemon is lost.
use super::{
    Arc, ContainerEventKind, ContainerSource, DOCKER_BACKOFF, DOCKER_RELIST, DockerProblem,
    Duration, EngineContainerSource, Msg, RulesMsg, mpsc, watch,
};

pub(super) async fn supervise_docker(
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
                // Container rules are scoped to the context, so it is known first.
                let context = source.context().name.clone();
                if messages
                    .send(Msg::Rules(RulesMsg::DockerContext(context)))
                    .await
                    .is_err()
                {
                    return;
                }
                // Subscribe before listing so nothing between the two is lost.
                let events = source.events();
                futures_util::pin_mut!(events);
                let mut relist = tokio::time::interval(DOCKER_RELIST);
                loop {
                    let listed = source
                        .list()
                        .await
                        .map_err(|error| DockerProblem::Unreachable {
                            detail: error.to_string(),
                        });
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
                    .send(Msg::Containers(Err(DockerProblem::ConnectionLost)))
                    .await;
            }
            Err(error) => {
                let _ = messages
                    .send(Msg::Containers(Err(DockerProblem::Unreachable {
                        detail: error.to_string(),
                    })))
                    .await;
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
