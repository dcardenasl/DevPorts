//! The health probing task.
use super::{HEALTH_EVERY, Msg, mpsc, oneshot, probe, watch};

pub(super) async fn probe_health(messages: mpsc::Sender<Msg>, mut shutdown: watch::Receiver<bool>) {
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
