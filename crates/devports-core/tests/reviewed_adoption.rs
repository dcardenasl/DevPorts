//! Reviewed adoption through the running actor, with disposable files and a fake host.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use devports_core::adopt::review::{AdoptionError, AdoptionTarget, ServiceEdits};
use devports_core::process::fake::FakeProcessSource;
use devports_core::process::{ListeningProcess, ProcessId, ProcessInfo, ProcessStart};
use devports_core::runtime::{Core, CoreHandle, CoreOptions};
use tempfile::TempDir;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    std::fs::write(path, text).expect("write");
}

async fn setup() -> (TempDir, Core, CoreHandle, String) {
    let home = TempDir::new().expect("home");
    let root = home.path().join("work/shop");
    write(
        &home.path().join(".config/devports/config.yml"),
        &format!(
            "version: 1\nroots: [{}]\nobservation:\n  host_interval: 250ms\n",
            home.path().join("work").display()
        ),
    );
    write(
        &root.join(".devports.yml"),
        "# keep this comment\nversion: 1\nproject: shop\nservices:\n  api:\n    cwd: ./api\n    port: 8080\n",
    );
    std::fs::create_dir_all(root.join("admin")).expect("cwd");
    let source = Arc::new(FakeProcessSource::new(vec![ListeningProcess {
        info: ProcessInfo {
            id: ProcessId {
                pid: 21,
                start: ProcessStart(1_000_000),
            },
            ppid: 1,
            pgid: 21,
            uid: 4_000_000,
            argv: vec!["php".to_owned(), "spark".to_owned(), "serve".to_owned()],
            cwd: Some(std::fs::canonicalize(root.join("admin")).expect("canonical")),
        },
        ports: vec![8081],
    }]));
    let core = Core::start(
        CoreOptions {
            home: home.path().to_path_buf(),
            database: home.path().join("db"),
            socket: home.path().join("s/d.sock"),
        },
        source,
    )
    .await
    .expect("core");
    let handle = core.handle();
    for _ in 0..100 {
        if let Some(view) = handle
            .snapshot()
            .instances
            .iter()
            .find(|view| view.pid == Some(21))
        {
            let reference = view.instance_ref.clone();
            return (home, core, handle, reference);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("fake listener was not observed");
}

// covers: UXR-3.1
#[tokio::test]
async fn parallel_and_lost_response_saves_keep_one_service_and_refresh_the_snapshot() {
    let (_home, core, handle, reference) = setup().await;
    let choices = handle.adoption_options(&reference).await.expect("options");
    assert!(
        choices
            .destinations
            .iter()
            .any(|destination| destination.eligible
                && destination.target == AdoptionTarget::Project("shop".to_owned()))
    );
    let preview = handle
        .preview_adoption(&reference, AdoptionTarget::Project("shop".to_owned()))
        .await
        .expect("preview");
    let before = std::fs::read_to_string(&preview.manifest).expect("before");
    let fields = ServiceEdits {
        name: "backend".to_owned(),
        start: Some("php spark serve --port ${PORT}".to_owned()),
    };
    let checked = handle
        .validate_adoption(&preview.id, fields.clone())
        .await
        .expect("validate");
    assert!(checked.yaml.contains("backend:"));
    assert_eq!(
        std::fs::read_to_string(&preview.manifest).expect("unchanged"),
        before
    );
    let (first, second) = tokio::join!(
        handle.apply_adoption(&preview.id, fields.clone()),
        handle.apply_adoption(&preview.id, fields.clone())
    );
    let applied = first.expect("first result");
    assert_eq!(applied, second.expect("coalesced result"));
    assert_eq!(
        handle
            .apply_adoption(&preview.id, fields.clone())
            .await
            .expect("lost-response replay"),
        applied
    );
    let loaded =
        devports_core::manifest::load(Path::new(&preview.manifest), None).expect("saved manifest");
    assert_eq!(loaded.manifest.services.len(), 2);
    assert!(loaded.manifest.services.contains_key("backend"));
    assert!(
        std::fs::read_to_string(&preview.manifest)
            .expect("text")
            .starts_with("# keep this comment\n")
    );
    for _ in 0..100 {
        if handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.pid == Some(21) && view.service.as_deref() == Some("backend"))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.pid == Some(21) && view.service.as_deref() == Some("backend"))
    );
    assert!(matches!(
        handle
            .apply_adoption(
                &preview.id,
                ServiceEdits {
                    name: "different".to_owned(),
                    ..fields
                }
            )
            .await,
        Err(AdoptionError::RequestChanged)
    ));
    handle.shutdown();
    core.wait().await;
}

// covers: UXR-3.1
#[tokio::test]
async fn dropping_the_response_consumer_does_not_repeat_or_cancel_an_accepted_save() {
    let (_home, core, handle, reference) = setup().await;
    let preview = handle
        .preview_adoption(&reference, AdoptionTarget::Project("shop".to_owned()))
        .await
        .expect("preview");
    let fields = ServiceEdits {
        name: "backend".to_owned(),
        start: preview.service.start.clone(),
    };
    let worker_handle = handle.clone();
    let id = preview.id.clone();
    let request = fields.clone();
    let caller = tokio::spawn(async move { worker_handle.apply_adoption(&id, request).await });
    // The actor refuses editing a running/completed review; this is an acceptance
    // acknowledgement, not a delay-based assumption about worker scheduling.
    let mut accepted = false;
    for _ in 0..100 {
        if matches!(
            handle.validate_adoption(&preview.id, fields.clone()).await,
            Err(AdoptionError::RequestChanged)
        ) {
            accepted = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(accepted);
    caller.abort();
    let _ = caller.await;
    let applied = handle
        .apply_adoption(&preview.id, fields)
        .await
        .expect("surviving operation or receipt");
    let loaded = devports_core::manifest::load(&applied.manifest, None).expect("saved");
    assert_eq!(loaded.manifest.services.len(), 2);
    assert!(loaded.manifest.services.contains_key("backend"));
    handle.shutdown();
    core.wait().await;
}

// covers: UXR-3.1
#[tokio::test]
async fn stale_file_conflicts_preserve_the_foreign_content_and_need_another_review() {
    let (_home, core, handle, reference) = setup().await;
    let preview = handle
        .preview_adoption(&reference, AdoptionTarget::Project("shop".to_owned()))
        .await
        .expect("preview");
    let fields = ServiceEdits {
        name: "backend".to_owned(),
        start: preview.service.start.clone(),
    };
    let foreign = format!(
        "{}# foreign editor\n",
        std::fs::read_to_string(&preview.manifest).expect("text")
    );
    std::fs::write(&preview.manifest, &foreign).expect("external change");
    assert!(matches!(
        handle.validate_adoption(&preview.id, fields.clone()).await,
        Err(AdoptionError::DocumentChanged { .. })
    ));
    assert!(matches!(
        handle.apply_adoption(&preview.id, fields.clone()).await,
        Err(AdoptionError::DocumentChanged { .. })
    ));
    assert!(matches!(
        handle.apply_adoption(&preview.id, fields).await,
        Err(AdoptionError::DocumentChanged { .. })
    ));
    assert_eq!(
        std::fs::read_to_string(&preview.manifest).expect("preserved"),
        foreign
    );
    handle.shutdown();
    core.wait().await;
}
