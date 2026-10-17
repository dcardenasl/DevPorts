//! Stopping real processes (`PRUEBAS.md`: I-06 and I-07).

#![cfg(target_os = "macos")]
#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use devports_core::platform::macos::MacProcessSource;
use devports_core::process::terminate::{
    OsSignaler, Outcome, Refusal, Timings, protected_pids, terminate,
};
use devports_core::process::{ProcessId, ProcessSource, ProcessStart};

struct Guard(Child, Vec<u32>);

impl Drop for Guard {
    fn drop(&mut self) {
        for pid in &self.1 {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn timings() -> Timings {
    Timings {
        grace: Duration::from_millis(1500),
        after_kill: Duration::from_secs(2),
        poll: Duration::from_millis(50),
    }
}

/// Starts `script` under `sh`; it must print the pids of what it spawned, space
/// separated, on its first line.
fn spawn(script: &str) -> (Guard, Vec<u32>) {
    let mut child = Command::new("sh")
        .args(["-c", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("sh starts");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("the script reports its children");
    let pids: Vec<u32> = line
        .split_whitespace()
        .map(|pid| pid.parse().expect("a pid"))
        .collect();
    (Guard(child, pids.clone()), pids)
}

fn exists(source: MacProcessSource, pid: u32) -> bool {
    source.process_info(pid).is_some()
}

fn identity(source: MacProcessSource, pid: u32) -> ProcessId {
    source.process_info(pid).expect("alive").id
}

// covers: I-06, I-07
#[tokio::test]
async fn a_launcher_and_all_its_children_die_and_the_caller_survives() {
    let source = MacProcessSource::new().expect("source");
    let own = std::process::id();
    let own_group = source.process_info(own).expect("self").pgid;
    let (guard, children) =
        spawn("sleep 300 & a=$!; sleep 300 & b=$!; sleep 300 & c=$!; echo $a $b $c; wait");
    let root = identity(source, guard.0.id());
    let uid = source.process_info(own).expect("self").uid;

    let outcome = terminate(
        &source,
        &OsSignaler,
        root,
        &protected_pids(&source, own),
        uid,
        timings(),
    )
    .await;

    assert_eq!(outcome, Outcome::Stopped);
    assert!(!exists(source, guard.0.id()), "the launcher is gone");
    for child in &children {
        assert!(!exists(source, *child), "child {child} is gone");
    }
    // I-07: the shell shared this process's group; nothing signalled the group.
    assert!(exists(source, own), "the caller must survive");
    assert_eq!(source.process_info(own).expect("self").pgid, own_group);
}

#[tokio::test]
async fn a_process_that_ignores_sigterm_is_killed_after_the_grace_period() {
    let source = MacProcessSource::new().expect("source");
    let own = std::process::id();
    let (guard, _) = spawn(
        "python3 -c \"import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(300)\" & echo $!; wait",
    );
    let root = identity(source, guard.0.id());
    // Give python a moment to install its handler.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let uid = source.process_info(own).expect("self").uid;

    let outcome = terminate(
        &source,
        &OsSignaler,
        root,
        &protected_pids(&source, own),
        uid,
        timings(),
    )
    .await;

    assert_eq!(outcome, Outcome::Stopped);
    assert!(!exists(source, guard.0.id()));
}

// covers: U-09
#[tokio::test]
async fn the_devports_process_itself_is_never_a_target() {
    let source = MacProcessSource::new().expect("source");
    let own = std::process::id();
    let root = identity(source, own);
    let uid = source.process_info(own).expect("self").uid;

    let outcome = terminate(
        &source,
        &OsSignaler,
        root,
        &protected_pids(&source, own),
        uid,
        timings(),
    )
    .await;

    assert_eq!(outcome, Outcome::Refused(Refusal::Protected(own)));
    assert!(exists(source, own));
}

// covers: PT-03
#[tokio::test]
async fn a_recycled_pid_is_left_alone() {
    let source = MacProcessSource::new().expect("source");
    let own = std::process::id();
    let (guard, _) = spawn("sleep 300 & echo $!; wait");
    let alive = identity(source, guard.0.id());
    // Same pid, but a start time that is not the one recorded.
    let imposter = ProcessId {
        pid: alive.pid,
        start: ProcessStart(alive.start.0.saturating_add(1)),
    };
    let uid = source.process_info(own).expect("self").uid;

    let outcome = terminate(
        &source,
        &OsSignaler,
        imposter,
        &protected_pids(&source, own),
        uid,
        timings(),
    )
    .await;

    assert_eq!(outcome, Outcome::AlreadyGone);
    assert!(
        exists(source, guard.0.id()),
        "the real process was not touched"
    );
}
