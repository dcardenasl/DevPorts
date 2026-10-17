//! In-memory [`ProcessSource`] for tests.

use std::sync::{Mutex, PoisonError};

use super::{ListeningProcess, ProcessBrief, ProcessInfo, ProcessSource, ProcessSourceError};

/// A [`ProcessSource`] whose contents the test controls.
#[derive(Debug, Default)]
pub struct FakeProcessSource {
    processes: Mutex<Vec<ListeningProcess>>,
}

impl FakeProcessSource {
    /// Creates a source that reports `processes`.
    #[must_use]
    pub fn new(processes: Vec<ListeningProcess>) -> Self {
        Self {
            processes: Mutex::new(processes),
        }
    }

    /// Replaces what the source reports, as if the host had changed.
    pub fn set(&self, processes: Vec<ListeningProcess>) {
        *self
            .processes
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = processes;
    }
}

impl ProcessSource for FakeProcessSource {
    fn snapshot(&self) -> Result<Vec<ListeningProcess>, ProcessSourceError> {
        Ok(self
            .processes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn all_processes(&self) -> Result<Vec<ProcessBrief>, ProcessSourceError> {
        Ok(self
            .processes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|process| ProcessBrief {
                id: process.info.id,
                ppid: process.info.ppid,
                uid: process.info.uid,
            })
            .collect())
    }

    fn process_info(&self, pid: u32) -> Option<ProcessInfo> {
        self.processes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|process| process.info.id.pid == pid)
            .map(|process| process.info.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ProcessId, ProcessStart};

    fn listening(pid: u32, port: u16) -> ListeningProcess {
        ListeningProcess {
            info: ProcessInfo {
                id: ProcessId {
                    pid,
                    start: ProcessStart(1),
                },
                ppid: 1,
                pgid: pid,
                uid: 501,
                argv: vec!["php".to_owned(), "-S".to_owned()],
                cwd: None,
            },
            ports: vec![port],
        }
    }

    #[test]
    fn reports_what_it_was_given_and_what_it_is_set_to() {
        let source = FakeProcessSource::new(vec![listening(10, 8080)]);
        assert_eq!(source.snapshot().unwrap().len(), 1);
        assert!(source.process_info(10).is_some());
        assert!(source.process_info(11).is_none());

        source.set(vec![listening(11, 8081), listening(12, 8082)]);
        assert_eq!(source.snapshot().unwrap().len(), 2);
        assert!(source.process_info(10).is_none());
        assert_eq!(source.process_info(12).unwrap().id.pid, 12);
    }
}
