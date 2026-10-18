//! Editing the scan folders through the running actor: discovery follows without a restart.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use devports_core::process::fake::FakeProcessSource;
use devports_core::runtime::{Core, CoreHandle, CoreOptions};
use devports_core::settings::{SettingsError, SettingsOutcome};
use tempfile::TempDir;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    std::fs::write(path, text).expect("write");
}

fn project(root: &Path, name: &str) {
    write(
        &root.join(name).join(".devports.yml"),
        &format!("version: 1\nproject: {name}\nservices:\n  api:\n    cwd: .\n"),
    );
}

async fn start(home: &TempDir) -> (Core, CoreHandle) {
    let core = Core::start(
        CoreOptions {
            home: home.path().to_path_buf(),
            database: home.path().join("db"),
            socket: home.path().join("s/d.sock"),
        },
        Arc::new(FakeProcessSource::new(Vec::new())),
    )
    .await
    .expect("core");
    let handle = core.handle();
    (core, handle)
}

async fn projects(handle: &CoreHandle, expected: &[&str]) {
    for _ in 0..100 {
        let mut names: Vec<String> = handle
            .snapshot()
            .projects
            .iter()
            .map(|project| project.name.clone())
            .collect();
        names.sort();
        if names == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the snapshot never listed {expected:?}");
}

// covers: UXR-6.1
#[tokio::test]
async fn a_new_folder_is_discovered_at_once_and_removing_it_deletes_nothing() {
    let home = TempDir::new().expect("home");
    let base = std::fs::canonicalize(home.path()).expect("canonical");
    let work = base.join("work");
    let more = base.join("more");
    project(&work, "shop");
    project(&more, "blog");
    write(
        &base.join(".config/devports/config.yml"),
        &format!(
            "# mine\nversion: 1\nroots: [\"{}\"]\nobservation:\n  host_interval: 250ms\n",
            work.display()
        ),
    );
    let (core, handle) = start(&home).await;
    projects(&handle, &["shop"]).await;
    let seen = handle.scan_folders().await.expect("folders");
    assert!(seen.editable && seen.roots[0].projects == 1);

    let both = vec![work.display().to_string(), more.display().to_string()];
    let shown = handle
        .preview_scan_folders(both.clone())
        .await
        .expect("preview");
    assert_eq!(shown.added, ["blog"]);
    let reviewed = std::fs::read_to_string(base.join(".config/devports/config.yml")).expect("read");
    assert!(
        reviewed.contains("roots: [\"") && !reviewed.contains("more"),
        "preview wrote nothing"
    );

    let outcome = handle
        .set_scan_folders(both.clone(), seen.base.clone())
        .await
        .expect("saved");
    assert_eq!(outcome, SettingsOutcome::Saved);
    projects(&handle, &["blog", "shop"]).await;
    let text = std::fs::read_to_string(base.join(".config/devports/config.yml")).expect("read");
    assert!(text.starts_with("# mine\nversion: 1\n") && text.contains("host_interval: 250ms"));

    // A stale version is a conflict and keeps what is there.
    let conflict = handle
        .set_scan_folders(vec![more.display().to_string()], seen.base)
        .await;
    assert!(matches!(
        conflict,
        Err(SettingsError::DocumentChanged { .. })
    ));
    assert_eq!(
        std::fs::read_to_string(base.join(".config/devports/config.yml")).expect("read"),
        text
    );

    // Removing a folder stops listing its projects, without touching their files.
    let fresh = handle.scan_folders().await.expect("folders");
    handle
        .set_scan_folders(vec![more.display().to_string()], fresh.base)
        .await
        .expect("removed");
    projects(&handle, &["blog"]).await;
    assert!(work.join("shop/.devports.yml").exists());
    handle.shutdown();
    core.wait().await;
}
