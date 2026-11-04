//! `doctor`: why a process does or does not belong to a declared service
//! (ADR-044). Read-only and answered from what the engine already holds.

use std::path::Path;

use devports_proto::doctor::{
    DoctorArgs, DoctorCandidate, DoctorMismatch, DoctorProcess, DoctorProject, DoctorResult,
    DoctorVerdict,
};

use super::Engine;
use crate::catalog::{Classification, Service};
use crate::clock::Clock;
use crate::fingerprint::Fingerprint;
use crate::ports::PortProbe;
use crate::process::{ListeningProcess, ProcessSource};

impl<C: Clock, S: ProcessSource, P: PortProbe> Engine<C, S, P> {
    /// The search settings, the projects found and, for the port or pid asked
    /// about, the verdict on each listening process.
    pub(crate) fn doctor(&self, args: &DoctorArgs) -> DoctorResult {
        let wanted = |process: &&ListeningProcess| {
            args.port.is_some_and(|port| process.ports.contains(&port))
                || args.pid.is_some_and(|pid| process.info.id.pid == pid)
        };
        DoctorResult {
            roots: self
                .config
                .roots
                .iter()
                .map(|root| root.display().to_string())
                .collect(),
            max_depth: u32::try_from(self.config.max_depth).unwrap_or(u32::MAX),
            projects: self
                .catalog
                .projects()
                .iter()
                .map(|project| DoctorProject {
                    name: project.name.clone(),
                    root: project.root.display().to_string(),
                    services: project
                        .services
                        .iter()
                        .map(|service| service.name.clone())
                        .collect(),
                })
                .collect(),
            processes: self
                .host
                .iter()
                .filter(wanted)
                .map(|process| self.diagnose(process))
                .collect(),
        }
    }

    fn diagnose(&self, process: &ListeningProcess) -> DoctorProcess {
        let command = Fingerprint::from_argv(&process.info.argv);
        let cwd = process.info.cwd.as_deref();
        let verdict = match self.catalog.classify(process) {
            Classification::Service { project, service } => DoctorVerdict::Matched {
                project: project.name.clone(),
                service: service.name.clone(),
            },
            Classification::NoProject => {
                if cwd.is_none() {
                    DoctorVerdict::NoFolder
                } else {
                    DoctorVerdict::OutsideProjects
                }
            }
            Classification::UnregisteredInProject { project } => DoctorVerdict::NoService {
                project: project.name.clone(),
                candidates: project
                    .services
                    .iter()
                    .filter_map(|service| {
                        mismatch(service, cwd, &command).map(|reason| DoctorCandidate {
                            service: service.name.clone(),
                            reason,
                        })
                    })
                    .collect(),
                port_of: project
                    .services
                    .iter()
                    .find(|service| {
                        service
                            .port
                            .is_some_and(|port| process.ports.contains(&port))
                    })
                    .map(|service| service.name.clone()),
            },
            Classification::Ambiguous {
                project,
                candidates,
            } => DoctorVerdict::Ambiguous {
                project: project.name.clone(),
                services: candidates
                    .iter()
                    .map(|service| service.name.clone())
                    .collect(),
            },
        };
        DoctorProcess {
            pid: process.info.id.pid,
            ports: process.ports.clone(),
            cwd: cwd.map(|dir| dir.display().to_string()),
            fingerprint: command.render(),
            verdict,
        }
    }
}

/// Why `service` does not accept a process that runs in `cwd` with `command`;
/// `None` when it would accept it (the same two tests as `Catalog::classify`).
fn mismatch(
    service: &Service,
    cwd: Option<&Path>,
    command: &Fingerprint,
) -> Option<DoctorMismatch> {
    let Some(service_cwd) = service.cwd.as_deref() else {
        return Some(DoctorMismatch::ContainerService);
    };
    if !cwd.is_some_and(|dir| dir.starts_with(service_cwd)) {
        return Some(DoctorMismatch::FolderOutside {
            service_cwd: service_cwd.display().to_string(),
        });
    }
    match service.fingerprint.as_ref() {
        Some(needle) if !command.contains(needle) => Some(DoctorMismatch::CommandDiffers {
            expected: needle.render(),
            observed: command.render(),
        }),
        _ => None,
    }
}
