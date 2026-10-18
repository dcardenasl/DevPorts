//! Ignoring, listing and restoring through the running actor, with a fake host.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;
use std::time::Duration;

use devports_core::process::fake::FakeProcessSource;
use devports_core::process::{ListeningProcess, ProcessId, ProcessInfo, ProcessStart};
use devports_core::runtime::{Core, CoreHandle, CoreOptions};
use tempfile::TempDir;

async fn start(home: &TempDir, source: Arc<FakeProcessSource>) -> (Core, CoreHandle) {
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
    (core, handle)
}

fn listener(pid: u32, port: u16, cwd: &std::path::Path) -> ListeningProcess {
    ListeningProcess {
        info: ProcessInfo {
            id: ProcessId {
                pid,
                start: ProcessStart(1_000_000 + u64::from(pid)),
            },
            ppid: 1,
            pgid: pid,
            uid: 4_000_000,
            argv: vec!["node".to_owned(), "server.js".to_owned()],
            cwd: Some(cwd.to_path_buf()),
        },
        ports: vec![port],
    }
}

async fn wait_for(handle: &CoreHandle, ignored: bool) -> String {
    for _ in 0..100 {
        if let Some(view) = handle
            .snapshot()
            .instances
            .iter()
            .find(|view| view.argv.first().map(String::as_str) == Some("node"))
            && view.ignored == ignored
        {
            return view.instance_ref.clone();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the process never reached ignored = {ignored}");
}

// covers: UXR-5.1
#[tokio::test]
async fn ignore_survives_a_restart_and_restore_brings_the_process_back() {
    let home = TempDir::new().expect("home");
    let scratch = home.path().join("scratch");
    std::fs::create_dir_all(&scratch).expect("folder");
    let scratch = std::fs::canonicalize(scratch).expect("canonical");
    let source = Arc::new(FakeProcessSource::new(vec![listener(31, 5200, &scratch)]));
    let (core, handle) = start(&home, Arc::clone(&source)).await;
    let reference = wait_for(&handle, false).await;
    let preview = handle.ignore_preview(&reference).await.expect("preview");
    assert_eq!(preview.command, ["node", "server.js"]);
    assert!(handle.ignored_rules().await.expect("rules").is_empty());
    handle.ignore(&reference).await.expect("ignore");
    wait_for(&handle, true).await;
    handle.shutdown();
    core.wait().await;

    // A new core, a new process generation and another port: still ignored.
    let source = Arc::new(FakeProcessSource::new(vec![listener(32, 5201, &scratch)]));
    let (core, handle) = start(&home, source).await;
    wait_for(&handle, true).await;
    let rules = handle.ignored_rules().await.expect("rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].folder.as_deref(), scratch.to_str());
    handle.restore_ignored(&rules[0].id).await.expect("restore");
    wait_for(&handle, false).await;
    assert!(handle.ignored_rules().await.expect("rules").is_empty());
    handle
        .restore_ignored(&rules[0].id)
        .await
        .expect("restoring twice is harmless");
    handle.shutdown();
    core.wait().await;
}
