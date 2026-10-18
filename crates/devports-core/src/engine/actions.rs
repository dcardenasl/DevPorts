//! What callers can ask of the engine besides `run`: listing, the log, and the
//! actions only a person may take from the application.

use devports_proto::views::{InstanceState, InstanceView};
use devports_proto::{
    ErrorBody, ErrorCode, LogArgs, LogEntry, LogResult, LsArgs, LsResult, Op, PingResult, Request,
    Response,
};
use serde_json::{Value, json};

use super::leases::target_of;
use super::{Effect, Engine, StopReason, Target, TerminationRequest};
use crate::clock::Clock;
use crate::engine::StopExpectation;
use crate::messages;
use crate::ports::PortProbe;
use crate::process::ProcessSource;
use crate::store::{Role, State};

/// Why an action asked for from the application could not be done.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ActionError {
    /// No instance has that reference.
    #[error("no existe la instancia «{0}»")]
    UnknownInstance(String),
    /// There is nothing running to stop.
    #[error("la instancia «{0}» no está en marcha")]
    NotRunning(String),
    /// The action was refused for a stated reason.
    #[error("{0}")]
    Refused(String),
    /// The target changed since the UI showed its stop confirmation.
    #[error("la instancia cambió desde la confirmación; revísala de nuevo")]
    TargetChanged,
}

impl<C: Clock, S: ProcessSource, P: PortProbe> Engine<C, S, P> {
    /// Lists instances (`CLI.md`, section 4): by default those of the project
    /// the caller is in, or of every project when it is in none; `all` adds
    /// processes that belong to no project.
    #[must_use]
    pub fn ls(&self, args: &LsArgs) -> LsResult {
        let project = args.project.clone().or_else(|| {
            args.cwd
                .as_deref()
                .and_then(|cwd| self.catalog.project_containing(std::path::Path::new(cwd)))
                .map(|project| project.name.clone())
        });
        let instances = self
            .views
            .iter()
            .filter(|view| match (&project, args.all) {
                (Some(name), false) => view.project.as_ref() == Some(name),
                (None, false) => view.project.is_some() || view.shared,
                (_, true) => true,
            })
            .cloned()
            .collect();
        LsResult { instances }
    }

    /// The activity log, oldest first.
    #[must_use]
    pub fn log(&self, args: &LogArgs) -> LogResult {
        let now = self.now();
        let since = match args.since_seconds {
            Some(seconds) => now.saturating_sub(i64::try_from(seconds).unwrap_or(i64::MAX)),
            None => start_of_today(now),
        };
        let entries = self
            .store
            .events_since(since, args.project.as_deref())
            .unwrap_or_default()
            .into_iter()
            .map(|event| {
                let payload: Value = serde_json::from_str(&event.payload).unwrap_or(Value::Null);
                LogEntry {
                    at: messages::iso(event.ts),
                    summary: messages::event_summary(
                        &event.kind,
                        event.service.as_deref(),
                        &payload,
                    ),
                    kind: event.kind,
                    project: event.project,
                    service: event.service,
                }
            })
            .collect();
        LogResult { entries }
    }

    /// Stops whatever `reference` is, at a person's request from the application.
    ///
    /// Unlike a token holder, a person may stop a Principal or a process nobody
    /// registered; the application asks for confirmation first when the instance
    /// is a data service or a shared one.
    ///
    /// # Errors
    ///
    /// [`ActionError`] if the instance does not exist or has nothing to stop.
    pub fn stop_instance(&mut self, reference: &str) -> Result<Vec<Effect>, ActionError> {
        let target = self
            .targets
            .get(reference)
            .cloned()
            .ok_or_else(|| ActionError::UnknownInstance(reference.to_owned()))?;
        if let Some(mut record) = self
            .tracked
            .iter()
            .find(|record| record.reference == reference)
            .cloned()
        {
            if matches!(record.state, State::Stopping) {
                return Err(ActionError::NotRunning(reference.to_owned()));
            }
            record.state = State::Stopping;
            self.persist(&record);
            self.update_tracked(&record);
            self.rebuild_views();
        }
        Ok(vec![Effect::Terminate(TerminationRequest {
            reference: reference.to_owned(),
            target,
            reason: StopReason::PersonRequest,
        })])
    }

    /// Stops the exact instance a person confirmed, only while its visible facts
    /// still match the current engine view.
    ///
    /// # Errors
    ///
    /// [`ActionError::TargetChanged`] if the confirmed target is stale, or the
    /// same errors as [`Engine::stop_instance`] when it cannot be stopped.
    pub fn stop_confirmed(
        &mut self,
        reference: &str,
        expected: &StopExpectation,
    ) -> Result<Vec<Effect>, ActionError> {
        let view = self
            .views
            .iter()
            .find(|view| view.instance_ref == reference)
            .ok_or_else(|| ActionError::UnknownInstance(reference.to_owned()))?;
        let target_identity = self.targets.get(reference).map(super::Target::identity);
        if view.state == devports_proto::views::InstanceState::Down
            || !expected.matches(view)
            || expected.target_identity.as_deref() != target_identity.as_deref()
        {
            return Err(ActionError::TargetChanged);
        }
        self.stop_instance(reference)
    }

    /// Captures the concrete application instances eligible for a project stop.
    /// Data, shared resources and instances without a known live target are excluded.
    #[must_use]
    pub fn preview_project_stop(&self, project: &str) -> Vec<InstanceView> {
        self.views
            .iter()
            .filter(|view| project_stop_eligible(view, project))
            .filter(|view| self.targets.contains_key(&view.instance_ref))
            .cloned()
            .collect()
    }

    /// Revalidates one member of a confirmed project stop without expanding it.
    ///
    /// # Errors
    ///
    /// Returns an action error when membership, policy or identity changed.
    pub fn stop_project_confirmed(
        &mut self,
        reference: &str,
        expected: &StopExpectation,
    ) -> Result<Vec<Effect>, ActionError> {
        let project = expected
            .project
            .as_deref()
            .ok_or(ActionError::TargetChanged)?;
        let view = self
            .views
            .iter()
            .find(|view| view.instance_ref == reference)
            .ok_or_else(|| ActionError::UnknownInstance(reference.to_owned()))?;
        if !project_stop_eligible(view, project) {
            return Err(ActionError::TargetChanged);
        }
        self.stop_confirmed(reference, expected)
    }

    /// Keeps an unregistered process or container: it stays, and stops asking for
    /// attention. The choice is remembered across restarts.
    ///
    /// # Errors
    ///
    /// [`ActionError`] if the instance does not exist or cannot be told apart
    /// from others (no folder and no container).
    pub fn ignore_instance(&mut self, reference: &str) -> Result<(), ActionError> {
        let view = self
            .views
            .iter()
            .find(|view| view.instance_ref == reference)
            .cloned()
            .ok_or_else(|| ActionError::UnknownInstance(reference.to_owned()))?;
        let key = crate::ignore::rule_key(&view, &self.docker_context).ok_or_else(|| {
            ActionError::Refused("no se puede recordar un proceso sin carpeta conocida".to_owned())
        })?;
        let now = self.now();
        self.store
            .add_ignored(&key, now)
            .map_err(|error| ActionError::Refused(error.to_string()))?;
        self.ignored.insert(key);
        self.event(
            "kept",
            view.project.as_deref(),
            view.service.as_deref(),
            Some(reference),
            &json!({"port": view.port}),
        );
        self.rebuild_views();
        Ok(())
    }

    /// The rule that ignoring `reference` would save, exactly as it would be stored.
    ///
    /// # Errors
    ///
    /// [`ActionError`] if the instance does not exist or cannot be told apart.
    pub fn ignore_preview(
        &self,
        reference: &str,
    ) -> Result<crate::ignore::IgnoreRule, ActionError> {
        let view = self
            .views
            .iter()
            .find(|view| view.instance_ref == reference)
            .ok_or_else(|| ActionError::UnknownInstance(reference.to_owned()))?;
        let key = crate::ignore::rule_key(view, &self.docker_context).ok_or_else(|| {
            ActionError::Refused("no se puede recordar un proceso sin carpeta conocida".to_owned())
        })?;
        Ok(crate::ignore::describe(&key, 0))
    }

    /// Every saved rule, newest first, including those no live instance matches.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] if the store cannot be read.
    pub fn ignored_rules(&self) -> Result<Vec<crate::ignore::IgnoreRule>, ActionError> {
        let mut rules: Vec<_> = self
            .store
            .ignored_rules()
            .map_err(|error| ActionError::Refused(error.to_string()))?
            .into_iter()
            .map(|(key, at)| crate::ignore::describe(&key, at))
            .collect();
        rules.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(rules)
    }

    /// Forgets one saved rule so what it kept out of the way is observed again.
    /// Restoring a rule that is already gone succeeds without changing anything.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] if the store cannot be read or written.
    pub fn restore_ignored(&mut self, id: &str) -> Result<(), ActionError> {
        let found = self
            .store
            .ignored_rules()
            .map_err(|error| ActionError::Refused(error.to_string()))?
            .into_iter()
            .map(|(key, _)| key)
            .find(|key| crate::ignore::rule_id(key) == id);
        let Some(key) = found else {
            return Ok(());
        };
        self.store
            .remove_ignored(&key)
            .map_err(|error| ActionError::Refused(error.to_string()))?;
        self.ignored.remove(&key);
        self.event("restored", None, None, None, &json!({"rule": id}));
        self.rebuild_views();
        Ok(())
    }

    /// Adds the default extension to a Temporal, at a person's request.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] when the maximum would be passed or it is not a
    /// running Temporal.
    pub fn extend_instance(&mut self, reference: &str) -> Result<(), ActionError> {
        self.extend_reference(reference, None)
            .map(|_| ())
            .map_err(|error| ActionError::Refused(error.message))
    }

    /// Proposes a manifest for `dir` from what runs inside it. Read-only.
    ///
    /// # Errors
    ///
    /// `not_found` if `dir` is not a folder.
    pub fn adopt_propose(
        &self,
        dir: &std::path::Path,
    ) -> Result<devports_proto::AdoptResult, ErrorBody> {
        let dir = std::fs::canonicalize(dir)
            .ok()
            .filter(|dir| dir.is_dir())
            .ok_or_else(|| {
                ErrorBody::new(
                    ErrorCode::NotFound,
                    format!("«{}» no es una carpeta", dir.display()),
                )
            })?;
        let proposal = crate::adopt::propose(&dir, &self.views);
        Ok(devports_proto::AdoptResult {
            proposal: proposal.to_yaml(),
        })
    }

    /// What stopping `reference` would act on, if anything.
    #[must_use]
    pub fn target_of_instance(&self, reference: &str) -> Option<Target> {
        self.targets.get(reference).cloned().or_else(|| {
            self.tracked
                .iter()
                .find(|record| record.reference == reference)
                .and_then(target_of)
        })
    }

    /// Whether `reference` is a registered Temporal.
    #[must_use]
    pub fn is_temporal(&self, reference: &str) -> bool {
        self.tracked
            .iter()
            .any(|record| record.reference == reference && record.role == Role::Temporal)
    }

    /// Answers one request of the socket. The effects are for the caller to carry out.
    pub fn handle_request(&mut self, request: Request) -> (Response, Vec<Effect>) {
        let id = request.id;
        match request.op {
            Op::Ping => (
                respond(
                    &id,
                    &PingResult {
                        version: env!("CARGO_PKG_VERSION").to_owned(),
                        protocol: devports_proto::PROTOCOL_VERSION,
                        pid: self.own_pid,
                    },
                ),
                Vec::new(),
            ),
            Op::RunRegister(register) => {
                let (outcome, effects) = self.run_register(&register);
                (respond(&id, &outcome), effects)
            }
            Op::Ls(args) => (respond(&id, &self.ls(&args)), Vec::new()),
            Op::Log(args) => (respond(&id, &self.log(&args)), Vec::new()),
            Op::Stop(args) => match self.stop_by_token(&args.token) {
                Ok((result, effects)) => (respond(&id, &result), effects),
                Err(error) => (Response::err(id, error), Vec::new()),
            },
            Op::Extend(args) => match self.extend_by_token(&args.token, args.by_seconds) {
                Ok(result) => (respond(&id, &result), Vec::new()),
                Err(error) => (Response::err(id, error), Vec::new()),
            },
            Op::AdoptPropose(args) => match self.adopt_propose(std::path::Path::new(&args.dir)) {
                Ok(result) => (respond(&id, &result), Vec::new()),
                Err(error) => (Response::err(id, error), Vec::new()),
            },
        }
    }
}

fn respond<T: serde::Serialize>(id: &str, result: &T) -> Response {
    Response::ok(id, result).unwrap_or_else(|error| {
        Response::err(id, ErrorBody::new(ErrorCode::Internal, error.to_string()))
    })
}

/// Midnight (local) at or before `now`, in seconds since the epoch.
fn start_of_today(now: i64) -> i64 {
    jiff::Timestamp::from_second(now)
        .ok()
        .map(|moment| moment.to_zoned(jiff::tz::TimeZone::system()))
        .and_then(|zoned| zoned.start_of_day().ok())
        .map_or_else(
            || now.saturating_sub(86_400),
            |start| start.timestamp().as_second(),
        )
}

fn project_stop_eligible(view: &InstanceView, project: &str) -> bool {
    view.project.as_deref() == Some(project)
        && view.kind == "app"
        && !view.shared
        && view.target_identity.is_some()
        && matches!(
            view.state,
            InstanceState::Healthy | InstanceState::Unhealthy | InstanceState::Forgotten
        )
}
