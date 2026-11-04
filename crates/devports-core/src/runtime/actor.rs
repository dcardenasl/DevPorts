//! The actor: sole owner of the engine and of everything that changes it.
use super::{
    ActionError, AdoptionBook, Applied, Arc, CatalogUpdate, Clock, DockerProblem, Effect, Engine,
    EngineContainerSource, MAX_ENGINE_PROBLEMS, Msg, Notice, OsSignaler, Outcome, PathBuf,
    PortProbe, Problem, ProcessSource, ProjectSummary, Refusal, SUMMARY_EVERY_SECONDS,
    SharedSummary, Snapshot, StopExpectation, StopFailure, StopScope, TICK, Target, Terminated,
    TerminationRequest, Timings, attention_items, broadcast, mpsc, oneshot, protected_pids,
    terminate, watch,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DockerState {
    pub(super) reachable: bool,
    pub(super) error: Option<DockerProblem>,
}

pub(super) struct Actor<C: Clock, S: ProcessSource + 'static, P: PortProbe> {
    pub(super) engine: Engine<C, Arc<S>, P>,
    pub(super) inbox: mpsc::Receiver<Msg>,
    pub(super) messages: mpsc::Sender<Msg>,
    pub(super) snapshot: watch::Sender<Arc<Snapshot>>,
    pub(super) notices: broadcast::Sender<Notice>,
    pub(super) docker: watch::Receiver<Option<Arc<EngineContainerSource>>>,
    pub(super) source: Arc<S>,
    pub(super) own_pid: u32,
    pub(super) revision: u32,
    pub(super) docker_state: DockerState,
    pub(super) catalog_problems: Vec<Problem>,
    pub(super) shutdown: watch::Receiver<bool>,
    pub(super) projects: Vec<ProjectSummary>,
    pub(super) shared: Vec<SharedSummary>,
    pub(super) projects_at: i64,
    /// The engine's views revision of the last snapshot sent.
    pub(super) published_views: u64,
    /// Something other than the views changed since the last snapshot.
    pub(super) dirty: bool,
    /// What the engine could not do, kept for the interface.
    pub(super) engine_problems: Vec<Problem>,
    pub(super) pending_stops:
        std::collections::BTreeMap<String, oneshot::Sender<Result<(), ActionError>>>,
    pub(super) adoptions: AdoptionBook,
    pub(super) adoption_waiters: std::collections::BTreeMap<
        String,
        Vec<oneshot::Sender<Result<Applied, crate::adopt::review::AdoptionError>>>,
    >,
    pub(super) home: Arc<PathBuf>,
}

impl<C: Clock + 'static, S: ProcessSource + 'static, P: PortProbe + 'static> Actor<C, S, P> {
    pub(super) async fn run(mut self) {
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        self.publish();
        loop {
            let wake = tokio::select! {
                _ = self.shutdown.wait_for(|stopped| *stopped) => break,
                _ = ticker.tick() => None,
                message = self.inbox.recv() => match message {
                    Some(message) => Some(message),
                    None => break,
                },
            };
            let effects = match wake {
                Some(message) => self.handle(message),
                None => self.engine.tick(),
            };
            self.execute(effects);
            self.publish();
        }
    }

    pub(super) fn handle(&mut self, message: Msg) -> Vec<Effect> {
        match message {
            Msg::Adoption(message) => {
                self.handle_adoption(message);
                Vec::new()
            }
            Msg::Request(request, reply) => {
                let (response, effects) = self.engine.handle_request(request);
                let _ = reply.send(response);
                effects
            }
            Msg::Stop(reference, expected, scope, reply) => {
                self.handle_stop(&reference, &expected, scope, reply)
            }
            Msg::PreviewProjectStop(project, reply) => {
                let _ = reply.send(self.engine.preview_project_stop(&project));
                Vec::new()
            }
            Msg::Extend(reference, reply) => {
                let _ = reply.send(self.engine.extend_instance(&reference));
                Vec::new()
            }
            Msg::Host(scan, hints) => {
                self.engine.observe_manifest_hints(hints);
                self.engine.observe_host(scan)
            }
            Msg::Containers(Ok(containers)) => {
                self.docker_state = DockerState {
                    reachable: true,
                    error: None,
                };
                self.engine.observe_containers(Some(containers))
            }
            Msg::Containers(Err(reason)) => {
                self.docker_state = DockerState {
                    reachable: false,
                    error: Some(reason),
                };
                self.engine.observe_containers(None)
            }
            Msg::Health(results) => {
                for (reference, healthy) in results {
                    self.engine.record_health(&reference, healthy);
                }
                self.engine.rebuild_views();
                Vec::new()
            }
            Msg::Catalog(update) => {
                let CatalogUpdate {
                    catalog,
                    shared,
                    problems,
                } = *update;
                self.catalog_problems = problems;
                self.engine.set_catalog(catalog, shared);
                self.projects_at = 0;
                Vec::new()
            }
            Msg::Terminated(reference, reason, outcome) => {
                self.engine
                    .termination_finished(&reference, reason, &outcome);
                self.finish_stop(&reference, outcome);
                Vec::new()
            }
            Msg::Rules(message) => {
                self.handle_rules(message);
                Vec::new()
            }
            Msg::Preferences(message) => {
                self.handle_preferences(message);
                Vec::new()
            }
            Msg::Ignore(reference, reply) => {
                let _ = reply.send(self.engine.ignore_instance(&reference));
                Vec::new()
            }
            Msg::Log(args, reply) => {
                let _ = reply.send(self.engine.log(&args));
                Vec::new()
            }
            Msg::Describe(reference, reply) => {
                let _ = reply.send(self.describe_adoption(&reference));
                Vec::new()
            }
            Msg::UsesContext(reply) => {
                let _ = reply.send(self.uses_context());
                Vec::new()
            }
            Msg::Adopted(applied) => {
                self.engine
                    .record_adopted(&applied.project, &applied.service);
                self.projects_at = 0;
                Vec::new()
            }
            Msg::HealthTargets(reply) => {
                let _ = reply.send(self.engine.health_targets());
                Vec::new()
            }
        }
    }

    pub(super) fn finish_stop(&mut self, reference: &str, outcome: Terminated) {
        if let Some(reply) = self.pending_stops.remove(reference) {
            let result = match outcome {
                Terminated::Stopped | Terminated::AlreadyGone => Ok(()),
                Terminated::StillAlive => Err(ActionError::StopFailed {
                    failure: StopFailure::StillAlive,
                }),
                Terminated::Refused(failure) | Terminated::Failed(failure) => {
                    Err(ActionError::StopFailed { failure })
                }
            };
            let _ = reply.send(result);
        }
    }

    pub(super) fn handle_stop(
        &mut self,
        reference: &str,
        expected: &StopExpectation,
        scope: StopScope,
        reply: oneshot::Sender<Result<(), ActionError>>,
    ) -> Vec<Effect> {
        if self.pending_stops.contains_key(reference) {
            let _ = reply.send(Err(ActionError::NotRunning {
                reference: reference.to_owned(),
            }));
            return Vec::new();
        }
        let result = match scope {
            StopScope::Instance => self.engine.stop_confirmed(reference, expected),
            StopScope::Project => self.engine.stop_project_confirmed(reference, expected),
        };
        match result {
            Ok(effects) => {
                self.pending_stops.insert(reference.to_owned(), reply);
                effects
            }
            Err(error) => {
                let _ = reply.send(Err(error));
                Vec::new()
            }
        }
    }

    /// Carries out what the engine decided.
    pub(super) fn execute(&self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Notify(notice) => {
                    // Nobody listening is fine.
                    let _ = self.notices.send(notice);
                }
                Effect::Terminate(request) => self.terminate(request),
            }
        }
    }

    pub(super) fn terminate(&self, request: TerminationRequest) {
        let messages = self.messages.clone();
        let source = Arc::clone(&self.source);
        let docker = self.docker.borrow().clone();
        let own_pid = self.own_pid;
        tokio::spawn(async move {
            let TerminationRequest {
                reference,
                target,
                reason,
            } = request;
            let outcome = match target {
                Target::Process(root) => {
                    let uid = rustix::process::getuid().as_raw();
                    let protected = {
                        let source = Arc::clone(&source);
                        tokio::task::spawn_blocking(move || protected_pids(&*source, own_pid))
                            .await
                            .unwrap_or_default()
                    };
                    match terminate(
                        &*source,
                        &OsSignaler,
                        root,
                        &protected,
                        uid,
                        Timings::default(),
                    )
                    .await
                    {
                        Outcome::Stopped => Terminated::Stopped,
                        Outcome::AlreadyGone => Terminated::AlreadyGone,
                        Outcome::StillAlive(_) => Terminated::StillAlive,
                        Outcome::Refused(refusal) => Terminated::Refused(match refusal {
                            Refusal::Protected(pid) => StopFailure::Protected { pid },
                            Refusal::OtherUser(pid) => StopFailure::OtherUser { pid },
                        }),
                        Outcome::Unreadable => {
                            Terminated::Failed(StopFailure::ProcessTableUnreadable)
                        }
                    }
                }
                Target::Container(id) => match docker {
                    Some(docker) => match docker.stop(&id).await {
                        Ok(()) => Terminated::Stopped,
                        Err(error) => Terminated::Failed(StopFailure::Failed {
                            detail: error.to_string(),
                        }),
                    },
                    None => Terminated::Failed(StopFailure::DockerUnavailable),
                },
            };
            let _ = messages
                .send(Msg::Terminated(reference, reason, outcome))
                .await;
        });
    }

    pub(super) fn publish(&mut self) {
        let now = self.engine.now();
        if now.saturating_sub(self.projects_at) >= SUMMARY_EVERY_SECONDS {
            let projects = self.engine.project_summaries();
            if projects != self.projects {
                self.projects = projects;
                self.dirty = true;
            }
            let shared = self.engine.shared_summaries();
            if shared != self.shared {
                self.shared = shared;
                self.dirty = true;
            }
            self.projects_at = now;
        }
        let classifications = self.engine.classifications();
        let unscanned = self.engine.unscanned();
        // What waits for a decision depends on the clock as well as on the views.
        let attention = attention_items(self.engine.views(), now);
        // Failures the engine reports stay visible until the core restarts: a
        // write that failed once is something the person should know about.
        for problem in self.engine.take_problems() {
            if !self.engine_problems.contains(&problem) {
                self.engine_problems.push(problem);
                self.engine_problems.truncate(MAX_ENGINE_PROBLEMS);
            }
        }
        let mut problems = self.catalog_problems.clone();
        problems.extend(self.engine_problems.iter().cloned());
        let revision = self.engine.views_revision();

        let current = self.snapshot.borrow().clone();
        let unchanged = !self.dirty
            && self.published_views == revision
            && current.attention == attention
            && current.docker_reachable == self.docker_state.reachable
            && current.docker_error == self.docker_state.error
            && current.problems == problems
            && current.classifications == classifications
            && current.unscanned == unscanned;
        if unchanged {
            return;
        }
        self.dirty = false;
        self.published_views = revision;
        self.revision = self.revision.wrapping_add(1);
        let _ = self.snapshot.send(Arc::new(Snapshot {
            revision: self.revision,
            instances: self.engine.views().to_vec(),
            docker_reachable: self.docker_state.reachable,
            docker_error: self.docker_state.error.clone(),
            problems,
            projects: self.projects.clone(),
            classifications,
            unscanned,
            shared: self.shared.clone(),
            attention,
            extend_minutes: u32::try_from(
                self.engine
                    .config()
                    .lease
                    .extend_by
                    .as_secs()
                    .checked_div(60)
                    .unwrap_or(0),
            )
            .unwrap_or(u32::MAX),
        }));
    }
}
