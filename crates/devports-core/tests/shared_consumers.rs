//! Declared consumers of a shared service through the running actor, with disposable files.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use devports_core::process::fake::FakeProcessSource;
use devports_core::runtime::{Core, CoreHandle, CoreOptions};
use devports_core::uses::{UseOutcome, UseRequest, UsesError};
use tempfile::TempDir;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    std::fs::write(path, text).expect("write");
}

fn manifest(name: &str, uses: &str) -> String {
    format!(
        "# {name}\nversion: 1\nproject: {name}\n{uses}services:\n  api:\n    cwd: .\n    port: 8080\n"
    )
}

async fn setup() -> (TempDir, Core, CoreHandle) {
    let home = TempDir::new().expect("home");
    let work = home.path().join("work");
    write(
        &home.path().join(".config/devports/config.yml"),
        &format!(
            "version: 1\nroots: [{}]\nobservation:\n  host_interval: 250ms\n",
            work.display()
        ),
    );
    write(
        &home.path().join(".config/devports/shared.yml"),
        "version: 1\nservices:\n  mysql:\n    docker: mysql\n    kind: data\n",
    );
    write(&work.join("shop/.devports.yml"), &manifest("shop", ""));
    write(
        &work.join("blog/.devports.yml"),
        &manifest("blog", "uses: [mysql]\n"),
    );
    write(&work.join("docs/.devports.yml"), &manifest("docs", ""));
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
    for _ in 0..100 {
        if handle.snapshot().projects.len() == 3 {
            return (home, core, handle);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("projects were not discovered");
}

async fn uses_of(handle: &CoreHandle, project: &str) -> Vec<String> {
    for _ in 0..100 {
        if let Some(found) = handle
            .snapshot()
            .projects
            .iter()
            .find(|candidate| candidate.name == project)
        {
            return found.uses.clone();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("project missing");
}

async fn wait_for_uses(handle: &CoreHandle, project: &str, expected: &[&str]) {
    for _ in 0..100 {
        if uses_of(handle, project).await == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the snapshot never showed {expected:?} for {project}");
}

// covers: UXR-4.1
#[tokio::test]
async fn associating_projects_updates_the_files_the_catalog_and_repeats_safely() {
    let (home, core, handle) = setup().await;
    let options = handle.shared_consumers("mysql").await.expect("options");
    assert!(options.defined);
    let names: Vec<_> = options
        .consumers
        .iter()
        .map(|c| c.project.as_str())
        .collect();
    assert_eq!(names, ["blog", "docs", "shop"]);
    assert!(options.consumers[0].declared && !options.consumers[1].declared);
    assert!(options.consumers.iter().all(|c| c.eligible));
    let request = |name: &str, include: bool| UseRequest {
        project: name.to_owned(),
        include,
        base: options
            .consumers
            .iter()
            .find(|c| c.project == name)
            .and_then(|c| c.base.clone())
            .expect("base"),
    };
    let requests = vec![
        request("shop", true),
        request("docs", true),
        request("blog", false),
    ];
    let reports = handle
        .set_shared_consumers("mysql", requests.clone())
        .await
        .expect("saved");
    assert!(reports.iter().all(|r| r.result == Ok(UseOutcome::Saved)));
    let shop = std::fs::read_to_string(home.path().join("work/shop/.devports.yml")).expect("shop");
    assert!(shop.starts_with("# shop\n") && shop.contains("uses: [mysql]"));
    let blog = std::fs::read_to_string(home.path().join("work/blog/.devports.yml")).expect("blog");
    assert!(blog.contains("uses: []"));
    wait_for_uses(&handle, "shop", &["mysql"]).await;
    wait_for_uses(&handle, "docs", &["mysql"]).await;
    wait_for_uses(&handle, "blog", &[]).await;
    // A lost response is retried with the same request: nothing changes again.
    let again = handle
        .set_shared_consumers("mysql", requests)
        .await
        .expect("replay");
    assert!(again.iter().all(|r| r.result == Ok(UseOutcome::Unchanged)));
    assert_eq!(
        std::fs::read_to_string(home.path().join("work/shop/.devports.yml")).expect("shop"),
        shop
    );
    // Zero consumers is a valid state, and an unknown resource is refused.
    assert!(
        handle
            .set_shared_consumers("mysql", Vec::new())
            .await
            .expect("empty")
            .is_empty()
    );
    let refused = handle
        .set_shared_consumers("redis", vec![request("shop", true)])
        .await
        .expect("reports");
    assert!(matches!(
        refused[0].result,
        Err(UsesError::UnknownShared { .. })
    ));
    handle.shutdown();
    core.wait().await;
}
