//! The CLI and the core against the real machine: real processes, real ports,
//! the real process table (`PRUEBAS.md`: I-01, I-02 and I-03). macOS only, like
//! the product. Every process these tests start is killed when they end, even
//! if an assertion fails.

#![cfg(target_os = "macos")]
#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use devports_core::runtime::{Core, CoreHandle, CoreOptions};
use devports_proto::views::{InstanceRole, InstanceState};
use tempfile::TempDir;

/// Kills what it holds when dropped.
#[derive(Default)]
struct Reaper(Vec<Child>);

impl Drop for Reaper {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct Host {
    _home: TempDir,
    socket: PathBuf,
    project: PathBuf,
    api_port: u16,
    core: Option<Core>,
    handle: CoreHandle,
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

async fn host() -> Host {
    let home = TempDir::new().expect("tempdir");
    let work = home.path().join("work");
    let project = work.join("shop");
    let api_port = free_port();
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
            "version: 1\nproject: shop\nservices:\n  api:\n    cwd: .\n    port: {api_port}\n    start: python3 -m http.server ${{PORT}} --bind 127.0.0.1\n    match: python3 -m http.server\n"
        ),
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
    for _ in 0..100 {
        if handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.service.as_deref() == Some("api"))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Host {
        project: std::fs::canonicalize(project).expect("canonical"),
        _home: home,
        socket,
        api_port,
        core: Some(core),
        handle,
    }
}

impl Host {
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_devports"));
        command
            .args(args)
            .current_dir(&self.project)
            .env("DEVPORTS_SOCKET", &self.socket)
            .env("DEVPORTS_NO_AUTOSTART", "1");
        command
    }

    async fn finish(mut self) {
        if let Some(core) = self.core.take() {
            self.handle.shutdown();
            core.wait().await;
        }
    }
}

async fn until(what: &str, seconds: u64, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out waiting for {what}");
}

fn port_is_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(200),
    )
    .is_ok()
}

// covers: I-01, RF-07
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn seven_agents_racing_for_one_service_leave_one_server_and_six_refusals() {
    let host = host().await;
    let mut reaper = Reaper::default();
    let children: Vec<Child> = (0..7)
        .map(|_| {
            host.command(&["run", "api"])
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("the CLI starts")
        })
        .collect();

    // Six of them are refused at once; the seventh became the server and stays.
    let mut refused = 0;
    let mut survivors = Vec::new();
    for mut child in children {
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().expect("wait") {
                assert_eq!(status.code(), Some(10), "a loser exits with code 10");
                refused += 1;
                break;
            }
            if started.elapsed() > Duration::from_secs(10)
                || port_is_open(host.api_port) && survivors.is_empty()
            {
                // Still running after the others answered: this is the server.
                survivors.push(child);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    reaper.0.extend(survivors);
    until("the server to listen", 15, || port_is_open(host.api_port)).await;
    // Give any late loser time to be refused.
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(
        reaper.0.len(),
        1,
        "exactly one server, got {refused} refusals"
    );
    assert_eq!(refused, 6);

    let snapshot = host.handle.snapshot();
    let principals: Vec<_> = snapshot
        .instances
        .iter()
        .filter(|view| {
            view.service.as_deref() == Some("api") && view.role == InstanceRole::Principal
        })
        .collect();
    assert_eq!(principals.len(), 1);
    assert!(
        snapshot
            .instances
            .iter()
            .all(|view| view.service.as_deref() != Some("api")
                || view.role != InstanceRole::Unregistered),
        "no duplicate was created"
    );
    drop(reaper);
    host.finish().await;
}

// covers: I-02, RF-10
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn seven_servers_started_by_hand_give_one_principal_and_six_duplicates() {
    let host = host().await;
    let mut reaper = Reaper::default();
    // The preferred port goes to the Principal; six more on other ports.
    let mut ports = vec![host.api_port];
    ports.extend((0..6).map(|_| free_port()));
    for port in &ports {
        reaper.0.push(
            Command::new("python3")
                .args([
                    "-m",
                    "http.server",
                    &port.to_string(),
                    "--bind",
                    "127.0.0.1",
                ])
                .current_dir(&host.project)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("python starts"),
        );
    }
    let started = Instant::now();
    until("seven servers to be classified", 20, || {
        let snapshot = host.handle.snapshot();
        let of_api = |role| {
            snapshot
                .instances
                .iter()
                .filter(|view| {
                    view.service.as_deref() == Some("api")
                        && view.role == role
                        && view.state != InstanceState::Down
                })
                .count()
        };
        of_api(InstanceRole::Principal) == 1 && of_api(InstanceRole::Unregistered) == 6
    })
    .await;
    // The detection itself takes a couple of scans (250 ms each in this test).
    assert!(started.elapsed() < Duration::from_secs(10));

    let snapshot = host.handle.snapshot();
    let principal = snapshot
        .instances
        .iter()
        .find(|view| view.service.as_deref() == Some("api") && view.role == InstanceRole::Principal)
        .expect("a principal");
    assert_eq!(
        principal.port,
        Some(host.api_port),
        "the preferred port wins"
    );
    assert_eq!(snapshot.attention.len(), 6);
    drop(reaper);
    host.finish().await;
}

// covers: I-03, RF-09
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_temporal_with_a_short_lease_is_stopped_and_frees_its_port() {
    let host = host().await;
    let output = tokio::task::spawn_blocking({
        let mut command = host.command(&[
            "run",
            "api",
            "--as",
            "test",
            "--reason",
            "short lease",
            "--ttl",
            "4s",
            "--",
            "python3",
            "-m",
            "http.server",
            "{PORT}",
            "--bind",
            "127.0.0.1",
        ]);
        // The CLI becomes the server, so it is started, not waited for.
        move || {
            command
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("starts")
        }
    })
    .await
    .expect("join");
    let mut reaper = Reaper(vec![output]);

    let temporal_port = {
        let mut port = None;
        until("the temporal to be registered", 10, || {
            port = host
                .handle
                .snapshot()
                .instances
                .iter()
                .find(|view| view.role == InstanceRole::Temporal)
                .and_then(|view| view.port);
            port.is_some()
        })
        .await;
        port.expect("a port")
    };
    until("the server to listen", 10, || port_is_open(temporal_port)).await;

    // The lease ends, the process is stopped and the port is free again.
    until("the lease to end", 30, || !port_is_open(temporal_port)).await;
    let status = reaper.0[0].wait().expect("wait");
    assert!(!status.success(), "it was ended by a signal: {status:?}");
    until("the instance to leave the list", 10, || {
        !host
            .handle
            .snapshot()
            .instances
            .iter()
            .any(|view| view.role == InstanceRole::Temporal)
    })
    .await;
    // The record of the end is written when the stop is reported, which can
    // come a moment after the scan noticed the process was gone.
    let mut log_text = String::new();
    until("the end of the lease to be logged", 10, || {
        let log = host.command(&["log"]).output().expect("log");
        log_text = String::from_utf8_lossy(&log.stdout).into_owned();
        log_text.contains("venció")
    })
    .await;
    assert!(log_text.contains("short lease"), "{log_text}");
    drop(reaper);
    host.finish().await;
}
