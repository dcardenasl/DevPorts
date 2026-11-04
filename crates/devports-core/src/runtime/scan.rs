//! The host scanner task.
use super::{Arc, Duration, Msg, ProcessSource, mpsc, watch};

pub(super) async fn scan_host<S: ProcessSource + 'static>(
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
