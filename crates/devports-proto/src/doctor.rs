//! Types of `doctor`, the read-only diagnosis of why a process does or does not
//! belong to a declared service (ADR-044).
//!
//! Everything here is a fact the core already holds: folders, names and
//! normalised command lines. No token, no hash. The CLI writes the sentences.

use serde::{Deserialize, Serialize};

/// Arguments of `doctor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorArgs {
    /// Explain the processes listening on this port.
    pub port: Option<u16>,
    /// Explain the process with this id.
    pub pid: Option<u32>,
}

/// Result of `doctor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorResult {
    /// Folders searched for projects.
    pub roots: Vec<String>,
    /// How many levels below a root are searched.
    pub max_depth: u32,
    /// The projects found.
    pub projects: Vec<DoctorProject>,
    /// The processes asked about, one entry each.
    pub processes: Vec<DoctorProcess>,
}

/// A project the catalog knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorProject {
    /// Project name.
    pub name: String,
    /// Its folder.
    pub root: String,
    /// Names of its declared services.
    pub services: Vec<String>,
}

/// One process asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorProcess {
    /// Process id.
    pub pid: u32,
    /// Ports it listens on.
    pub ports: Vec<u16>,
    /// Its working directory, when the system said.
    pub cwd: Option<String>,
    /// Its command line, normalised the way a manifest `match` is.
    pub fingerprint: String,
    /// What the catalog decided.
    pub verdict: DoctorVerdict,
}

/// What the catalog decided about a process, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum DoctorVerdict {
    /// It is this service.
    Matched {
        /// The project.
        project: String,
        /// The service.
        service: String,
    },
    /// The system did not report a working directory, so no folder can match.
    NoFolder,
    /// Its folder is inside no known project.
    OutsideProjects,
    /// It is inside a project but no service accepts it.
    NoService {
        /// The project its folder belongs to.
        project: String,
        /// Why each declared service was discarded.
        candidates: Vec<DoctorCandidate>,
        /// The service that declares the port it listens on, if any.
        port_of: Option<String>,
    },
    /// Several services accept it equally.
    Ambiguous {
        /// The project.
        project: String,
        /// The services that tie.
        services: Vec<String>,
    },
}

/// Why one service did not accept a process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCandidate {
    /// The service.
    pub service: String,
    /// Why not.
    pub reason: DoctorMismatch,
}

/// The reason one service did not accept a process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum DoctorMismatch {
    /// The service runs in a container; a host process is never it.
    ContainerService,
    /// The process runs outside the service's folder.
    FolderOutside {
        /// The service's folder.
        service_cwd: String,
    },
    /// Right folder, different command.
    CommandDiffers {
        /// What the manifest's `match` normalises to.
        expected: String,
        /// What the process normalises to.
        observed: String,
    },
}
