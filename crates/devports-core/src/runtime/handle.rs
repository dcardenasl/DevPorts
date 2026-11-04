//! The cloneable handle every client (panel commands, IPC, the CLI) uses to talk to the actor.
use super::{
    ActionError, AdoptTarget, Applied, ApplyError, Arc, Description, InstanceView, LogArgs,
    LogError, LogResult, Msg, Notice, PathBuf, PeerInfo, Request, RequestHandler, Response,
    Snapshot, StopExpectation, StopScope, Unsuitability, adoption, apply, broadcast, mpsc, oneshot,
    watch,
};

/// A cloneable handle to the running core.
#[derive(Clone)]
pub struct CoreHandle {
    pub(super) messages: mpsc::Sender<Msg>,
    pub(super) snapshot: watch::Receiver<Arc<Snapshot>>,
    pub(super) notices: broadcast::Sender<Notice>,
    pub(super) shutdown: watch::Sender<bool>,
    pub(super) home: Arc<PathBuf>,
}

impl CoreHandle {
    /// The latest snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Arc<Snapshot> {
        Arc::clone(&self.snapshot.borrow())
    }

    /// Follows the snapshot: resolves each time it changes.
    #[must_use]
    pub fn watch(&self) -> watch::Receiver<Arc<Snapshot>> {
        self.snapshot.clone()
    }

    /// Receives the notices (lease about to end, duplicate detected).
    #[must_use]
    pub fn notices(&self) -> broadcast::Receiver<Notice> {
        self.notices.subscribe()
    }

    /// Answers one socket request.
    pub async fn request(&self, request: Request) -> Response {
        let id = request.id.clone();
        let (reply, answer) = oneshot::channel();
        if self
            .messages
            .send(Msg::Request(request, reply))
            .await
            .is_err()
        {
            return unavailable(id);
        }
        answer.await.unwrap_or_else(|_| unavailable(id))
    }

    /// Stops an instance because the person asked (from the application).
    ///
    /// # Errors
    ///
    /// [`ActionError`] when it does not exist or has nothing to stop.
    pub async fn stop(
        &self,
        reference: &str,
        expected: StopExpectation,
    ) -> Result<(), ActionError> {
        self.stop_with_scope(reference, expected, StopScope::Instance)
            .await
    }

    /// Returns a read-only, exact preview of stoppable project applications.
    ///
    /// # Errors
    /// Returns an action error if the core has shut down.
    pub async fn preview_project_stop(
        &self,
        project: &str,
    ) -> Result<Vec<InstanceView>, ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::PreviewProjectStop(project.to_owned(), reply))
            .await?;
        answer.await.map_err(|_| gone())
    }

    /// Stops only one confirmed project application and waits for termination.
    ///
    /// # Errors
    /// Returns an error on changed identity/membership, forbidden policy or failed termination.
    pub async fn stop_project(
        &self,
        reference: &str,
        expected: StopExpectation,
    ) -> Result<(), ActionError> {
        self.stop_with_scope(reference, expected, StopScope::Project)
            .await
    }

    /// Extends a Temporal because the person asked.
    ///
    /// # Errors
    ///
    /// [`ActionError`] when it does not exist, is not a Temporal or is at the limit.
    pub async fn extend(&self, reference: &str) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Extend(reference.to_owned(), reply))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Keeps an unregistered process or container: it stays and stops asking for
    /// attention.
    ///
    /// # Errors
    ///
    /// [`ActionError`] when it does not exist or cannot be told apart from others.
    pub async fn ignore(&self, reference: &str) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Ignore(reference.to_owned(), reply))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// The activity log.
    ///
    /// # Errors
    ///
    /// [`LogError`] when the store cannot be read or the core is closing: never an
    /// empty list standing in for a failure.
    pub async fn log(&self, args: LogArgs) -> Result<LogResult, LogError> {
        let (reply, answer) = oneshot::channel();
        self.messages
            .send(Msg::Log(args, reply))
            .await
            .map_err(|_| LogError::Closing)?;
        answer.await.map_err(|_| LogError::Closing)?
    }

    /// Writes the manifest that adopts `reference`, on a person's request.
    ///
    /// # Errors
    ///
    /// [`ApplyError`] when it cannot be described there or the file cannot be
    /// written; nothing is changed in that case.
    pub async fn adopt(&self, reference: &str, target: AdoptTarget) -> Result<Applied, ApplyError> {
        let description = self
            .describe_for_adoption(reference)
            .await
            .map_err(|error| {
                ApplyError::Unsuitable(Unsuitability::Other {
                    detail: error.to_string(),
                })
            })?;
        let Description {
            view,
            projects,
            shared,
        } = description;
        let home_for_write = self.home.as_ref().clone();
        let applied = tokio::task::spawn_blocking(move || match target {
            AdoptTarget::Project(name) => {
                let root = projects
                    .iter()
                    .find(|(candidate, _)| *candidate == name)
                    .map(|(_, root)| root.clone())
                    .ok_or(ApplyError::Unsuitable(Unsuitability::ProjectNotInCatalog))?;
                apply::add_service(&root, &view, Some(&shared))
            }
            AdoptTarget::Shared => {
                let path = crate::config::config_dir(&home_for_write).join("shared.yml");
                apply::add_shared(&path, &view)
            }
            AdoptTarget::New(name) => {
                let root = view
                    .cwd
                    .as_deref()
                    .map(PathBuf::from)
                    // The project is the repository, not the folder being served.
                    .map(|cwd| crate::gitinfo::read(&cwd).map_or(cwd, |git| git.worktree))
                    .ok_or(ApplyError::Unsuitable(Unsuitability::NoKnownFolder))?;
                apply::create_project(
                    &root,
                    &home_for_write,
                    name.as_deref(),
                    &view,
                    Some(&shared),
                )
            }
        })
        .await
        .map_err(|error| ApplyError::Io(error.to_string()))??;
        // The new manifest is picked up now instead of at the next pass.
        let home = Arc::clone(&self.home);
        if let Ok(Ok(update)) =
            tokio::task::spawn_blocking(move || adoption::load_catalog(&home)).await
        {
            let _ = self.messages.send(Msg::Catalog(Box::new(update))).await;
        }
        let _ = self.messages.send(Msg::Adopted(applied.clone())).await;
        Ok(applied)
    }

    /// Asks the whole core to stop.
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }

    pub(super) async fn stop_with_scope(
        &self,
        reference: &str,
        expected: StopExpectation,
        scope: StopScope,
    ) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Stop(reference.to_owned(), expected, scope, reply))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    pub(super) async fn send_action(&self, message: Msg) -> Result<(), ActionError> {
        self.messages.send(message).await.map_err(|_| gone())
    }
}

pub(super) fn gone() -> ActionError {
    ActionError::Closing
}

pub(super) fn unavailable(id: String) -> Response {
    Response::err(
        id,
        devports_proto::ErrorBody::new(
            devports_proto::ErrorCode::Unavailable,
            "DevPorts se está cerrando",
        ),
    )
}

pub(super) struct Dispatcher(pub(super) CoreHandle);

impl RequestHandler for Dispatcher {
    async fn handle(&self, request: Request, _peer: PeerInfo) -> Response {
        self.0.request(request).await
    }
}
