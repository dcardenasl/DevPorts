//! Turning state and observation into the instances people see.
//!
//! Instances come from two places: *registered* ones (the store: Principals and
//! Temporals that went through `devports run`) and *observed* ones (whatever
//! listens on the host or runs in Docker right now). Observed things that no
//! registered instance claims are grouped by service; in each group one is the
//! Principal "de hecho" and the rest are duplicates (`ARQUITECTURA.md`, sec. 5).

use std::collections::{BTreeMap, BTreeSet};

use devports_proto::views::{ChainLink, InstanceRole, InstanceState, InstanceView};

use super::{Engine, HUNG_AFTER, HealthTarget, Target};
use crate::catalog::{Classification, ContainerClassification};
use crate::clock::Clock;
use crate::docker::Container;
use crate::fingerprint::Fingerprint;
use crate::ids::{container_reference, observed_reference};
use crate::manifest::{Health, ServiceKind};
use crate::messages;
use crate::ports::PortProbe;
use crate::process::{ListeningProcess, ProcessId, ProcessInfo, ProcessSource};
use crate::store::{InstanceRecord, Role, State};

/// A thing seen on the host, not (yet) claimed by a registered instance.
struct Candidate {
    reference: String,
    target: Target,
    ports: Vec<u16>,
    /// When it started, seconds since the epoch; for "oldest wins".
    started: i64,
    view: InstanceView,
}

/// Workers that inherit their parent's listening socket (the pools of
/// `php-fpm`, the workers of a cluster) are one server, not several: only the
/// process that owns the socket is listed, and stopping it takes the workers too.
fn shares_socket_with_parent(process: &ListeningProcess, all: &[ListeningProcess]) -> bool {
    all.iter().any(|parent| {
        parent.info.id.pid == process.info.ppid
            && parent.info.id != process.info.id
            && parent.ports.iter().any(|port| process.ports.contains(port))
    })
}

fn is_system_process(info: &ProcessInfo) -> bool {
    info.argv
        .first()
        .is_some_and(|program| crate::process::is_system_program(program))
}

impl<C: Clock, S: ProcessSource, P: PortProbe> Engine<C, S, P> {
    /// Recomputes [`Engine::views`] from the registered instances, the last host
    /// scan and the last container list.
    pub(crate) fn rebuild_views(&mut self) {
        let mut views: Vec<InstanceView> = Vec::new();
        let mut targets: BTreeMap<String, Target> = BTreeMap::new();
        let mut claimed_processes: BTreeSet<ProcessId> = BTreeSet::new();
        let mut claimed_containers: BTreeSet<String> = BTreeSet::new();

        for record in &self.tracked {
            let (view, target) = self.registered_view(record);
            if let (Some(pid), Some(start)) = (record.pid, record.pid_start) {
                claimed_processes.insert(ProcessId {
                    pid,
                    start: crate::process::ProcessStart(start),
                });
            }
            if let Some(container) = &record.container_id {
                claimed_containers.insert(container.clone());
            }
            if let Some(target) = target {
                targets.insert(view.instance_ref.clone(), target);
            }
            views.push(view);
        }

        // Observed, unclaimed things, grouped by the service they belong to.
        let mut groups: BTreeMap<(Option<String>, String), Vec<Candidate>> = BTreeMap::new();
        let mut loose: Vec<Candidate> = Vec::new();
        let mut chains: BTreeMap<ProcessId, Vec<ChainLink>> = BTreeMap::new();

        for process in &self.host {
            if claimed_processes.contains(&process.info.id)
                || is_system_process(&process.info)
                || shares_socket_with_parent(process, &self.host)
            {
                continue;
            }
            let chain = self
                .chain_cache
                .get(&process.info.id)
                .cloned()
                .unwrap_or_else(|| self.chain_of(&process.info));
            chains.insert(process.info.id, chain.clone());
            let (key, mut candidate) = self.host_candidate(process, chain);
            if let Some(key) = key {
                groups.entry(key).or_default().push(candidate);
            } else {
                candidate.view.role = InstanceRole::Unregistered;
                loose.push(candidate);
            }
        }
        if let Some(containers) = &self.containers {
            for container in containers {
                if claimed_containers.contains(&container.id) || !container.state.is_running() {
                    continue;
                }
                let (key, mut candidate) = self.container_candidate(container);
                if let Some(key) = key {
                    groups.entry(key).or_default().push(candidate);
                } else {
                    candidate.view.role = InstanceRole::Unregistered;
                    loose.push(candidate);
                }
            }
        }
        self.chain_cache = chains;

        let tracked_principals: BTreeMap<(Option<String>, String), String> = views
            .iter()
            .filter(|view| {
                view.role == InstanceRole::Principal
                    && matches!(
                        view.state,
                        InstanceState::Healthy | InstanceState::Unhealthy | InstanceState::Starting
                    )
            })
            .filter_map(|view| {
                Some((
                    (view.project.clone(), view.service.clone()?),
                    view.instance_ref.clone(),
                ))
            })
            .collect();

        for (key, mut candidates) in groups {
            let principal_ref = tracked_principals.get(&key).cloned().or_else(|| {
                // No registered Principal: the one on the preferred port, or the
                // oldest, is the Principal "de hecho".
                let preferred = self.preferred_port(&key.0, &key.1);
                candidates.sort_by(|a, b| {
                    let a_hit = preferred.is_some_and(|port| a.ports.contains(&port));
                    let b_hit = preferred.is_some_and(|port| b.ports.contains(&port));
                    b_hit
                        .cmp(&a_hit)
                        .then(a.started.cmp(&b.started))
                        .then(a.reference.cmp(&b.reference))
                });
                candidates.first().map(|first| first.reference.clone())
            });
            for candidate in candidates {
                let mut view = candidate.view;
                if Some(&candidate.reference) == principal_ref.as_ref()
                    && !tracked_principals.contains_key(&key)
                {
                    view.role = InstanceRole::Principal;
                } else {
                    view.role = InstanceRole::Unregistered;
                    view.duplicate_of.clone_from(&principal_ref);
                }
                targets.insert(candidate.reference, candidate.target);
                views.push(view);
            }
        }
        for candidate in loose {
            targets.insert(candidate.reference, candidate.target);
            views.push(candidate.view);
        }

        self.add_declared_but_down(&mut views);
        for view in &mut views {
            view.ignored = view.role == InstanceRole::Unregistered
                && super::ignore_key(view).is_some_and(|key| self.ignored.contains(&key));
        }
        views.sort_by(sort_key);
        if views != self.views {
            self.views = views;
            self.views_revision = self.views_revision.wrapping_add(1);
        }
        self.targets = targets;
    }

    fn preferred_port(&self, project: &Option<String>, service: &str) -> Option<u16> {
        match project {
            Some(name) => self
                .catalog
                .projects()
                .iter()
                .find(|candidate| &candidate.name == name)
                .and_then(|project| {
                    project
                        .services
                        .iter()
                        .find(|candidate| candidate.name == service)
                })
                .and_then(|service| service.port),
            None => self.shared.services.get(service).and_then(|spec| spec.port),
        }
    }

    /// "Arrancó antes del último commit o checkout" when the repository changed
    /// after the process started.
    fn stale_note(&self, cwd: &str, started: i64) -> Option<String> {
        let git = self.git_info(std::path::Path::new(cwd))?;
        stale_from(git.last_change?, started)
    }

    /// What the health prober must check now: running instances with a port.
    #[must_use]
    pub fn health_targets(&self) -> Vec<HealthTarget> {
        self.views
            .iter()
            .filter(|view| {
                view.instance_ref != "-"
                    && view.registered
                    && matches!(
                        view.state,
                        InstanceState::Healthy | InstanceState::Unhealthy
                    )
            })
            .filter_map(|view| {
                let port = view.port?;
                let health = view
                    .service
                    .as_deref()
                    .and_then(|service| self.declared_health(view.project.as_deref(), service))
                    .unwrap_or_default();
                Some(HealthTarget {
                    reference: view.instance_ref.clone(),
                    port,
                    health,
                })
            })
            .collect()
    }

    fn declared_health(&self, project: Option<&str>, service: &str) -> Option<Health> {
        match project {
            Some(name) => self
                .catalog
                .projects()
                .iter()
                .find(|candidate| candidate.name == name)?
                .services
                .iter()
                .find(|candidate| candidate.name == service)
                .map(|service| service.health.clone()),
            None => self
                .shared
                .services
                .get(service)
                .map(|spec| spec.health.clone()),
        }
    }

    /// The container a service declares with `docker:`, if it is a container service.
    pub(super) fn declared_container(&self, project: Option<&str>, service: &str) -> Option<&str> {
        match project {
            Some(name) => self
                .catalog
                .projects()
                .iter()
                .find(|candidate| candidate.name == name)?
                .services
                .iter()
                .find(|candidate| candidate.name == service)?
                .docker
                .as_deref(),
            None => self.shared.services.get(service)?.docker.as_deref(),
        }
    }

    fn service_kind(&self, project: Option<&str>, service: &str) -> ServiceKind {
        match project {
            Some(name) => self
                .catalog
                .projects()
                .iter()
                .find(|candidate| candidate.name == name)
                .and_then(|project| {
                    project
                        .services
                        .iter()
                        .find(|candidate| candidate.name == service)
                })
                .map_or(ServiceKind::App, |service| service.kind),
            None => self
                .shared
                .services
                .get(service)
                .map_or(ServiceKind::App, |spec| spec.kind),
        }
    }

    /// The view of a registered instance, and what stopping it would act on.
    fn registered_view(&self, record: &InstanceRecord) -> (InstanceView, Option<Target>) {
        let host_alive = match (record.pid, record.pid_start) {
            (Some(pid), Some(start)) => self
                .host
                .iter()
                .any(|process| process.info.id.pid == pid && process.info.id.start.0 == start),
            _ => false,
        };
        let container = record.container_id.as_ref().and_then(|id| {
            self.containers
                .as_ref()
                .and_then(|containers| containers.iter().find(|candidate| &candidate.id == id))
        });
        let alive = host_alive
            || container.is_some_and(|container| container.state.is_running())
            // Docker unreachable: its containers are unknown, not gone.
            || (record.container_id.is_some() && self.containers.is_none());

        let hung = self.health.get(&record.reference).copied().unwrap_or(0) >= HUNG_AFTER;
        let state = match record.state {
            State::Reserved => InstanceState::Starting,
            State::Stopping => InstanceState::Stopping,
            State::ExpiredAlive => InstanceState::Forgotten,
            State::Running if !alive => InstanceState::Down,
            State::Running if hung => InstanceState::Unhealthy,
            State::Running => InstanceState::Healthy,
            State::Stopped | State::Released | State::Down => InstanceState::Down,
        };
        let role = match record.role {
            Role::Principal => InstanceRole::Principal,
            Role::Temporal => InstanceRole::Temporal,
            Role::Unregistered => InstanceRole::Unregistered,
        };
        let service = record.service.clone();
        let shared = record.project.is_none()
            && service
                .as_ref()
                .is_some_and(|name| self.shared.services.contains_key(name));
        let kind = service.as_deref().map_or(ServiceKind::App, |name| {
            self.service_kind(record.project.as_deref(), name)
        });
        let chain: Vec<ChainLink> =
            serde_json::from_str(&record.launcher_chain).unwrap_or_default();
        let target = match (record.pid, record.pid_start, &record.container_id) {
            (_, _, Some(id)) => Some(Target::Container(id.clone())),
            (Some(pid), Some(start), None) => Some(Target::Process(ProcessId {
                pid,
                start: crate::process::ProcessStart(start),
            })),
            _ => None,
        };
        let stale = record
            .cwd
            .as_deref()
            .zip(record.started_at)
            .and_then(|(cwd, started)| self.stale_note(cwd, started));
        let view = InstanceView {
            instance_ref: record.reference.clone(),
            project: record.project.clone(),
            service,
            role,
            state,
            kind: kind.as_str().to_owned(),
            port: record.port,
            url: record.port.map(url),
            pid: record.pid,
            container: container.map(|container| container.name.clone()),
            branch: record.branch.clone(),
            worktree: record.worktree.clone(),
            cwd: record.cwd.clone(),
            started_at: record.started_at.map(messages::iso),
            expires_at: record.expires_at.map(messages::iso),
            reason: record.reason.clone(),
            registered: true,
            launcher_chain: chain,
            duplicate_of: None,
            stale,
            shared,
            compose: None,
            argv: self.argv_of(record),
            image: container.map(|container| container.image.clone()),
            ignored: false,
        };
        (view, target)
    }

    /// A listener on the host, with the key of the service it belongs to if any.
    fn host_candidate(
        &self,
        process: &ListeningProcess,
        chain: Vec<ChainLink>,
    ) -> (Option<(Option<String>, String)>, Candidate) {
        let info = &process.info;
        let reference = observed_reference(info.id);
        let started =
            i64::try_from(info.id.start.0.checked_div(1_000_000).unwrap_or(0)).unwrap_or(0);
        let cwd = info.cwd.as_ref().map(|path| path.display().to_string());
        let git = info.cwd.as_deref().and_then(|cwd| self.git_info(cwd));
        let port = process.ports.first().copied();

        let mut view = InstanceView {
            instance_ref: reference.clone(),
            project: None,
            service: None,
            role: InstanceRole::Unregistered,
            state: self.observed_state(&reference),
            kind: "app".to_owned(),
            port,
            url: port.map(url),
            pid: Some(info.id.pid),
            container: None,
            branch: git.as_ref().and_then(|git| git.branch.clone()),
            worktree: git.as_ref().map(|git| git.worktree.display().to_string()),
            cwd,
            started_at: Some(messages::iso(started)),
            expires_at: None,
            reason: None,
            registered: false,
            launcher_chain: chain,
            duplicate_of: None,
            stale: git
                .as_ref()
                .and_then(|git| git.last_change)
                .and_then(|change| stale_from(change, started)),
            shared: false,
            compose: None,
            argv: info.argv.clone(),
            image: None,
            ignored: false,
        };

        let key = match self.catalog.classify(process) {
            Classification::Service { project, service } => {
                view.project = Some(project.name.clone());
                view.service = Some(service.name.clone());
                view.kind = service.kind.as_str().to_owned();
                Some((Some(project.name.clone()), service.name.clone()))
            }
            Classification::UnregisteredInProject { project }
            | Classification::Ambiguous { project, .. } => {
                view.project = Some(project.name.clone());
                None
            }
            Classification::NoProject => self.shared_host_service(process).map(|name| {
                view.service = Some(name.clone());
                view.shared = true;
                view.kind = self.service_kind(None, &name).as_str().to_owned();
                (None, name)
            }),
        };
        let candidate = Candidate {
            reference,
            target: Target::Process(info.id),
            ports: process.ports.clone(),
            started,
            view,
        };
        (key, candidate)
    }

    /// A shared host service (`shared.yml` with `cwd`) that `process` is an instance of.
    fn shared_host_service(&self, process: &ListeningProcess) -> Option<String> {
        let cwd = process.info.cwd.as_deref()?;
        let command = Fingerprint::from_argv(&process.info.argv);
        self.shared.services.iter().find_map(|(name, spec)| {
            let dir = spec.cwd.as_deref()?;
            let dir = crate::discovery::expand_tilde(dir, &self.home());
            let inside = cwd.starts_with(&dir);
            let matches = spec
                .match_text
                .as_deref()
                .is_none_or(|text| command.contains(&Fingerprint::from_match_text(text)));
            (inside && matches).then(|| name.clone())
        })
    }

    fn home(&self) -> std::path::PathBuf {
        self.config
            .roots
            .first()
            .and_then(|root| root.parent())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default()
    }

    /// A running container, with the key of the service it belongs to if any.
    fn container_candidate(
        &self,
        container: &Container,
    ) -> (Option<(Option<String>, String)>, Candidate) {
        let reference = container_reference(&container.id);
        let ports: Vec<u16> = container.ports.iter().map(|port| port.host_port).collect();
        let port = ports.first().copied();
        let mut view = InstanceView {
            instance_ref: reference.clone(),
            project: None,
            service: None,
            role: InstanceRole::Unregistered,
            state: self.observed_state(&reference),
            kind: if crate::adopt::is_data_image(&container.image) {
                "data"
            } else {
                "app"
            }
            .to_owned(),
            port,
            url: port.map(url),
            pid: None,
            container: Some(container.name.clone()),
            branch: None,
            worktree: None,
            cwd: container
                .compose
                .as_ref()
                .and_then(|compose| compose.working_dir.as_ref())
                .map(|dir| dir.display().to_string()),
            started_at: Some(messages::iso(container.created)),
            expires_at: None,
            reason: None,
            registered: false,
            launcher_chain: Vec::new(),
            duplicate_of: None,
            stale: None,
            shared: false,
            compose: None,
            argv: Vec::new(),
            image: Some(container.image.clone()),
            ignored: false,
        };

        let shared_name = self
            .shared
            .services
            .iter()
            .find(|(_, spec)| spec.docker.as_deref() == Some(container.name.as_str()))
            .map(|(name, _)| name.clone());
        let key = if let Some(name) = shared_name {
            view.service = Some(name.clone());
            view.shared = true;
            view.kind = self.service_kind(None, &name).as_str().to_owned();
            Some((None, name))
        } else {
            match self.catalog.classify_container(container) {
                ContainerClassification::Service { project, service } => {
                    view.project = Some(project.name.clone());
                    view.service = Some(service.name.clone());
                    view.kind = service.kind.as_str().to_owned();
                    Some((Some(project.name.clone()), service.name.clone()))
                }
                ContainerClassification::InProject { project } => {
                    view.project = Some(project.name.clone());
                    None
                }
                ContainerClassification::Compose { name, .. } => {
                    view.compose = Some(name.to_owned());
                    None
                }
                ContainerClassification::Ambiguous { .. } | ContainerClassification::NoProject => {
                    None
                }
            }
        };
        let candidate = Candidate {
            reference,
            target: Target::Container(container.id.clone()),
            ports,
            started: container.created,
            view,
        };
        (key, candidate)
    }

    fn observed_state(&self, reference: &str) -> InstanceState {
        if self.health.get(reference).copied().unwrap_or(0) >= HUNG_AFTER {
            InstanceState::Unhealthy
        } else {
            InstanceState::Healthy
        }
    }

    /// Services declared in a manifest (or `shared.yml`) with nothing running show
    /// as a Principal that is down, so "what should be running" is visible.
    fn add_declared_but_down(&self, views: &mut Vec<InstanceView>) {
        let present: BTreeSet<(Option<String>, String)> = views
            .iter()
            .filter(|view| view.role == InstanceRole::Principal)
            .filter_map(|view| Some((view.project.clone(), view.service.clone()?)))
            .collect();
        let mut down = |project: Option<&str>,
                        name: &str,
                        port: Option<u16>,
                        kind: ServiceKind,
                        cwd: Option<String>,
                        container: Option<String>| {
            if present.contains(&(project.map(str::to_owned), name.to_owned())) {
                return;
            }
            views.push(InstanceView {
                instance_ref: "-".to_owned(),
                project: project.map(str::to_owned),
                service: Some(name.to_owned()),
                role: InstanceRole::Principal,
                // Docker unreachable: a container service is unknown, not down.
                state: if container.is_some() && self.containers.is_none() {
                    InstanceState::Unknown
                } else {
                    InstanceState::Down
                },
                kind: kind.as_str().to_owned(),
                port,
                url: port.map(url),
                pid: None,
                container,
                branch: None,
                worktree: None,
                cwd,
                started_at: None,
                expires_at: None,
                reason: None,
                registered: false,
                launcher_chain: Vec::new(),
                duplicate_of: None,
                stale: None,
                shared: project.is_none(),
                compose: None,
                argv: Vec::new(),
                image: None,
                ignored: false,
            });
        };
        for project in self.catalog.projects() {
            for service in &project.services {
                down(
                    Some(&project.name),
                    &service.name,
                    service.port,
                    service.kind,
                    service.cwd.as_ref().map(|dir| dir.display().to_string()),
                    service.docker.clone(),
                );
            }
        }
        for (name, spec) in &self.shared.services {
            down(
                None,
                name,
                spec.port,
                spec.kind,
                spec.cwd.clone(),
                spec.docker.clone(),
            );
        }
    }

    /// The command line of the process a registered instance is bound to.
    fn argv_of(&self, record: &InstanceRecord) -> Vec<String> {
        match (record.pid, record.pid_start) {
            (Some(pid), Some(start)) => self
                .host
                .iter()
                .find(|process| process.info.id.pid == pid && process.info.id.start.0 == start)
                .map(|process| process.info.argv.clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// Who started a process, nearest first, with friendly labels for known agents.
    pub(crate) fn chain_of(&self, info: &ProcessInfo) -> Vec<ChainLink> {
        crate::process::ancestry::chain(&self.source, info.id.pid)
            .iter()
            .map(|ancestor| {
                let name = program_name(ancestor);
                ChainLink {
                    pid: ancestor.id.pid,
                    label: self.agent_label(ancestor, &name),
                    name,
                }
            })
            .collect()
    }

    fn agent_label(&self, info: &ProcessInfo, name: &str) -> Option<String> {
        let command = info.argv.join(" ").to_lowercase();
        self.config
            .known_agents
            .iter()
            .find(|(key, _)| {
                let key = key.to_lowercase();
                name.to_lowercase() == key
                    || command.contains(&format!("/{key}"))
                    || command.contains(&format!(" {key} "))
            })
            .map(|(_, label)| label.clone())
    }
}

/// The base name of a process's program.
fn program_name(info: &ProcessInfo) -> String {
    info.argv
        .first()
        .and_then(|program| program.rsplit('/').find(|part| !part.is_empty()))
        .map_or_else(|| format!("pid {}", info.id.pid), str::to_owned)
}

fn url(port: u16) -> String {
    format!("http://localhost:{port}")
}

fn stale_from(last_change: i64, started: i64) -> Option<String> {
    (last_change > started).then(|| {
        format!(
            "arrancó antes del último commit o checkout ({})",
            messages::clock(last_change)
        )
    })
}

fn sort_key(a: &InstanceView, b: &InstanceView) -> std::cmp::Ordering {
    let role_order = |view: &InstanceView| match view.role {
        InstanceRole::Principal => 0_u8,
        InstanceRole::Temporal => 1,
        InstanceRole::Unregistered => 2,
    };
    (
        a.project.is_none(),
        &a.project,
        &a.service,
        role_order(a),
        a.port,
        &a.instance_ref,
    )
        .cmp(&(
            b.project.is_none(),
            &b.project,
            &b.service,
            role_order(b),
            b.port,
            &b.instance_ref,
        ))
}
