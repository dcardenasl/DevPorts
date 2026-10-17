//! The running core end to end, with a fake host: a `run` over the socket
//! protocol is accepted, bound when its process appears, listed, and refused
//! when repeated.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::disallowed_methods
)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use devports_core::process::fake::FakeProcessSource;
use devports_core::process::{ListeningProcess, ProcessId, ProcessInfo, ProcessStart};
use devports_core::runtime::{Core, CoreHandle, CoreOptions};
use devports_proto::run::{LauncherInfo, RunOutcome, RunRegister, RunRole};
use devports_proto::views::InstanceState;
use devports_proto::{Op, Request};
use tempfile::TempDir;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

fn options(home: &Path) -> CoreOptions {
    CoreOptions {
        home: home.to_path_buf(),
        database: home.join("data").join("devports.db"),
        socket: home.join("s").join("d.sock"),
    }
}

fn register(service: &str, cwd: &Path) -> RunRegister {
    RunRegister {
        project: None,
        service: service.to_owned(),
        cwd: cwd.display().to_string(),
        role: RunRole::Principal,
        reason: None,
        ttl_seconds: None,
        reuse: false,
        command: None,
        launcher: LauncherInfo {
            pid: 4242,
            chain: Vec::new(),
        },
    }
}

async fn outcome(handle: &CoreHandle, register: RunRegister) -> RunOutcome {
    let response = handle
        .request(Request::new("t", Op::RunRegister(register)))
        .await;
    response.into_result::<RunOutcome>().expect("a run outcome")
}

async fn wait_for(handle: &CoreHandle, what: &str, done: impl Fn(&CoreHandle) -> bool) {
    for _ in 0..100 {
        if done(handle) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out waiting for {what}");
}

#[tokio::test]
async fn a_run_is_accepted_bound_listed_and_refused_when_repeated() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let home = TempDir::new().expect("tempdir");
    let work = home.path().join("work");
    let project = work.join("shop");
    write(
        &home.path().join(".config/devports/config.yml"),
        &format!(
            "version: 1\nroots: [{}]\nobservation:\n  host_interval: 250ms\n",
            work.display()
        ),
    );
    write(
        &project.join(".devports.yml"),
        &format!(
            "version: 1\nproject: shop\nservices:\n  api:\n    cwd: .\n    port: {port}\n    start: serve --port ${{PORT}}\n"
        ),
    );
    let project = std::fs::canonicalize(&project).expect("canonical");

    let source = Arc::new(FakeProcessSource::new(Vec::new()));
    let core = Core::start(options(home.path()), Arc::clone(&source))
        .await
        .expect("the core starts");
    let handle = core.handle();

    // The manifest is found by the first catalog pass.
    wait_for(&handle, "the declared service", |handle| {
        handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.service.as_deref() == Some("api"))
    })
    .await;

    let first = outcome(&handle, register("api", &project)).await;
    let RunOutcome::Accepted(accepted) = first else {
        panic!("expected an accepted run, got {first:?}");
    };
    assert_eq!(accepted.port, port);

    // The server shows up as a child of the launcher on the reserved port.
    let started = jiff::Timestamp::now().as_second() + 1;
    source.set(vec![ListeningProcess {
        info: ProcessInfo {
            id: ProcessId {
                pid: 4243,
                start: ProcessStart(u64::try_from(started).expect("positive") * 1_000_000),
            },
            ppid: 1,
            pgid: 4243,
            uid: 4_000_000, // never the real user: a stop in a test must be refused, not delivered
            argv: vec!["serve".to_owned(), "--port".to_owned(), port.to_string()],
            cwd: Some(project.clone()),
        },
        ports: vec![port],
    }]);
    wait_for(&handle, "the binding", |handle| {
        handle.snapshot().instances.iter().any(|view| {
            view.instance_ref == accepted.instance_ref && view.state != InstanceState::Starting
        })
    })
    .await;

    let second = outcome(&handle, register("api", &project)).await;
    assert!(matches!(second, RunOutcome::Rejected(_)), "{second:?}");

    handle.shutdown();
    core.wait().await;
}

#[tokio::test]
async fn a_second_core_on_the_same_socket_is_refused() {
    let home = TempDir::new().expect("tempdir");
    write(
        &home.path().join(".config/devports/config.yml"),
        "version: 1\nroots: []\n",
    );
    let first = Core::start(options(home.path()), Arc::new(FakeProcessSource::default()))
        .await
        .expect("the first core starts");
    let second = Core::start(options(home.path()), Arc::new(FakeProcessSource::default())).await;
    assert!(second.is_err(), "a second core must not start");
    first.handle().shutdown();
    first.wait().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "one scenario told from start to finish"
)]
async fn a_duplicate_asks_for_attention_until_the_person_keeps_it_and_adoption_writes_the_manifest()
{
    use devports_core::runtime::AdoptTarget;
    use devports_proto::views::AttentionKind;

    let home = TempDir::new().expect("tempdir");
    let work = home.path().join("work");
    let project = work.join("shop");
    write(
        &home.path().join(".config/devports/config.yml"),
        &format!(
            "version: 1\nroots: [{}]\nobservation:\n  host_interval: 250ms\n",
            work.display()
        ),
    );
    write(
        &project.join(".devports.yml"),
        "version: 1\nproject: shop\nservices:\n  api:\n    cwd: .\n    port: 8080\n",
    );
    let project = std::fs::canonicalize(&project).expect("canonical");
    let source = Arc::new(FakeProcessSource::new(Vec::new()));
    let core = Core::start(options(home.path()), Arc::clone(&source))
        .await
        .expect("the core starts");
    let handle = core.handle();
    wait_for(&handle, "the declared service", |handle| {
        handle
            .snapshot()
            .projects
            .iter()
            .any(|summary| summary.name == "shop")
    })
    .await;

    let listening = |pid: u32, port: u16, start: i64| ListeningProcess {
        info: ProcessInfo {
            id: ProcessId {
                pid,
                start: ProcessStart(u64::try_from(start).expect("positive") * 1_000_000),
            },
            ppid: 1,
            pgid: pid,
            uid: 4_000_000,
            argv: vec!["php".to_owned(), "spark".to_owned(), "serve".to_owned()],
            cwd: Some(project.clone()),
        },
        ports: vec![port],
    };
    let long_ago = jiff::Timestamp::now().as_second() - 3_600;
    source.set(vec![
        listening(11, 8080, long_ago),
        listening(12, 8090, long_ago + 60),
    ]);
    wait_for(&handle, "the duplicate to be flagged", |handle| {
        handle
            .snapshot()
            .attention
            .iter()
            .any(|item| item.kind == AttentionKind::Duplicate)
    })
    .await;
    let duplicate = handle
        .snapshot()
        .attention
        .iter()
        .find(|item| item.kind == AttentionKind::Duplicate)
        .expect("a duplicate")
        .instance_ref
        .clone();

    handle.ignore(&duplicate).await.expect("kept");
    wait_for(&handle, "the attention to clear", |handle| {
        handle.snapshot().attention.is_empty()
    })
    .await;

    // A process in a new folder is adopted as a project of its own.
    let fresh = home.path().join("work").join("fresh");
    std::fs::create_dir_all(&fresh).expect("mkdir");
    let fresh = std::fs::canonicalize(&fresh).expect("canonical");
    let mut stray = listening(21, 9100, long_ago);
    stray.info.cwd = Some(fresh.clone());
    stray.info.argv = vec!["node".to_owned(), "server.js".to_owned()];
    source.set(vec![stray]);
    wait_for(&handle, "the stray process", |handle| {
        handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.pid == Some(21))
    })
    .await;
    let reference = handle
        .snapshot()
        .instances
        .iter()
        .find(|view| view.pid == Some(21))
        .expect("listed")
        .instance_ref
        .clone();
    let applied = handle
        .adopt(&reference, AdoptTarget::New(Some("fresh-app".to_owned())))
        .await
        .expect("adopted");
    assert_eq!(applied.project, "fresh-app");
    assert!(fresh.join(".devports.yml").exists());
    wait_for(&handle, "the new project", |handle| {
        handle
            .snapshot()
            .projects
            .iter()
            .any(|summary| summary.name == "fresh-app")
    })
    .await;

    handle.shutdown();
    core.wait().await;
}

// covers: UXR-2.2
#[tokio::test]
async fn a_manual_stop_reports_termination_refusal_instead_of_acceptance_as_success() {
    use devports_core::engine::StopExpectation;
    let home = TempDir::new().expect("home");
    let project = home.path().join("work/shop");
    write(
        &home.path().join(".config/devports/config.yml"),
        &format!(
            "version: 1\nroots: [{}]\nobservation:\n  host_interval: 250ms\n",
            home.path().join("work").display()
        ),
    );
    write(
        &project.join(".devports.yml"),
        "version: 1\nproject: shop\nservices:\n  api:\n    cwd: .\n",
    );
    let source = Arc::new(FakeProcessSource::new(Vec::new()));
    let own = std::process::id();
    source.set(vec![ListeningProcess {
        info: ProcessInfo {
            id: ProcessId {
                pid: own,
                start: ProcessStart(1_000_000),
            },
            ppid: 1,
            pgid: own,
            uid: rustix::process::getuid().as_raw(),
            argv: vec!["controlled-listener".to_owned()],
            cwd: Some(std::fs::canonicalize(&project).expect("canonical")),
        },
        ports: vec![12345],
    }]);
    let core = Core::start(options(home.path()), source)
        .await
        .expect("core");
    let handle = core.handle();
    wait_for(&handle, "protected target", |handle| {
        handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.pid == Some(own))
    })
    .await;
    let preview = handle.preview_project_stop("shop").await.expect("preview");
    let view = preview.first().expect("target");
    let result = handle
        .stop_project(&view.instance_ref, StopExpectation::from_view(view))
        .await;
    assert!(
        result
            .expect_err("protected target cannot stop")
            .to_string()
            .contains("protegido")
    );
    handle.shutdown();
    core.wait().await;
}
