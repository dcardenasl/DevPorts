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
        "version: 1\nproject: shop\nservices:\n  api:\n    cwd: .\n    port: 8080\n    start: serve --port ${PORT}\n",
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
    assert_eq!(accepted.port, 8080);

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
            argv: vec!["serve".to_owned(), "--port".to_owned(), "8080".to_owned()],
            cwd: Some(project.clone()),
        },
        ports: vec![8080],
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
