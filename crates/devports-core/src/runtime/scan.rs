//! The host scanner task.
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Instant;

use super::{Arc, Duration, Msg, ProcessSource, mpsc, watch};
use crate::discovery::enclosing_manifest_dir;

/// How long what was found about a folder is trusted before looking again.
const HINT_TTL: Duration = Duration::from_secs(30);

pub(super) async fn scan_host<S: ProcessSource + 'static>(
    source: Arc<S>,
    home: PathBuf,
    every: Duration,
    messages: mpsc::Sender<Msg>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut remembered: HashMap<PathBuf, (Instant, Option<PathBuf>)> = HashMap::new();
    loop {
        tokio::select! {
            _ = shutdown.wait_for(|stopped| *stopped) => return,
            _ = ticker.tick() => {}
        }
        let reader = Arc::clone(&source);
        // A failed scan keeps the previous picture: unknown is not gone.
        let Ok(Ok(scan)) = tokio::task::spawn_blocking(move || reader.snapshot()).await else {
            continue;
        };
        let (hints, kept) = tokio::task::spawn_blocking({
            let home = home.clone();
            let previous = remembered.clone();
            let folders: Vec<PathBuf> = scan
                .iter()
                .filter_map(|process| process.info.cwd.clone())
                .collect();
            move || manifest_hints(&home, folders, &previous)
        })
        .await
        .unwrap_or_default();
        remembered = kept;
        if messages.send(Msg::Host(scan, hints)).await.is_err() {
            return;
        }
    }
}

type Remembered = HashMap<PathBuf, (Instant, Option<PathBuf>)>;

/// For each folder a listener runs in, the nearest folder above it with a manifest,
/// reusing what was found less than [`HINT_TTL`] ago so the disk is not read every scan.
fn manifest_hints(
    home: &std::path::Path,
    folders: Vec<PathBuf>,
    previous: &Remembered,
) -> (BTreeMap<PathBuf, PathBuf>, Remembered) {
    let now = Instant::now();
    let mut kept = Remembered::new();
    let mut hints = BTreeMap::new();
    for folder in folders {
        if kept.contains_key(&folder) {
            continue;
        }
        let found = match previous.get(&folder) {
            Some((at, found)) if now.duration_since(*at) < HINT_TTL => found.clone(),
            _ => enclosing_manifest_dir(&folder, home),
        };
        if let Some(dir) = &found {
            hints.insert(folder.clone(), dir.clone());
        }
        let at = previous.get(&folder).map_or(now, |(at, _)| {
            if now.duration_since(*at) < HINT_TTL {
                *at
            } else {
                now
            }
        });
        kept.insert(folder, (at, found));
    }
    (hints, kept)
}
