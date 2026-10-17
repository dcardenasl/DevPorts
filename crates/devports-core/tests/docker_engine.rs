//! The Docker source against a real daemon (`PRUEBAS.md`: D-01, D-03).
//!
//! These tests need a running Docker daemon and the `ubuntu:24.04` image, and
//! they create and remove one container. GitHub's macOS runners have no Docker,
//! so they are `#[ignore]`d and run by hand on the maintainer's Mac:
//!
//! ```text
//! cargo test -p devports-core --test docker_engine -- --ignored
//! ```

// The test drives the `docker` CLI to create and destroy its container, and
// waits for events; helpers are not covered by `clippy.toml`'s allow switches.
#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::process::{Command, Stdio};
use std::time::Duration;

use devports_core::docker::context::Endpoint;
use devports_core::docker::engine::EngineContainerSource;
use devports_core::docker::{ContainerEventKind, ContainerSource, ContainerState};
use futures_util::StreamExt;

/// Events must arrive within this long (the product promises 3 seconds).
const EVENT_DEADLINE: Duration = Duration::from_secs(3);

/// A container that is force-removed when the guard goes out of scope.
struct TestContainer(String);

impl Drop for TestContainer {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.0])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn docker(args: &[&str]) -> String {
    let output = Command::new("docker")
        .args(args)
        .output()
        .expect("the docker CLI is installed");
    assert!(
        output.status.success(),
        "docker {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("address")
        .port()
}

#[tokio::test]
#[ignore = "needs a Docker daemon and the ubuntu:24.04 image"]
async fn lists_a_compose_labelled_container_and_follows_its_lifecycle() {
    let source = EngineContainerSource::connect()
        .await
        .expect("connect to the active Docker context");
    assert!(matches!(source.context().endpoint, Endpoint::Unix(_)));

    let name = format!("devports-test-{}", std::process::id());
    let port = free_port();
    let mut events = Box::pin(source.events());

    // Created paused-by-nothing: `docker run -d` starts it, which emits `start`.
    let id = docker(&[
        "run",
        "-d",
        "--name",
        &name,
        "--label",
        "com.docker.compose.project=devports-test",
        "--label",
        "com.docker.compose.project.working_dir=/tmp/devports-test",
        "-p",
        &format!("127.0.0.1:{port}:80/tcp"),
        "ubuntu:24.04",
        "sleep",
        "300",
    ]);
    let _guard = TestContainer(name.clone());

    // D-01: the labels, the published port and the state come through.
    let listed = source.list().await.expect("list");
    let ours = listed
        .iter()
        .find(|container| container.id == id)
        .expect("the new container is listed");
    assert_eq!(ours.name, name);
    assert_eq!(ours.state, ContainerState::Running);
    assert_eq!(ours.ports.len(), 1);
    assert_eq!(ours.ports[0].host_port, port);
    assert_eq!(ours.ports[0].container_port, 80);
    let compose = ours.compose.as_ref().expect("compose labels");
    assert_eq!(compose.project, "devports-test");
    assert_eq!(
        compose.working_dir.as_deref(),
        Some(std::path::Path::new("/tmp/devports-test"))
    );

    // D-03: start was emitted before we subscribed to the stream's first poll,
    // so assert the transitions we cause from here on.
    docker(&["stop", "-t", "1", &name]);
    let mut seen_stop = false;
    let mut seen_die = false;
    while !(seen_stop && seen_die) {
        let event = tokio::time::timeout(EVENT_DEADLINE, events.next())
            .await
            .expect("an event within 3 seconds")
            .expect("the stream stays open")
            .expect("a well-formed event");
        if event.container_id != id {
            continue;
        }
        match event.kind {
            ContainerEventKind::Stop => seen_stop = true,
            ContainerEventKind::Die => seen_die = true,
            _ => {}
        }
    }
    let stopped = source.list().await.expect("list");
    let stopped = stopped
        .iter()
        .find(|container| container.id == id)
        .expect("a stopped container is still listed");
    assert_eq!(stopped.state, ContainerState::Exited);

    docker(&["start", &name]);
    loop {
        let event = tokio::time::timeout(EVENT_DEADLINE, events.next())
            .await
            .expect("a start event within 3 seconds")
            .expect("the stream stays open")
            .expect("a well-formed event");
        if event.container_id == id && event.kind == ContainerEventKind::Start {
            assert_eq!(event.name.as_deref(), Some(name.as_str()));
            break;
        }
    }

    docker(&["rm", "-f", &name]);
    loop {
        let event = tokio::time::timeout(EVENT_DEADLINE, events.next())
            .await
            .expect("a destroy event within 3 seconds")
            .expect("the stream stays open")
            .expect("a well-formed event");
        if event.container_id == id && event.kind == ContainerEventKind::Destroy {
            break;
        }
    }
    assert!(
        source
            .list()
            .await
            .expect("list")
            .iter()
            .all(|container| container.id != id)
    );
}
