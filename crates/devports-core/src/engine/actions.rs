//! What callers can ask of the engine besides `run`: listing, the log, and the
//! actions only a person may take from the application.

use devports_proto::{
    ErrorBody, ErrorCode, LogArgs, LogEntry, LogResult, LsArgs, LsResult, Op, PingResult, Request,
    Response,
};
use serde_json::Value;

use super::leases::target_of;
use super::{Effect, Engine, StopReason, Target, TerminationRequest};
use crate::clock::Clock;
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
