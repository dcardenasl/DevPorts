//! The whole product against a real Docker daemon (`PRUEBAS.md`: D-02 and D-04).
//!
//! Needs a running daemon and the `ubuntu:24.04` image, and creates containers
//! named `devports-test-*` that it always force-removes. GitHub's macOS runners
//! have no Docker, so these are `#[ignore]`d and run by `tools/docker-checks.sh`
//! on the maintainer's Mac.

#![cfg(target_os = "macos")]
#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use devports_core::runtime::{Core, CoreOptions};
use devports_proto::views::{InstanceRole, InstanceState};
use tempfile::TempDir;

const IMAGE: &str = "ubuntu:24.04";

/// Removes the named containers when dropped.
struct Containers(Vec<String>);

impl Drop for Containers {
    fn drop(&mut self) {
        for name in &self.0 {
            let _ = Command::new("docker")
                .args(["rm", "-f", name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn docker(args: &[&str]) -> String {
    let output = Command::new("docker")
        .args(args)
        .output()
        .expect("docker runs");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

fn suffix() -> String {
    std::process::id().to_string()
}

async fn until(what: &str, seconds: u64, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("timed out waiting for {what}");
}

// covers: D-02, RF-03, RF-09
#[ignore = "needs a Docker daemon; run tools/docker-checks.sh"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_container_temporal_is_stopped_and_never_removed() {
    let name = format!("devports-test-temporal-{}", suffix());
    let _cleanup = Containers(vec![name.clone()]);
    let home = TempDir::new().expect("tempdir");
    let work = home.path().join("work");
    let project = work.join("shop");
    write(
        &home.path().join(".config/devports/config.yml"),
        &format!("version: 1\nroots: [{}]\n", work.display()),
    );
    write(
        &project.join(".devports.yml"),
        "version: 1\nproject: shop\nservices:\n  db:\n    cwd: .\n    start: echo unused\n",
    );
    let socket = home.path().join("s").join("d.sock");
    let core = Core::start_system(CoreOptions {
        home: home.path().to_path_buf(),
        database: home.path().join("d").join("devports.db"),
        socket: socket.clone(),
    })
    .await
    .expect("the core starts");
    let handle = core.handle();
    until("Docker to answer", 20, || {
        handle.snapshot().docker_reachable
    })
    .await;

    let status = Command::new(env!("CARGO_BIN_EXE_devports"))
        .args([
            "run",
            "db",
            "--as",
            "test",
            "--reason",
            "container lease",
            "--ttl",
            "5s",
            "--",
            "docker",
            "run",
            "-d",
            "--name",
            &name,
            "-p",
            "{PORT}:8080",
            IMAGE,
            "sleep",
            "300",
        ])
        .current_dir(std::fs::canonicalize(&project).expect("canonical"))
        .env("DEVPORTS_SOCKET", &socket)
        .env("DEVPORTS_NO_AUTOSTART", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("the CLI runs");
    assert!(status.success());

    until("the container to be bound", 15, || {
        handle.snapshot().instances.iter().any(|view| {
            view.role == InstanceRole::Temporal && view.container.as_deref() == Some(name.as_str())
        })
    })
    .await;
    assert_eq!(
        docker(&["inspect", "-f", "{{.State.Running}}", &name]),
        "true"
    );

    until("the lease to end", 30, || {
        docker(&["inspect", "-f", "{{.State.Running}}", &name]) == "false"
    })
    .await;
    // Stopped, and still there with everything it had: nothing was removed.
    assert_eq!(
        docker(&["inspect", "-f", "{{.Name}}", &name]),
        format!("/{name}")
    );
    until("the instance to leave the list", 10, || {
        !handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.role == InstanceRole::Temporal)
    })
    .await;
    handle.shutdown();
    core.wait().await;
}

// covers: D-04, RF-09
#[ignore = "needs a Docker daemon; run tools/docker-checks.sh"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_data_container_is_listed_as_data_and_left_running() {
    let name = format!("devports-test-data-{}", suffix());
    let _cleanup = Containers(vec![name.clone()]);
    let port = free_port();
    assert!(
        Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                &name,
                "-p",
                &format!("{port}:3306"),
                IMAGE,
                "sleep",
                "300"
            ])
            .stdout(Stdio::null())
            .status()
            .expect("docker runs")
            .success()
    );
    let home = TempDir::new().expect("tempdir");
    write(
        &home.path().join(".config/devports/shared.yml"),
        &format!(
            "version: 1\nservices:\n  db:\n    docker: {name}\n    kind: data\n    port: {port}\n    start: docker start {name}\n"
        ),
    );
    write(
        &home.path().join(".config/devports/config.yml"),
        "version: 1\nroots: []\n",
    );
    let core = Core::start_system(CoreOptions {
        home: home.path().to_path_buf(),
        database: home.path().join("d").join("devports.db"),
        socket: home.path().join("s").join("d.sock"),
    })
    .await
    .expect("the core starts");
    let handle = core.handle();
    until("the data service to be seen", 20, || {
        handle.snapshot().instances.iter().any(|view| {
            view.service.as_deref() == Some("db")
                && view.kind == "data"
                && view.state == InstanceState::Healthy
        })
    })
    .await;
    // Past several ticks and scans: still running, no lease on it.
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_eq!(
        docker(&["inspect", "-f", "{{.State.Running}}", &name]),
        "true"
    );
    let snapshot = handle.snapshot();
    let view = snapshot
        .instances
        .iter()
        .find(|view| view.service.as_deref() == Some("db"))
        .expect("listed");
    assert!(view.expires_at.is_none());
    handle.shutdown();
    core.wait().await;
}
