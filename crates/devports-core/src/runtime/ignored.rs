//! Ignore rules: preview, list and restore, independent of any webview.
use tokio::sync::oneshot;

use super::{ActionError, Actor, CoreHandle, Msg, gone};
use crate::clock::Clock;
use crate::ignore::IgnoreRule;
use crate::ports::PortProbe;
use crate::process::ProcessSource;

pub(super) enum IgnoredMsg {
    /// The Docker context the containers come from.
    DockerContext(String),
    Preview(String, oneshot::Sender<Result<IgnoreRule, ActionError>>),
    List(oneshot::Sender<Result<Vec<IgnoreRule>, ActionError>>),
    Restore(String, oneshot::Sender<Result<(), ActionError>>),
}

impl CoreHandle {
    /// The rule that ignoring `reference` would save, as it will be stored.
    ///
    /// # Errors
    ///
    /// [`ActionError`] when the instance does not exist or cannot be told apart.
    pub async fn ignore_preview(&self, reference: &str) -> Result<IgnoreRule, ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Ignored(IgnoredMsg::Preview(
            reference.to_owned(),
            reply,
        )))
        .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Every saved ignore rule, newest first, with or without a live instance.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] when the store cannot be read.
    pub async fn ignored_rules(&self) -> Result<Vec<IgnoreRule>, ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Ignored(IgnoredMsg::List(reply)))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Removes one saved rule by its id; a rule that is already gone is not an error.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] when the store cannot be written.
    pub async fn restore_ignored(&self, id: &str) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Ignored(IgnoredMsg::Restore(id.to_owned(), reply)))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }
}

impl<C: Clock + 'static, S: ProcessSource + 'static, P: PortProbe + 'static> Actor<C, S, P> {
    pub(super) fn handle_ignored(&mut self, message: IgnoredMsg) {
        match message {
            IgnoredMsg::DockerContext(context) => {
                self.engine.set_docker_context(&context);
                self.dirty = true;
            }
            IgnoredMsg::Preview(reference, reply) => {
                let _ = reply.send(self.engine.ignore_preview(&reference));
            }
            IgnoredMsg::List(reply) => {
                let _ = reply.send(self.engine.ignored_rules());
            }
            IgnoredMsg::Restore(id, reply) => {
                let result = self.engine.restore_ignored(&id);
                self.dirty = true;
                let _ = reply.send(result);
            }
        }
    }
}
