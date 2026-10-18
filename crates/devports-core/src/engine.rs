//! The engine: every rule of DevPorts as one deterministic state machine.
//!
//! The engine owns the state (the catalog, the registered instances, the last
//! observation of the host) and answers questions and events with new state plus
//! a list of [`Effect`]s for the caller to carry out (stop a process, show a
//! notification). It never reads the clock, scans the host or touches a process
//! by itself: those come in as data (`observe_host`, a `Clock`) or go out as
//! effects, which is what makes the whole policy testable with a fake clock and
//! fake sources (`PRUEBAS.md`, principles 1 and 3).
//!
//! The engine is not thread-safe on purpose: exactly one actor owns it
//! (ADR-025, point 4).

// Style lints that would only force noisier signatures here: the engine passes
// small owned values around on purpose, and its long methods are flat tables.
#![allow(
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::similar_names,
    clippy::assigning_clones,
    clippy::ref_option
)]

mod actions;
mod binding;
mod leases;
mod run;
mod views;

use std::collections::{BTreeMap, BTreeSet};

use devports_proto::views::{ChainLink, InstanceView};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::catalog::Catalog;
use crate::clock::Clock;
use crate::config::{Config, SharedServices};
use crate::docker::Container;
use crate::ports::PortProbe;
use crate::process::{ListeningProcess, ProcessId, ProcessSource};
use crate::store::{InstanceRecord, Store, StoreError};

pub use actions::ActionError;

/// UI facts captured when a person confirmed stopping one exact instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopExpectation {
    /// Full fingerprint of the target process start or container ID.
    pub target_identity: Option<String>,
    /// Process id, when this is a host process.
    pub pid: Option<u32>,
    /// Container name, when this is a container.
    pub container: Option<String>,
    /// Listening port at confirmation time.
    pub port: Option<u16>,
    /// Owning project at confirmation time.
    pub project: Option<String>,
    /// Service at confirmation time.
    pub service: Option<String>,
    /// App or data kind at confirmation time.
    pub kind: String,
    /// Instance role at confirmation time.
    pub role: devports_proto::views::InstanceRole,
    /// Whether it is a shared service.
    pub shared: bool,
    /// Start time displayed at confirmation time.
    pub started_at: Option<String>,
}

impl StopExpectation {
    /// Captures only the facts needed to revalidate a destructive action.
    #[must_use]
    pub fn from_view(view: &InstanceView) -> Self {
        Self {
            target_identity: view.target_identity.clone(),
            pid: view.pid,
            container: view.container.clone(),
            port: view.port,
            project: view.project.clone(),
            service: view.service.clone(),
            kind: view.kind.clone(),
            role: view.role,
            shared: view.shared,
            started_at: view.started_at.clone(),
        }
    }

    fn matches(&self, view: &InstanceView) -> bool {
        self == &Self::from_view(view)
    }
}

/// How long repository facts (branch, last change) are trusted.
const GIT_TTL_SECONDS: i64 = 10;

/// One instance whose health the prober must check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthTarget {
    /// The instance's reference.
    pub reference: String,
    /// The port to check on localhost.
    pub port: u16,
    /// How to check it.
    pub health: crate::manifest::Health,
}

/// What the engine wants done outside itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Stop a process or container, then call [`Engine::termination_finished`].
    Terminate(TerminationRequest),
    /// Tell the person something.
    Notify(Notice),
}

/// A request to stop one instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminationRequest {
    /// The instance's reference.
    pub reference: String,
    /// What to stop.
    pub target: Target,
    /// Why it is being stopped.
    pub reason: StopReason,
}

/// What a termination acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A host process and its descendants.
    Process(ProcessId),
    /// A container, stopped with `docker stop`; never removed.
    Container(String),
}

impl Target {
    /// Full, opaque fingerprint captured by a stop confirmation. Unlike the
    /// short display reference it remains collision-resistant for revalidation.
    #[must_use]
    pub fn identity(&self) -> String {
        let material = match self {
            Self::Process(id) => format!("process:{}:{}", id.pid, id.start.0),
            Self::Container(id) => format!("container:{id}"),
        };
        crate::hash::sha256_hex(material.as_bytes())
    }
}

/// Why something is being stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The lease of a Temporal ended.
    LeaseExpired,
    /// The caller of a Temporal asked, with its token.
    TokenRequest,
    /// A person asked, from the application.
    PersonRequest,
}

/// Something worth telling the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// A Temporal will expire in about a minute.
    LeaseExpiring {
        /// The instance's reference.
        reference: String,
        /// Project, if any.
        project: Option<String>,
        /// Service.
        service: Option<String>,
        /// Its port.
        port: Option<u16>,
        /// Why the test was started.
        reason: Option<String>,
        /// When the lease ends, seconds since the epoch.
        expires_at: i64,
    },
    /// An unregistered duplicate of a Principal appeared.
    DuplicateDetected {
        /// The duplicate's reference.
        reference: String,
        /// Project it belongs to.
        project: String,
        /// Service it duplicates.
        service: String,
        /// Its port.
        port: Option<u16>,
    },
}

/// How the stopping of one instance ended, as reported back to the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminated {
    /// It stopped.
    Stopped,
    /// It was already gone.
    AlreadyGone,
    /// Something survived `SIGKILL`.
    StillAlive,
    /// The safety rules refused (with the reason, for the log).
    Refused(String),
    /// The container could not be stopped.
    Failed(String),
}

/// Why the engine could not start.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The inputs an engine needs, so tests and production assemble it the same way.
pub struct Parts<C, S, P> {
    /// Time.
    pub clock: C,
    /// The host's processes.
    pub source: S,
    /// Port availability.
    pub ports: P,
    /// User configuration.
    pub config: Config,
    /// Known projects.
    pub catalog: Catalog,
    /// Shared services.
    pub shared: SharedServices,
    /// Persistent state.
    pub store: Store,
    /// Fresh random bytes (tokens and references).
    pub entropy: Box<dyn FnMut() -> [u8; 16] + Send>,
    /// The pid of the DevPorts process, which must never be stopped.
    pub own_pid: u32,
}

/// The state machine.
pub struct Engine<C: Clock, S: ProcessSource, P: PortProbe> {
    pub(crate) clock: C,
    pub(crate) source: S,
    pub(crate) ports: P,
    pub(crate) config: Config,
    pub(crate) catalog: Catalog,
    pub(crate) shared: SharedServices,
    pub(crate) store: Store,
    /// Registered instances that still count, mirroring the store.
    pub(crate) tracked: Vec<InstanceRecord>,
    pub(crate) host: Vec<ListeningProcess>,
    /// `None` while the Docker daemon is unreachable: containers are then
    /// "unknown", never "gone".
    pub(crate) containers: Option<Vec<Container>>,
    /// Consecutive failed health checks, by instance reference.
    pub(crate) health: BTreeMap<String, u8>,
    /// Repository facts by folder, re-read at most every [`GIT_TTL_SECONDS`]: the
    /// views are rebuilt every second and must not touch the disk each time.
    pub(crate) git_cache:
        std::cell::RefCell<BTreeMap<std::path::PathBuf, (i64, Option<crate::gitinfo::GitInfo>)>>,
    /// What the person chose to keep, by [`crate::ignore::rule_key`].
    pub(crate) ignored: BTreeSet<String>,
    /// The Docker context the containers come from; part of a container's rule.
    pub(crate) docker_context: String,
    /// The person's classification corrections, by [`crate::ignore::rule_key`].
    pub(crate) corrections: BTreeMap<String, crate::classify::ProcessClass>,
    pub(crate) views: Vec<InstanceView>,
    /// Changes whenever [`Engine::views`] changes, so a caller can tell cheaply.
    pub(crate) views_revision: u64,
    /// What each listed instance's stop would act on, by reference.
    pub(crate) targets: BTreeMap<String, Target>,
    pub(crate) seen_duplicates: BTreeSet<String>,
    pub(crate) chain_cache: BTreeMap<ProcessId, Vec<ChainLink>>,
    pub(crate) entropy: Box<dyn FnMut() -> [u8; 16] + Send>,
    pub(crate) own_pid: u32,
    pub(crate) last_prune: i64,
    /// Failures the engine could not handle itself (a store write that failed),
    /// for the caller to report.
    pub(crate) problems: Vec<String>,
}

impl<C: Clock, S: ProcessSource, P: PortProbe> Engine<C, S, P> {
    /// Builds the engine, loading the registered instances from the store and
    /// recording that the core started.
    ///
    /// # Errors
    ///
    /// [`EngineError::Store`] if the store cannot be read.
    pub fn new(parts: Parts<C, S, P>) -> Result<Self, EngineError> {
        let tracked = parts.store.live_instances()?;
        let ignored_keys = parts.store.ignored_keys()?;
        let corrections = parts
            .store
            .classification_overrides()?
            .into_iter()
            .filter_map(|(key, class)| {
                crate::classify::ProcessClass::parse(&class).map(|class| (key, class))
            })
            .collect();
        let mut engine = Self {
            clock: parts.clock,
            source: parts.source,
            ports: parts.ports,
            config: parts.config,
            catalog: parts.catalog,
            shared: parts.shared,
            store: parts.store,
            tracked,
            host: Vec::new(),
            containers: None,
            health: BTreeMap::new(),
            ignored: ignored_keys.into_iter().collect(),
            docker_context: crate::ignore::DEFAULT_CONTEXT.to_owned(),
            corrections,
            git_cache: std::cell::RefCell::new(BTreeMap::new()),
            views: Vec::new(),
            views_revision: 0,
            targets: BTreeMap::new(),
            seen_duplicates: BTreeSet::new(),
            chain_cache: BTreeMap::new(),
            entropy: parts.entropy,
            own_pid: parts.own_pid,
            last_prune: 0,
            problems: Vec::new(),
        };
        engine.event(
            "core_started",
            None,
            None,
            None,
            &json!({"pid": engine.own_pid}),
        );
        engine.rebuild_views();
        Ok(engine)
    }

    /// Repository facts for `cwd`, from the cache when they are recent.
    pub(crate) fn git_info(&self, cwd: &std::path::Path) -> Option<crate::gitinfo::GitInfo> {
        let now = self.now();
        let mut cache = self.git_cache.borrow_mut();
        if let Some((read_at, info)) = cache.get(cwd)
            && now.saturating_sub(*read_at) < GIT_TTL_SECONDS
        {
            return info.clone();
        }
        let info = crate::gitinfo::read(cwd);
        cache.insert(cwd.to_path_buf(), (now, info.clone()));
        // Folders of processes that are gone must not pile up.
        if cache.len() > 512 {
            cache.retain(|_, (read_at, _)| now.saturating_sub(*read_at) < GIT_TTL_SECONDS);
        }
        info
    }

    /// The current time in seconds since the epoch.
    pub(crate) fn now(&self) -> i64 {
        self.clock.wall().as_second()
    }

    /// Writes one event to the log. A failed write does not stop the engine (the
    /// log is a record, not the state) but is kept for [`Self::take_problems`].
    pub(crate) fn event(
        &mut self,
        kind: &str,
        project: Option<&str>,
        service: Option<&str>,
        reference: Option<&str>,
        payload: &Value,
    ) {
        let now = self.now();
        if let Err(error) =
            self.store
                .append_event(now, kind, project, service, reference, &payload.to_string())
        {
            self.problems.push(format!(
                "no se pudo escribir en el registro de actividad: {error}"
            ));
        }
    }

    /// Writes a registered instance back to the store, keeping any failure for
    /// [`Self::take_problems`].
    pub(crate) fn persist(&mut self, record: &InstanceRecord) {
        if let Err(error) = self.store.update_instance(record) {
            self.problems.push(format!(
                "no se pudo guardar la instancia {}: {error}",
                record.reference
            ));
        }
    }

    /// The failures the engine could not handle itself since the last call.
    pub fn take_problems(&mut self) -> Vec<String> {
        std::mem::take(&mut self.problems)
    }

    /// The catalog, replaced after a manifest changed.
    pub fn set_catalog(&mut self, catalog: Catalog, shared: SharedServices) {
        self.catalog = catalog;
        self.shared = shared;
        self.rebuild_views();
    }

    /// The configuration in force.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The catalog in force.
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Every instance the engine knows of, as displayed.
    #[must_use]
    pub fn views(&self) -> &[InstanceView] {
        &self.views
    }

    /// The projects as the interface lays them out. Reads the repositories'
    /// branches from disk, so call it sparingly.
    #[must_use]
    pub fn project_summaries(&self) -> Vec<devports_proto::views::ProjectSummary> {
        use devports_proto::views::{ProjectSummary, RepoSummary, ServiceSummary};
        self.catalog
            .projects()
            .iter()
            .map(|project| {
                let mut repos: BTreeMap<std::path::PathBuf, RepoSummary> = BTreeMap::new();
                let mut services = Vec::new();
                for service in &project.services {
                    let folder = service.cwd.clone().unwrap_or_else(|| project.root.clone());
                    let git = crate::gitinfo::read(&folder);
                    let repo_path = git
                        .as_ref()
                        .map_or_else(|| project.root.clone(), |git| git.worktree.clone());
                    let repo_path = if repo_path.starts_with(&project.root) {
                        repo_path
                    } else {
                        project.root.clone()
                    };
                    repos
                        .entry(repo_path.clone())
                        .or_insert_with(|| RepoSummary {
                            name: repo_path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("proyecto")
                                .to_owned(),
                            path: repo_path.display().to_string(),
                            branch: git.and_then(|git| git.branch),
                        });
                    services.push(ServiceSummary {
                        name: service.name.clone(),
                        repo: repo_path.display().to_string(),
                        port: service.port,
                        kind: service.kind.as_str().to_owned(),
                    });
                }
                ProjectSummary {
                    name: project.name.clone(),
                    root: project.root.display().to_string(),
                    uses: project.uses.clone(),
                    repos: repos.into_values().collect(),
                    services,
                }
            })
            .collect()
    }

    /// Records which Docker context the containers come from. Rules for
    /// containers are scoped to it, so the views are rebuilt when it changes.
    pub fn set_docker_context(&mut self, context: &str) {
        if self.docker_context != context {
            context.clone_into(&mut self.docker_context);
            self.rebuild_views();
        }
    }

    /// The services of `shared.yml` with what the person declared for them.
    #[must_use]
    pub fn shared_summaries(&self) -> Vec<devports_proto::views::SharedSummary> {
        self.shared
            .services
            .iter()
            .map(|(name, spec)| devports_proto::views::SharedSummary {
                name: name.clone(),
                start: spec.start.clone(),
                endpoint: match (&spec.health, spec.port) {
                    (crate::manifest::Health::Http(path), Some(port)) => {
                        Some(format!("http://localhost:{port}{path}"))
                    }
                    _ => None,
                },
            })
            .collect()
    }

    /// Changes every time [`Self::views`] changes, and only then.
    #[must_use]
    pub fn views_revision(&self) -> u64 {
        self.views_revision
    }

    /// The names `shared.yml` defines.
    #[must_use]
    pub fn shared_names(&self) -> BTreeSet<String> {
        self.shared.names()
    }

    /// Writes the log line of an adoption done from the application.
    pub fn record_adopted(&mut self, project: &str, service: &str) {
        self.event(
            "adopted",
            Some(project),
            Some(service),
            None,
            &json!({"project": project}),
        );
    }

    /// Records the result of one health check of an instance. Three failures in a
    /// row make it unhealthy; one success clears the count.
    pub fn record_health(&mut self, reference: &str, healthy: bool) {
        let failures = self.health.entry(reference.to_owned()).or_insert(0);
        let before = *failures;
        *failures = if healthy {
            0
        } else {
            failures.saturating_add(1)
        };
        let crossed = (before >= HUNG_AFTER) != (*failures >= HUNG_AFTER);
        if crossed {
            self.rebuild_views();
        }
    }
}

/// Failed checks in a row before an instance counts as hung.
pub(crate) const HUNG_AFTER: u8 = 3;

#[cfg(test)]
mod scenarios;
#[cfg(test)]
mod tests;
