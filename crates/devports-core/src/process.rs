//! Observation of the host's listening processes.
//!
//! The core never calls the operating system directly. It talks to a
//! [`ProcessSource`], and only the macOS implementation exists in v1
//! (`ARQUITECTURA.md`, section 3; ADR-016). Tests use [`fake::FakeProcessSource`].

pub mod ancestry;
pub mod fake;

use std::path::PathBuf;

/// When a process started, in microseconds since the Unix epoch.
///
/// Together with the pid it identifies a process: a pid alone can be reused by
/// the operating system, a `(pid, start)` pair cannot (`ARQUITECTURA.md`,
/// section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessStart(pub u64);

/// Identity of one process: its pid and the moment it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId {
    /// Operating-system process id.
    pub pid: u32,
    /// Start time; makes the pair immune to pid reuse.
    pub start: ProcessStart,
}

/// What the operating system reports about one process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    /// Identity of the process.
    pub id: ProcessId,
    /// Parent process id.
    pub ppid: u32,
    /// Process group id.
    pub pgid: u32,
    /// Owning user id.
    pub uid: u32,
    /// Full command line, `argv[0]` first. Empty when it could not be read.
    pub argv: Vec<String>,
    /// Working directory, when it could be read.
    pub cwd: Option<PathBuf>,
}

/// A process that has at least one TCP socket in the `LISTEN` state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListeningProcess {
    /// The process itself.
    pub info: ProcessInfo,
    /// Local TCP ports it listens on, sorted and without repeats.
    pub ports: Vec<u16>,
}

/// Why a [`ProcessSource`] could not produce a snapshot.
#[derive(Debug, thiserror::Error)]
pub enum ProcessSourceError {
    /// The list of processes could not be obtained at all.
    #[error("could not list processes: {0}")]
    Enumerate(String),
    /// The identity of the current user could not be determined.
    #[error("could not determine the current user: {0}")]
    CurrentUser(String),
}

/// Source of truth about the host's listening processes.
pub trait ProcessSource: Send + Sync {
    /// Every process of the current user that listens on a TCP port.
    ///
    /// A process that disappears while the snapshot is taken is skipped, not an
    /// error: for development servers that is routine.
    ///
    /// # Errors
    ///
    /// Fails only when the whole listing is unavailable.
    fn snapshot(&self) -> Result<Vec<ListeningProcess>, ProcessSourceError>;

    /// What is known about one process of the current user, or `None` if it is
    /// gone or not readable.
    fn process_info(&self, pid: u32) -> Option<ProcessInfo>;
}
