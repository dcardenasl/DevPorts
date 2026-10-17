//! The macOS process source against real processes (`PRUEBAS.md`: I-02, partial).
//!
//! Each test launches a real listener, waits until the kernel reports it and
//! checks what the source says about it. Children are killed when their guard is
//! dropped, even if an assertion fails.

#![cfg(target_os = "macos")]
// Helpers outside `#[test]` functions are not covered by `clippy.toml`'s
// `allow-*-in-tests`; spawning and sleeping are the whole point of this file.
#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use devports_core::platform::macos::MacProcessSource;
use devports_core::process::{ListeningProcess, ProcessSource};

/// A spawned child that is killed and reaped when it goes out of scope.
struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A TCP port that was free a moment ago.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .expect("local address")
        .port()
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("devports-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir.canonicalize().expect("canonicalize scratch dir")
}

fn spawn_python(cwd: &Path, args: &[&str]) -> Guard {
    Guard(
        Command::new("python3")
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("python3 is installed"),
    )
}

/// Polls the source until `predicate` matches a process, or fails after 10 s.
fn wait_for(
    source: MacProcessSource,
    predicate: impl Fn(&ListeningProcess) -> bool,
) -> ListeningProcess {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(10))
        .expect("deadline fits");
    loop {
        if let Some(found) = source
            .snapshot()
            .expect("snapshot")
            .into_iter()
            .find(&predicate)
        {
            return found;
        }
        assert!(Instant::now() < deadline, "the listener never showed up");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn finds_a_listening_server_with_its_port_cwd_and_command() {
    let source = MacProcessSource::new().expect("source");
    let dir = scratch_dir("http");
    let port = free_port();
    let port_text = port.to_string();
    let server = spawn_python(
        &dir,
        &["-m", "http.server", &port_text, "--bind", "127.0.0.1"],
    );

    let found = wait_for(source, |process| process.ports.contains(&port));

    assert_eq!(found.info.id.pid, server.0.id());
    assert_eq!(found.ports, vec![port]);
    assert_eq!(found.info.cwd.as_deref(), Some(dir.as_path()));
    assert!(
        found.info.argv.iter().any(|part| part == "http.server"),
        "argv was {:?}",
        found.info.argv
    );
    assert!(found.info.argv.iter().any(|part| part == &port_text));
    assert_eq!(found.info.ppid, std::process::id());
    assert!(found.info.id.start.0 > 0);
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

#[test]
fn reports_every_port_of_a_process_that_listens_on_several() {
    let source = MacProcessSource::new().expect("source");
    let dir = scratch_dir("multi");
    let (first, second) = (free_port(), free_port());
    let script = format!(
        "import socket, time\n\
         held = []\n\
         for port in ({first}, {second}):\n\
         \x20   s = socket.socket()\n\
         \x20   s.bind(('127.0.0.1', port))\n\
         \x20   s.listen()\n\
         \x20   held.append(s)\n\
         time.sleep(300)\n"
    );
    let _server = spawn_python(&dir, &["-c", &script]);

    let found = wait_for(source, |process| {
        process.ports.contains(&first) && process.ports.contains(&second)
    });

    let mut expected = vec![first, second];
    expected.sort_unstable();
    assert_eq!(found.ports, expected);
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

#[test]
fn a_stopped_server_leaves_the_snapshot() {
    let source = MacProcessSource::new().expect("source");
    let dir = scratch_dir("gone");
    let port = free_port();
    let port_text = port.to_string();
    let mut server = spawn_python(
        &dir,
        &["-m", "http.server", &port_text, "--bind", "127.0.0.1"],
    );
    wait_for(source, |process| process.ports.contains(&port));

    server.0.kill().expect("kill the server");
    server.0.wait().expect("reap the server");

    let snapshot = source.snapshot().expect("snapshot");
    assert!(
        snapshot
            .iter()
            .all(|process| !process.ports.contains(&port))
    );
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

#[test]
fn a_process_that_only_connects_is_not_a_listener() {
    let source = MacProcessSource::new().expect("source");
    let dir = scratch_dir("client");
    // The test itself listens; the child only connects, so it must not appear.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    let script = format!(
        "import socket, time\n\
         s = socket.create_connection(('127.0.0.1', {port}))\n\
         time.sleep(300)\n"
    );
    let client = spawn_python(&dir, &["-c", &script]);
    let (_accepted, _) = listener.accept().expect("the client connects");

    let snapshot = source.snapshot().expect("snapshot");
    assert!(
        snapshot
            .iter()
            .all(|process| process.info.id.pid != client.0.id())
    );
    assert!(
        snapshot
            .iter()
            .any(|process| process.info.id.pid == std::process::id()
                && process.ports.contains(&port))
    );
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

#[test]
fn describes_an_existing_process_and_not_a_missing_one() {
    let source = MacProcessSource::new().expect("source");

    let own = source
        .process_info(std::process::id())
        .expect("the current process is visible");
    assert_eq!(own.id.pid, std::process::id());
    assert!(!own.argv.is_empty());
    assert_eq!(
        own.cwd
            .as_deref()
            .map(|path| path.canonicalize().expect("cwd")),
        Some(
            std::env::current_dir()
                .expect("cwd")
                .canonicalize()
                .expect("cwd")
        )
    );

    // pid_max on macOS is 99 998; this pid cannot exist.
    assert!(source.process_info(4_000_000).is_none());
}
