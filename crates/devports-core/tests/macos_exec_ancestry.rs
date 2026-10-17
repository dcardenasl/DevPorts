//! `exec`, process ids and ancestry on a real macOS (`ARQUITECTURA.md` V9).
//!
//! Binding a registered instance to a listener relies on three facts that this
//! file checks with real processes instead of trusting documentation:
//!
//! 1. `sh -c 'exec <command>'` keeps the process id, so a server started
//!    directly *is* the launcher the CLI recorded;
//! 2. a server that forks is a *descendant* of the launcher;
//! 3. a server that detaches loses its ancestry (it is adopted by `launchd`),
//!    which is why the weaker, directory-based binding rule exists;
//!
//! and one fact the stop procedure relies on: a non-interactive shell does not
//! create its own process group, so a server inherits the group of whoever
//! started it, which is why stopping must never signal a whole group.

#![cfg(target_os = "macos")]
// Spawning, sleeping and panicking helpers are the whole point of this file.
#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use devports_core::platform::macos::MacProcessSource;
use devports_core::process::ancestry::{chain, is_self_or_descendant};
use devports_core::process::{ListeningProcess, ProcessId, ProcessSource};

/// Kills everything it was given when it goes out of scope, even after a failed
/// assertion.
struct Cleanup {
    children: Vec<Child>,
    pids: Vec<u32>,
}

impl Cleanup {
    fn new(child: Child) -> Self {
        Self {
            children: vec![child],
            pids: Vec::new(),
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for pid in &self.pids {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .expect("local address")
        .port()
}

/// Runs `script` with `sh -c`, with `stdout` piped, and returns the child plus
/// the first line it printed (the script prints a pid on purpose).
fn run_shell(script: &str) -> (Child, u32) {
    let mut child = Command::new("sh")
        .args(["-c", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sh starts");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout is piped"))
        .read_line(&mut line)
        .expect("the script prints a pid");
    let printed = line.trim().parse().expect("the first line is a pid");
    (child, printed)
}

fn wait_for_port(source: MacProcessSource, port: u16) -> ListeningProcess {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(10))
        .expect("deadline fits");
    loop {
        if let Some(found) = source
            .snapshot()
            .expect("snapshot")
            .into_iter()
            .find(|process| process.ports.contains(&port))
        {
            return found;
        }
        assert!(Instant::now() < deadline, "nothing listened on {port}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn server(port: u16) -> String {
    format!("python3 -m http.server {port} --bind 127.0.0.1")
}

#[test]
fn exec_keeps_the_pid_so_the_listener_is_the_launcher() {
    let source = MacProcessSource::new().expect("source");
    let port = free_port();
    let (child, shell_pid) = run_shell(&format!("echo $$; exec {}", server(port)));
    let launcher = child.id();
    let _cleanup = Cleanup::new(child);

    assert_eq!(shell_pid, launcher, "`$$` is the pid the parent spawned");
    let found = wait_for_port(source, port);

    // The CLI records its own pid before `exec`; after `exec` the server has it.
    assert_eq!(found.info.id.pid, launcher);
    assert_eq!(found.info.ppid, std::process::id());
    assert!(
        found.info.argv.iter().any(|part| part == "http.server"),
        "the process image was replaced: {:?}",
        found.info.argv
    );
    let launcher_id = found.info.id;
    assert!(is_self_or_descendant(&source, launcher, launcher_id));
}

#[test]
fn a_server_inherits_the_process_group_of_whoever_started_it() {
    let source = MacProcessSource::new().expect("source");
    let port = free_port();
    let (child, _) = run_shell(&format!("echo $$; exec {}", server(port)));
    let _cleanup = Cleanup::new(child);

    let found = wait_for_port(source, port);
    let ours = source
        .process_info(std::process::id())
        .expect("the test process is visible");

    // Not its own group leader: signalling the group would reach the test
    // runner (in real life, the shell of the agent or of the terminal).
    assert_ne!(found.info.pgid, found.info.id.pid);
    assert_eq!(found.info.pgid, ours.pgid);
}

#[test]
fn a_server_that_forks_is_a_descendant_of_the_launcher() {
    let source = MacProcessSource::new().expect("source");
    let port = free_port();
    let (child, server_pid) = run_shell(&format!("{} & echo $!; wait", server(port)));
    let launcher = child.id();
    let mut cleanup = Cleanup::new(child);
    cleanup.pids.push(server_pid);

    let found = wait_for_port(source, port);
    let launcher_id = source
        .process_info(launcher)
        .expect("the launcher is alive")
        .id;

    assert_eq!(found.info.id.pid, server_pid);
    assert_ne!(found.info.id.pid, launcher);
    assert_eq!(found.info.ppid, launcher);
    assert!(is_self_or_descendant(&source, server_pid, launcher_id));
    let ancestors: Vec<u32> = chain(&source, server_pid)
        .iter()
        .map(|info| info.id.pid)
        .collect();
    assert_eq!(ancestors.first(), Some(&launcher));
}

#[test]
fn a_detached_server_loses_its_ancestry() {
    let source = MacProcessSource::new().expect("source");
    let port = free_port();
    // The subshell backgrounds the server and exits, so `launchd` adopts it.
    let (mut child, server_pid) =
        run_shell(&format!("( {} >/dev/null 2>&1 & echo $! )", server(port)));
    child.wait().expect("the launcher exits on its own");
    let launcher = child.id();
    let mut cleanup = Cleanup::new(child);
    cleanup.pids.push(server_pid);

    let found = wait_for_port(source, port);

    assert_eq!(found.info.id.pid, server_pid);
    assert_eq!(found.info.ppid, 1, "adopted by launchd");
    assert!(chain(&source, server_pid).is_empty());
    // Whatever the launcher was, nothing ties the server to it any more.
    let some_launcher = ProcessId {
        pid: launcher,
        start: found.info.id.start,
    };
    assert!(!is_self_or_descendant(&source, server_pid, some_launcher));
}
