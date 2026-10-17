//! Who started a process.
//!
//! A registered instance is bound to whatever listens on its reserved port when
//! that process is the launcher or one of its descendants (`ARQUITECTURA.md`,
//! section 7). `exec` keeps the launcher's pid, so a server started directly is
//! the launcher itself; one that forks (`npm run dev`) is a descendant. A
//! process that detaches (the parent exits and `launchd` adopts it) loses its
//! ancestry, which is why a weaker, directory-based rule exists as well.
//!
//! Ancestry only informs the binding and the activity log; no destructive action
//! depends on it alone (ADR-004).

use super::{ProcessId, ProcessInfo, ProcessSource};

/// How many ancestors are followed at most. Real process trees are a handful of
/// levels deep; the cap makes a corrupt or cyclic table harmless.
pub const MAX_DEPTH: usize = 64;

/// The ancestors of `pid`, nearest first: its parent, the parent's parent, and
/// so on.
///
/// The walk stops, without error, at a parent that cannot be read (another
/// user's process, or one that is gone), at `launchd`, at a parent that started
/// *after* its child (the parent's pid was reused), and at a cycle.
#[must_use]
pub fn chain<S: ProcessSource + ?Sized>(source: &S, pid: u32) -> Vec<ProcessInfo> {
    let mut ancestors: Vec<ProcessInfo> = Vec::new();
    let Some(mut current) = source.process_info(pid) else {
        return ancestors;
    };
    while ancestors.len() < MAX_DEPTH {
        let parent_pid = current.ppid;
        if parent_pid <= 1 || parent_pid == current.id.pid {
            break;
        }
        let Some(parent) = source.process_info(parent_pid) else {
            break;
        };
        if parent.id.start > current.id.start
            || parent.id.pid == pid
            || ancestors.iter().any(|seen| seen.id.pid == parent.id.pid)
        {
            break;
        }
        ancestors.push(parent.clone());
        current = parent;
    }
    ancestors
}

/// Whether `pid` is `ancestor` itself or one of its descendants. The ancestor is
/// compared by `(pid, start)`, so a new process that received a recycled pid does
/// not count.
#[must_use]
pub fn is_self_or_descendant<S: ProcessSource + ?Sized>(
    source: &S,
    pid: u32,
    ancestor: ProcessId,
) -> bool {
    let Some(process) = source.process_info(pid) else {
        return false;
    };
    if process.id == ancestor {
        return true;
    }
    chain(source, pid)
        .iter()
        .any(|candidate| candidate.id == ancestor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::fake::FakeProcessSource;
    use crate::process::{ListeningProcess, ProcessStart};

    fn process(pid: u32, parent: u32, start: u64) -> ListeningProcess {
        ListeningProcess {
            info: ProcessInfo {
                id: ProcessId {
                    pid,
                    start: ProcessStart(start),
                },
                ppid: parent,
                pgid: pid,
                uid: 501,
                argv: Vec::new(),
                cwd: None,
            },
            ports: Vec::new(),
        }
    }

    fn id(pid: u32, start: u64) -> ProcessId {
        ProcessId {
            pid,
            start: ProcessStart(start),
        }
    }

    fn pids(chain: &[ProcessInfo]) -> Vec<u32> {
        chain.iter().map(|info| info.id.pid).collect()
    }

    /// 10 (launcher) -> 20 (npm) -> 30 (node)
    fn tree() -> FakeProcessSource {
        FakeProcessSource::new(vec![
            process(10, 1, 100),
            process(20, 10, 200),
            process(30, 20, 300),
        ])
    }

    #[test]
    fn the_chain_lists_ancestors_nearest_first_and_stops_at_launchd() {
        assert_eq!(pids(&chain(&tree(), 30)), [20, 10]);
        assert_eq!(pids(&chain(&tree(), 20)), [10]);
        assert!(chain(&tree(), 10).is_empty());
    }

    #[test]
    fn an_unknown_process_has_no_chain() {
        assert!(chain(&tree(), 999).is_empty());
    }

    #[test]
    fn the_walk_stops_at_a_parent_that_cannot_be_read() {
        let source = FakeProcessSource::new(vec![process(30, 20, 300), process(10, 1, 100)]);
        assert!(chain(&source, 30).is_empty());
    }

    #[test]
    fn a_parent_that_started_after_its_child_is_a_reused_pid() {
        let source = FakeProcessSource::new(vec![process(30, 20, 300), process(20, 1, 900)]);
        assert!(chain(&source, 30).is_empty());
    }

    #[test]
    fn a_cycle_terminates() {
        let source = FakeProcessSource::new(vec![process(10, 20, 100), process(20, 10, 100)]);
        let found = chain(&source, 10);
        assert_eq!(pids(&found), [20]);
    }

    #[test]
    fn a_process_that_is_its_own_parent_terminates() {
        let source = FakeProcessSource::new(vec![process(10, 10, 100)]);
        assert!(chain(&source, 10).is_empty());
    }

    #[test]
    fn a_deep_chain_is_cut_at_the_cap() {
        let count = u32::try_from(MAX_DEPTH + 20).unwrap();
        let processes = (0..count)
            .map(|index| {
                process(
                    index + 2,
                    if index == 0 { 1 } else { index + 1 },
                    1_000 + u64::from(index),
                )
            })
            .collect();
        let source = FakeProcessSource::new(processes);
        assert_eq!(chain(&source, count + 1).len(), MAX_DEPTH);
    }

    #[test]
    fn a_descendant_is_found_through_any_number_of_levels() {
        let source = tree();
        assert!(is_self_or_descendant(&source, 30, id(10, 100)));
        assert!(is_self_or_descendant(&source, 30, id(20, 200)));
        assert!(!is_self_or_descendant(&source, 10, id(30, 300)));
    }

    #[test]
    fn a_process_is_its_own_launcher_after_exec() {
        assert!(is_self_or_descendant(&tree(), 10, id(10, 100)));
    }

    // covers: PT-03, U-10
    #[test]
    fn a_recycled_pid_is_not_the_launcher() {
        // Pid 10 exists, but it started at 150, not at 100: it is not the process
        // the launcher was recorded as.
        let source = FakeProcessSource::new(vec![process(10, 1, 150), process(30, 10, 300)]);
        assert!(!is_self_or_descendant(&source, 30, id(10, 100)));
    }

    #[test]
    fn an_unrelated_or_unknown_process_is_not_a_descendant() {
        let source = FakeProcessSource::new(vec![process(10, 1, 100), process(40, 1, 400)]);
        assert!(!is_self_or_descendant(&source, 40, id(10, 100)));
        assert!(!is_self_or_descendant(&source, 999, id(10, 100)));
    }
}
