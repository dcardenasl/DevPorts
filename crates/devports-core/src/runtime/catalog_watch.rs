//! The catalog watcher task: re-reads manifests and configuration when they change.
use super::{
    CATALOG_EVERY, Catalog, CatalogUpdate, Msg, PathBuf, RuntimeError, SharedServices,
    discover_all, discovery_problems, load_settings, mpsc, watch,
};

pub(super) async fn watch_catalog(
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
