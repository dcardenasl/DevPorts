//! Stopping a process safely (`ARQUITECTURA.md`, section 8).
//!
//! This is the only code that ends processes, and the one place where a mistake
//! costs the user their work, so the rules are narrow and enforced here, not by
//! the callers:
//!
//! * the target is identified by `(pid, start_time)`: a recycled pid is a
//!   different process and is left alone;
//! * the set stopped is the target and its descendants, nothing else; process
//!   *groups* are never signalled, because a non-interactive shell does not make
//!   its own group and signalling it would reach whoever started the target;
//! * the DevPorts process itself, its ancestors and `launchd` are never touched;
//! * a polite `SIGTERM` first, a `SIGKILL` only for what is still alive after the
//!   grace period, and no retry loop: what survives is reported.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::time::Duration;

use super::{ProcessBrief, ProcessId, ProcessSource};

/// The two signals DevPorts sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Ask politely.
    Term,
    /// Insist.
    Kill,
}

/// Sends signals to single processes.
pub trait Signaler: Send + Sync {
    /// Sends `signal` to the process `pid`.
    ///
    /// # Errors
    ///
    /// The system's error; "no such process" is not a failure worth reporting
    /// and callers ignore it.
    fn signal(&self, pid: u32, signal: Signal) -> io::Result<()>;
}

/// Signals real processes through the operating system.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsSignaler;

impl Signaler for OsSignaler {
    fn signal(&self, pid: u32, signal: Signal) -> io::Result<()> {
        let raw = i32::try_from(pid).map_err(io::Error::other)?;
        let pid = rustix::process::Pid::from_raw(raw)
            .ok_or_else(|| io::Error::other("pid 0 is not a process"))?;
        let signal = match signal {
            Signal::Term => rustix::process::Signal::TERM,
            Signal::Kill => rustix::process::Signal::KILL,
        };
        rustix::process::kill_process(pid, signal).map_err(io::Error::from)
    }
}

/// Why a process was not touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// It is the DevPorts process, one of its ancestors, or `launchd`.
    Protected(u32),
    /// It belongs to another user.
    OtherUser(u32),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Protected(pid) => {
                write!(f, "el proceso {pid} está protegido (DevPorts o su entorno)")
            }
            Self::OtherUser(pid) => write!(f, "el proceso {pid} pertenece a otro usuario"),
        }
    }
}

/// Timings of the procedure; the defaults are the documented ones.
#[derive(Debug, Clone, Copy)]
pub struct Timings {
    /// How long to wait after `SIGTERM` before `SIGKILL`.
    pub grace: Duration,
    /// How long to wait after `SIGKILL` before giving up.
    pub after_kill: Duration,
    /// How often to look again.
    pub poll: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            grace: Duration::from_secs(5),
            after_kill: Duration::from_secs(2),
            poll: Duration::from_millis(100),
        }
    }
}

/// How the procedure ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The target was already gone, or is not the process it was recorded as.
    AlreadyGone,
    /// Everything that was signalled is gone.
    Stopped,
    /// These survived `SIGKILL`: it is reported, never retried in a loop.
    StillAlive(Vec<ProcessId>),
    /// The rules forbid touching it.
    Refused(Refusal),
}

/// The pids that must never be signalled: this process, its ancestors and 1.
#[must_use]
pub fn protected_pids<S: ProcessSource + ?Sized>(source: &S, own_pid: u32) -> BTreeSet<u32> {
    let mut protected: BTreeSet<u32> = [own_pid, 1].into_iter().collect();
    protected.extend(
        super::ancestry::chain(source, own_pid)
            .into_iter()
            .map(|ancestor| ancestor.id.pid),
    );
    protected
}

/// The target plus every descendant, found from the parent links of `table`.
///
/// `None` if the target is not in the table with the recorded start time.
#[must_use]
pub fn termination_set(table: &[ProcessBrief], root: ProcessId) -> Option<Vec<ProcessBrief>> {
    let by_pid: BTreeMap<u32, &ProcessBrief> =
        table.iter().map(|brief| (brief.id.pid, brief)).collect();
    let root_brief = by_pid.get(&root.pid).filter(|brief| brief.id == root)?;
    let mut set = vec![**root_brief];
    let mut seen: BTreeSet<u32> = [root.pid].into_iter().collect();
    let mut index = 0;
    while let Some(parent) = set.get(index).copied() {
        index = index.saturating_add(1);
        for child in table {
            // A child that started before its parent is a recycled parent pid.
            if child.ppid == parent.id.pid
                && child.id.start >= parent.id.start
                && seen.insert(child.id.pid)
            {
                set.push(*child);
            }
        }
    }
    Some(set)
}

/// Checks the set against the rules.
///
/// # Errors
///
/// The [`Refusal`] for the first process the rules forbid.
pub fn check_set(set: &[ProcessBrief], protected: &BTreeSet<u32>, uid: u32) -> Result<(), Refusal> {
    for brief in set {
        if protected.contains(&brief.id.pid) {
            return Err(Refusal::Protected(brief.id.pid));
        }
        if brief.uid != uid {
            return Err(Refusal::OtherUser(brief.id.pid));
        }
    }
    Ok(())
}

/// Stops `root` and its descendants. See the module documentation for the rules.
pub async fn terminate<S: ProcessSource + ?Sized, G: Signaler + ?Sized>(
    source: &S,
    signaler: &G,
    root: ProcessId,
    protected: &BTreeSet<u32>,
    uid: u32,
    timings: Timings,
) -> Outcome {
    let Ok(table) = source.all_processes() else {
        return Outcome::AlreadyGone;
    };
    let Some(set) = termination_set(&table, root) else {
        return Outcome::AlreadyGone;
    };
    if let Err(refusal) = check_set(&set, protected, uid) {
        return Outcome::Refused(refusal);
    }

    let mut known: BTreeSet<ProcessId> = set.iter().map(|brief| brief.id).collect();
    signal_all(signaler, &known, Signal::Term);
    let mut alive = wait_until_gone(
        source,
        root,
        &mut known,
        signaler,
        timings.grace,
        timings.poll,
    )
    .await;
    if alive.is_empty() {
        return Outcome::Stopped;
    }
    signal_all(signaler, &alive.iter().copied().collect(), Signal::Kill);
    alive = wait_until_gone(
        source,
        root,
        &mut known,
        signaler,
        timings.after_kill,
        timings.poll,
    )
    .await;
    if alive.is_empty() {
        Outcome::Stopped
    } else {
        Outcome::StillAlive(alive)
    }
}

fn signal_all<G: Signaler + ?Sized>(signaler: &G, targets: &BTreeSet<ProcessId>, signal: Signal) {
    for target in targets {
        // A process that exits between listing and signalling is the goal, not an error.
        let _ = signaler.signal(target.pid, signal);
    }
}

/// Polls until everything known is gone or `limit` passes; returns what is left.
/// Descendants that appear meanwhile are added and asked to stop too.
async fn wait_until_gone<S: ProcessSource + ?Sized, G: Signaler + ?Sized>(
    source: &S,
    root: ProcessId,
    known: &mut BTreeSet<ProcessId>,
    signaler: &G,
    limit: Duration,
    poll: Duration,
) -> Vec<ProcessId> {
    let mut waited = Duration::ZERO;
    loop {
        let table = source.all_processes().unwrap_or_default();
        if let Some(current) = termination_set(&table, root) {
            let newcomers: BTreeSet<ProcessId> = current
                .iter()
                .map(|brief| brief.id)
                .filter(|id| !known.contains(id))
                .collect();
            if !newcomers.is_empty() {
                signal_all(signaler, &newcomers, Signal::Term);
                known.extend(newcomers);
            }
        }
        let existing: BTreeSet<ProcessId> = table.iter().map(|brief| brief.id).collect();
        let alive: Vec<ProcessId> = known
            .iter()
            .copied()
            .filter(|id| existing.contains(id))
            .collect();
        if alive.is_empty() || waited >= limit {
            return alive;
        }
        tokio::time::sleep(poll).await;
        waited = waited.saturating_add(poll);
    }
}

#[cfg(test)]
mod tests;
