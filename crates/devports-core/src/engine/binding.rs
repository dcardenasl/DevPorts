//! Following registered instances through the host's changes: a reserved port
//! gets its process, a process that disappears ends its instance, an unregistered
//! duplicate is noticed (`ARQUITECTURA.md`, sections 6 and 7).

use serde_json::json;

use super::{Effect, Engine, Notice};
use crate::clock::Clock;
use crate::docker::Container;
use crate::ports::PortProbe;
use crate::process::ancestry::is_self_or_descendant;
use crate::process::{ListeningProcess, ProcessId, ProcessSource, ProcessStart};
use crate::store::{InstanceRecord, State};
use devports_proto::views::InstanceRole;

/// What a reserved port turned into.
enum Binding {
    Process(ProcessId, ProcessDetails),
    Container(String),
    /// Something unrelated holds the port.
    Stolen,
    /// Nothing has opened it yet.
    Pending,
}

/// What is copied from the process into the instance when it is bound.
struct ProcessDetails {
    cwd: Option<String>,
}

impl<C: Clock, S: ProcessSource, P: PortProbe> Engine<C, S, P> {
    /// Takes a new scan of the host's listeners.
    pub fn observe_host(&mut self, scan: Vec<ListeningProcess>) -> Vec<Effect> {
        self.host = scan;
        self.settle()
    }

    /// Takes a new list of containers, or `None` when Docker cannot be reached
    /// (containers are then unknown, never "gone").
    pub fn observe_containers(&mut self, containers: Option<Vec<Container>>) -> Vec<Effect> {
        self.containers = containers;
        self.settle()
    }

    pub(crate) fn settle(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.advance_registered();
        self.rebuild_views();
        self.announce_duplicates(&mut effects);
        effects
    }

    /// Moves every registered instance along: bind, release or end.
    fn advance_registered(&mut self) {
        let now = self.now();
        let grace =
            i64::try_from(self.config.observation.reservation_grace.as_secs()).unwrap_or(i64::MAX);
        let snapshot: Vec<InstanceRecord> = self.tracked.clone();
        for mut record in snapshot {
            let changed = match record.state {
                State::Reserved => self.advance_reserved(&mut record, now, grace),
                State::Running | State::Stopping | State::ExpiredAlive => {
                    self.advance_running(&mut record, now)
                }
                State::Stopped | State::Released | State::Down => false,
            };
            if changed {
                self.persist(&record);
                if let Some(slot) = self
                    .tracked
                    .iter_mut()
                    .find(|candidate| candidate.id == record.id)
                {
                    slot.clone_from(&record);
                }
            }
        }
        self.tracked.retain(|record| record.state.is_live());
    }

    fn advance_reserved(&mut self, record: &mut InstanceRecord, now: i64, grace: i64) -> bool {
        match self.find_binding(record) {
            Binding::Process(id, details) => {
                record.pid = Some(id.pid);
                record.pid_start = Some(id.start.0);
                record.state = State::Running;
                record.started_at = Some(now);
                if details.cwd.is_some() {
                    record.cwd = details.cwd;
                }
                let git = record
                    .cwd
                    .as_deref()
                    .and_then(|cwd| crate::gitinfo::read(std::path::Path::new(cwd)));
                record.branch = git.as_ref().and_then(|git| git.branch.clone());
                record.worktree = git.as_ref().map(|git| git.worktree.display().to_string());
                self.log_bound(record);
                true
            }
            Binding::Container(id) => {
                record.container_id = Some(id);
                record.state = State::Running;
                record.started_at = Some(now);
                self.log_bound(record);
                true
            }
            Binding::Stolen => {
                record.state = State::Released;
                record.ended_at = Some(now);
                record.end_reason = Some("port_stolen".to_owned());
                self.event(
                    "port_stolen",
                    record.project.as_deref(),
                    record.service.as_deref(),
                    Some(&record.reference),
                    &json!({"port": record.port}),
                );
                true
            }
            Binding::Pending => {
                let expired = record
                    .reserved_at
                    .is_some_and(|at| now >= at.saturating_add(grace));
                if expired {
                    record.state = State::Released;
                    record.ended_at = Some(now);
                    record.end_reason = Some("reservation_expired".to_owned());
                    self.event(
                        "reservation_released",
                        record.project.as_deref(),
                        record.service.as_deref(),
                        Some(&record.reference),
                        &json!({"port": record.port}),
                    );
                }
                expired
            }
        }
    }

    fn log_bound(&mut self, record: &InstanceRecord) {
        self.event(
            "bound",
            record.project.as_deref(),
            record.service.as_deref(),
            Some(&record.reference),
            &json!({"port": record.port, "pid": record.pid, "container": record.container_id}),
        );
    }

    fn advance_running(&mut self, record: &mut InstanceRecord, now: i64) -> bool {
        let gone = match (&record.container_id, record.pid, record.pid_start) {
            (Some(id), _, _) => self.containers.as_ref().is_some_and(|containers| {
                !containers
                    .iter()
                    .any(|container| &container.id == id && container.state.is_running())
            }),
            (None, Some(pid), Some(start)) => !self
                .host
                .iter()
                .any(|process| process.info.id.pid == pid && process.info.id.start.0 == start),
            _ => false,
        };
        if !gone {
            return false;
        }
        let was_stopping = record.state == State::Stopping;
        record.state = State::Stopped;
        record.ended_at = Some(now);
        record
            .end_reason
            .get_or_insert_with(|| "process_gone".to_owned());
        if !was_stopping {
            self.event(
                "process_gone",
                record.project.as_deref(),
                record.service.as_deref(),
                Some(&record.reference),
                &json!({"port": record.port}),
            );
        }
        true
    }

    /// Decides what the process on a reserved port is, by the rules of
    /// `ARQUITECTURA.md`, section 7.
    fn find_binding(&self, record: &InstanceRecord) -> Binding {
        let Some(port) = record.port else {
            return Binding::Pending;
        };
        let reserved_at = record.reserved_at.unwrap_or(0);

        // The container the service itself declares wins when it runs: starting
        // an existing container (`docker start`) keeps its old creation time.
        let declared = record
            .service
            .as_deref()
            .and_then(|service| self.declared_container(record.project.as_deref(), service));
        if let Some(containers) = &self.containers
            && let Some(container) = containers.iter().find(|container| {
                container.state.is_running() && declared == Some(container.name.as_str())
            })
        {
            return Binding::Container(container.id.clone());
        }

        // A container publishing the port, created after the reservation, wins:
        // the host process that shows up on the port may be the runtime's proxy.
        if let Some(containers) = &self.containers
            && let Some(container) = containers.iter().find(|container| {
                container.state.is_running()
                    && container.created >= reserved_at.saturating_sub(1)
                    && container
                        .ports
                        .iter()
                        .any(|published| published.host_port == port)
            })
        {
            return Binding::Container(container.id.clone());
        }

        let listeners: Vec<&ListeningProcess> = self
            .host
            .iter()
            .filter(|process| process.ports.contains(&port))
            .collect();
        if listeners.is_empty() {
            return Binding::Pending;
        }
        let started_after = |process: &ListeningProcess| {
            let seconds = process.info.id.start.0.checked_div(1_000_000).unwrap_or(0);
            i64::try_from(seconds).is_ok_and(|seconds| seconds >= reserved_at.saturating_sub(1))
        };
        let launcher = record
            .launcher_pid
            .zip(record.launcher_start)
            .map(|(pid, start)| ProcessId {
                pid,
                start: ProcessStart(start),
            });

        // Strong: the launcher itself (`exec` keeps the pid) or a descendant of it.
        if let Some(launcher) = launcher
            && let Some(process) = listeners.iter().find(|process| {
                started_after(process)
                    && is_self_or_descendant(&self.source, process.info.id.pid, launcher)
            })
        {
            return Binding::Process(process.info.id, details(process));
        }
        // Medium: a process that detached from its launcher but runs inside the
        // project (or the folder `run` was called from).
        let base = record
            .project
            .as_deref()
            .and_then(|name| {
                self.catalog
                    .projects()
                    .iter()
                    .find(|project| project.name == name)
            })
            .map(|project| project.root.clone())
            .or_else(|| record.cwd.as_deref().map(std::path::PathBuf::from));
        if let Some(base) = base
            && let Some(process) = listeners.iter().find(|process| {
                started_after(process)
                    && process
                        .info
                        .cwd
                        .as_deref()
                        .is_some_and(|cwd| cwd.starts_with(&base))
            })
        {
            return Binding::Process(process.info.id, details(process));
        }
        Binding::Stolen
    }

    /// Logs and notifies duplicates that were not there on the previous scan.
    fn announce_duplicates(&mut self, effects: &mut Vec<Effect>) {
        let duplicates: Vec<devports_proto::views::InstanceView> = self
            .views
            .iter()
            .filter(|view| view.role == InstanceRole::Unregistered && view.duplicate_of.is_some())
            .cloned()
            .collect();
        let current: std::collections::BTreeSet<String> = duplicates
            .iter()
            .map(|view| view.instance_ref.clone())
            .collect();
        self.seen_duplicates
            .retain(|reference| current.contains(reference));
        for view in duplicates {
            if !self.seen_duplicates.insert(view.instance_ref.clone()) {
                continue;
            }
            self.event(
                "duplicate_detected",
                view.project.as_deref(),
                view.service.as_deref(),
                Some(&view.instance_ref),
                &json!({"port": view.port, "duplicate_of": view.duplicate_of}),
            );
            if let (Some(project), Some(service)) = (view.project, view.service) {
                effects.push(Effect::Notify(Notice::DuplicateDetected {
                    reference: view.instance_ref,
                    project,
                    service,
                    port: view.port,
                }));
            }
        }
    }
}

fn details(process: &ListeningProcess) -> ProcessDetails {
    ProcessDetails {
        cwd: process
            .info
            .cwd
            .as_ref()
            .map(|path| path.display().to_string()),
    }
}
