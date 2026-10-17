//! U-09 and PT-04: the safety rules of stopping, with a fake process table.

// Test data is built with plain arithmetic and short names on purpose.
#![allow(clippy::arithmetic_side_effects, clippy::similar_names)]

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use proptest::prelude::*;

use super::*;
use crate::process::fake::FakeProcessSource;
use crate::process::{ListeningProcess, ProcessInfo, ProcessStart};

const ME: u32 = 501;

fn id(pid: u32, start: u64) -> ProcessId {
    ProcessId {
        pid,
        start: ProcessStart(start),
    }
}

fn brief(pid: u32, ppid: u32, start: u64, uid: u32) -> ProcessBrief {
    ProcessBrief {
        id: id(pid, start),
        ppid,
        uid,
    }
}

fn source(table: &[ProcessBrief]) -> FakeProcessSource {
    FakeProcessSource::new(
        table
            .iter()
            .map(|brief| ListeningProcess {
                info: ProcessInfo {
                    id: brief.id,
                    ppid: brief.ppid,
                    pgid: brief.id.pid,
                    uid: brief.uid,
                    argv: Vec::new(),
                    cwd: None,
                },
                ports: Vec::new(),
            })
            .collect(),
    )
}

/// Records every signal; optionally makes processes disappear when signalled, as
/// a real one does.
#[derive(Default)]
struct Recorder {
    sent: Mutex<Vec<(u32, Signal)>>,
}

impl Recorder {
    fn sent(&self) -> Vec<(u32, Signal)> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Signaler for Recorder {
    fn signal(&self, pid: u32, signal: Signal) -> io::Result<()> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((pid, signal));
        Ok(())
    }
}

/// A process table whose members vanish when they receive the given signal.
struct Mortal<'a> {
    source: &'a FakeProcessSource,
    table: Mutex<Vec<ProcessBrief>>,
    dies_on: Signal,
    stubborn: BTreeSet<u32>,
    sent: Mutex<Vec<(u32, Signal)>>,
}

impl Signaler for Mortal<'_> {
    fn signal(&self, pid: u32, signal: Signal) -> io::Result<()> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((pid, signal));
        let dies = signal == Signal::Kill || signal == self.dies_on;
        if dies && !(self.stubborn.contains(&pid) && signal != Signal::Kill) {
            let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
            table.retain(|brief| brief.id.pid != pid);
            self.source.set(
                table
                    .iter()
                    .map(|brief| ListeningProcess {
                        info: ProcessInfo {
                            id: brief.id,
                            ppid: brief.ppid,
                            pgid: brief.id.pid,
                            uid: brief.uid,
                            argv: Vec::new(),
                            cwd: None,
                        },
                        ports: Vec::new(),
                    })
                    .collect(),
            );
        }
        Ok(())
    }
}

fn fast() -> Timings {
    Timings {
        grace: Duration::from_millis(30),
        after_kill: Duration::from_millis(30),
        poll: Duration::from_millis(5),
    }
}

fn protected(pids: &[u32]) -> BTreeSet<u32> {
    pids.iter().copied().chain([1]).collect()
}

// covers: U-09
#[test]
fn the_set_is_the_target_plus_its_descendants_and_nothing_else() {
    //  10 -> 20 -> 30        40 (unrelated)       50 -> 60 (sibling tree)
    let table = [
        brief(10, 1, 100, ME),
        brief(20, 10, 200, ME),
        brief(30, 20, 300, ME),
        brief(40, 1, 150, ME),
        brief(50, 1, 160, ME),
        brief(60, 50, 260, ME),
    ];
    let set = termination_set(&table, id(10, 100)).unwrap();
    let pids: BTreeSet<u32> = set.iter().map(|brief| brief.id.pid).collect();
    assert_eq!(pids, [10, 20, 30].into_iter().collect());
}

// covers: PT-03
#[test]
fn a_recycled_pid_is_not_the_target() {
    let table = [brief(10, 1, 999, ME)];
    assert!(termination_set(&table, id(10, 100)).is_none());
    assert!(termination_set(&table, id(11, 100)).is_none());
}

#[test]
fn a_child_older_than_its_parent_is_a_recycled_parent_pid_and_is_left_out() {
    let table = [brief(10, 1, 500, ME), brief(20, 10, 100, ME)];
    let set = termination_set(&table, id(10, 500)).unwrap();
    assert_eq!(set.len(), 1);
}

#[test]
fn cycles_in_the_table_terminate() {
    let table = [brief(10, 20, 100, ME), brief(20, 10, 100, ME)];
    assert_eq!(termination_set(&table, id(10, 100)).unwrap().len(), 2);
}

// covers: U-09
#[test]
fn the_prohibited_pids_are_refused_even_deep_in_the_tree() {
    let set = [
        brief(10, 1, 100, ME),
        brief(20, 10, 200, ME),
        brief(99, 20, 300, ME),
    ];
    // 99 is this very process: the tree below the target contains DevPorts.
    assert_eq!(
        check_set(&set, &protected(&[99]), ME),
        Err(Refusal::Protected(99))
    );
    assert_eq!(
        check_set(&set, &protected(&[10]), ME),
        Err(Refusal::Protected(10))
    );
    assert_eq!(
        check_set(&[brief(1, 0, 1, ME)], &protected(&[]), ME),
        Err(Refusal::Protected(1))
    );
    assert!(check_set(&set, &protected(&[7]), ME).is_ok());
}

// covers: U-09
#[test]
fn another_users_process_is_refused() {
    let set = [brief(10, 1, 100, ME), brief(20, 10, 200, 0)];
    assert_eq!(
        check_set(&set, &protected(&[]), ME),
        Err(Refusal::OtherUser(20))
    );
}

// covers: U-09
#[test]
fn protected_pids_include_the_ancestors_of_this_process() {
    let table = [
        brief(10, 1, 100, ME),
        brief(20, 10, 200, ME),
        brief(30, 20, 300, ME),
    ];
    let found = protected_pids(&source(&table), 30);
    assert_eq!(found, [1, 10, 20, 30].into_iter().collect());
}

#[tokio::test]
async fn an_unknown_target_is_simply_gone() {
    let fake = source(&[brief(10, 1, 100, ME)]);
    let recorder = Recorder::default();
    let outcome = terminate(&fake, &recorder, id(77, 1), &protected(&[]), ME, fast()).await;
    assert_eq!(outcome, Outcome::AlreadyGone);
    assert!(recorder.sent().is_empty());
}

#[tokio::test]
async fn a_refused_target_receives_no_signal_at_all() {
    let table = [brief(10, 1, 100, ME), brief(99, 10, 200, ME)];
    let fake = source(&table);
    let recorder = Recorder::default();
    let outcome = terminate(&fake, &recorder, id(10, 100), &protected(&[99]), ME, fast()).await;
    assert_eq!(outcome, Outcome::Refused(Refusal::Protected(99)));
    assert!(recorder.sent().is_empty());
}

#[tokio::test]
async fn processes_that_obey_sigterm_are_never_sent_sigkill() {
    let table = vec![
        brief(10, 1, 100, ME),
        brief(20, 10, 200, ME),
        brief(40, 1, 150, ME),
    ];
    let fake = source(&table);
    let mortal = Mortal {
        source: &fake,
        table: Mutex::new(table),
        dies_on: Signal::Term,
        stubborn: BTreeSet::new(),
        sent: Mutex::new(Vec::new()),
    };

    let outcome = terminate(&fake, &mortal, id(10, 100), &protected(&[]), ME, fast()).await;

    assert_eq!(outcome, Outcome::Stopped);
    let sent = mortal.sent.lock().unwrap().clone();
    assert!(
        sent.iter().all(|(_, signal)| *signal == Signal::Term),
        "{sent:?}"
    );
    let touched: BTreeSet<u32> = sent.iter().map(|(pid, _)| *pid).collect();
    assert_eq!(touched, [10, 20].into_iter().collect(), "40 is unrelated");
}

#[tokio::test]
async fn a_process_that_ignores_sigterm_gets_sigkill_afterwards() {
    let table = vec![brief(10, 1, 100, ME), brief(20, 10, 200, ME)];
    let fake = source(&table);
    let mortal = Mortal {
        source: &fake,
        table: Mutex::new(table),
        dies_on: Signal::Term,
        stubborn: [20].into_iter().collect(),
        sent: Mutex::new(Vec::new()),
    };

    let outcome = terminate(&fake, &mortal, id(10, 100), &protected(&[]), ME, fast()).await;

    assert_eq!(outcome, Outcome::Stopped);
    let sent = mortal.sent.lock().unwrap().clone();
    let first_kill = sent
        .iter()
        .position(|(_, signal)| *signal == Signal::Kill)
        .unwrap();
    let last_term = sent
        .iter()
        .rposition(|(_, signal)| *signal == Signal::Term)
        .unwrap();
    assert!(
        last_term < first_kill,
        "SIGKILL only after the grace period: {sent:?}"
    );
    assert!(sent.contains(&(20, Signal::Kill)));
    assert!(
        !sent.contains(&(10, Signal::Kill)),
        "the obedient one is left alone"
    );
}

#[tokio::test]
async fn what_survives_sigkill_is_reported_and_not_retried() {
    let table = vec![brief(10, 1, 100, ME)];
    let fake = source(&table);
    // Signals are recorded but nothing ever dies.
    let recorder = Recorder::default();

    let outcome = terminate(&fake, &recorder, id(10, 100), &protected(&[]), ME, fast()).await;

    assert_eq!(outcome, Outcome::StillAlive(vec![id(10, 100)]));
    let sent = recorder.sent();
    assert_eq!(
        sent,
        [(10, Signal::Term), (10, Signal::Kill)],
        "one of each, no loop"
    );
}

proptest! {
    /// PT-04: whatever the tree, the signals go to the target and its descendants
    /// only, and never to a protected pid.
    // covers: PT-04
    #[test]
    fn signals_never_leave_the_target_subtree(
        parents in prop::collection::vec(0_usize..40, 1..40),
        root_index in 0_usize..40,
        protected_index in 0_usize..40,
    ) {
        // Process i has pid i + 2 and parent `parents[i] % (i + 1)` (a tree where
        // parents start earlier), or launchd for the first one.
        let table: Vec<ProcessBrief> = parents
            .iter()
            .enumerate()
            .map(|(index, parent)| {
                let ppid = if index == 0 { 1 } else { u32::try_from(parent % index).unwrap_or(0) + 2 };
                brief(u32::try_from(index).unwrap() + 2, ppid, 1_000 + u64::try_from(index).unwrap(), ME)
            })
            .collect();
        let root = table[root_index % table.len()].id;
        let protected_pid = table[protected_index % table.len()].id.pid;

        let set = termination_set(&table, root).unwrap();
        let allowed: BTreeSet<u32> = set.iter().map(|brief| brief.id.pid).collect();

        // Everything in the set descends from the root.
        let by_pid: std::collections::BTreeMap<u32, &ProcessBrief> =
            table.iter().map(|brief| (brief.id.pid, brief)).collect();
        for member in &set {
            let mut cursor = member.id.pid;
            let mut steps = 0;
            while cursor != root.pid {
                cursor = by_pid[&cursor].ppid;
                steps += 1;
                prop_assert!(steps <= table.len() + 1, "{} is not below the root", member.id.pid);
            }
        }
        // Nothing outside the subtree is in the set, and a protected pid in the
        // set is refused as a whole.
        for brief in &table {
            if !allowed.contains(&brief.id.pid) {
                prop_assert!(!set.iter().any(|member| member.id.pid == brief.id.pid));
            }
        }
        let verdict = check_set(&set, &[protected_pid].into_iter().collect(), ME);
        prop_assert_eq!(verdict.is_err(), allowed.contains(&protected_pid));
    }
}

/// A process source that can lose its process table, or hand a pid to a
/// different process once the target has been listed.
struct Unreliable<'a> {
    inner: &'a FakeProcessSource,
    /// Listings fail once this many calls to `all_processes` have been made.
    fail_after: usize,
    calls: Mutex<usize>,
    /// What `process_info` reports for this pid instead of the truth.
    replaced: Option<(u32, ProcessId)>,
}

impl ProcessSource for Unreliable<'_> {
    fn snapshot(&self) -> Result<Vec<ListeningProcess>, crate::process::ProcessSourceError> {
        self.inner.snapshot()
    }

    fn all_processes(&self) -> Result<Vec<ProcessBrief>, crate::process::ProcessSourceError> {
        let mut calls = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
        *calls += 1;
        if *calls > self.fail_after {
            return Err(crate::process::ProcessSourceError::Enumerate(
                "table unavailable".to_owned(),
            ));
        }
        self.inner.all_processes()
    }

    fn process_info(&self, pid: u32) -> Option<ProcessInfo> {
        let mut info = self.inner.process_info(pid)?;
        if let Some((replaced, other)) = self.replaced
            && replaced == pid
        {
            info.id = other;
        }
        Some(info)
    }
}

// covers: U-09, NF-06
#[tokio::test]
async fn an_unreadable_process_table_is_never_reported_as_stopped() {
    let table = [brief(10, 1, 100, ME)];
    let inner = source(&table);
    let recorder = Recorder::default();

    // Unreadable from the start: nothing is signalled.
    let none = Unreliable {
        inner: &inner,
        fail_after: 0,
        calls: Mutex::new(0),
        replaced: None,
    };
    let outcome = terminate(&none, &recorder, id(10, 100), &protected(&[]), ME, fast()).await;
    assert_eq!(outcome, Outcome::Unreadable);
    assert!(recorder.sent().is_empty());

    // Readable once, then lost while waiting: the stop cannot be confirmed.
    let lost = Unreliable {
        inner: &inner,
        fail_after: 1,
        calls: Mutex::new(0),
        replaced: None,
    };
    let outcome = terminate(&lost, &recorder, id(10, 100), &protected(&[]), ME, fast()).await;
    assert_eq!(outcome, Outcome::Unreadable);
}

// covers: U-09, PT-03, NF-06
#[tokio::test]
async fn a_pid_handed_to_another_process_after_listing_is_not_signalled() {
    let table = [brief(10, 1, 100, ME)];
    let inner = source(&table);
    let recorder = Recorder::default();
    // By the time of the signal, pid 10 belongs to a process that started later.
    let racing = Unreliable {
        inner: &inner,
        fail_after: usize::MAX,
        calls: Mutex::new(0),
        replaced: Some((10, id(10, 999))),
    };
    let outcome = terminate(&racing, &recorder, id(10, 100), &protected(&[]), ME, fast()).await;
    assert!(recorder.sent().is_empty(), "{:?}", recorder.sent());
    // Nothing was done to it, and it is still listed under its recorded identity.
    assert!(matches!(outcome, Outcome::StillAlive(_) | Outcome::Stopped));
}
