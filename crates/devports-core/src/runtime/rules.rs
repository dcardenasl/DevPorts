//! Rules about observed processes (ignore and classification corrections),
//! independent of any webview.
use tokio::sync::oneshot;

use super::{ActionError, Actor, CoreHandle, Msg, gone};
use crate::classify::ProcessClass;
use crate::clock::Clock;
use crate::ignore::IgnoreRule;
use crate::ports::PortProbe;
use crate::process::ProcessSource;

pub(super) enum RulesMsg {
    /// The Docker context the containers come from.
    DockerContext(String),
    Preview(String, oneshot::Sender<Result<IgnoreRule, ActionError>>),
    List(oneshot::Sender<Result<Vec<IgnoreRule>, ActionError>>),
    Restore(String, oneshot::Sender<Result<(), ActionError>>),
    Classify(
        String,
        Option<ProcessClass>,
        oneshot::Sender<Result<(), ActionError>>,
    ),
}

impl CoreHandle {
    /// The exact criterion a rule about `reference` (an ignore or a classification
    /// correction) would be stored under.
    ///
    /// # Errors
    ///
    /// [`ActionError`] when the instance does not exist or cannot be told apart.
    pub async fn rule_preview(&self, reference: &str) -> Result<IgnoreRule, ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Rules(RulesMsg::Preview(reference.to_owned(), reply)))
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
        self.send_action(Msg::Rules(RulesMsg::List(reply))).await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Removes one saved rule by its id; a rule that is already gone is not an error.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] when the store cannot be written.
    pub async fn restore_ignored(&self, id: &str) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Rules(RulesMsg::Restore(id.to_owned(), reply)))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Moves a loose process between the panel's groups, or back to the automatic
    /// guess with `None`. Nothing is ignored, adopted or stopped.
    ///
    /// # Errors
    ///
    /// [`ActionError`] when the instance is gone, cannot be told apart or the store refuses.
    pub async fn set_classification(
        &self,
        reference: &str,
        class: Option<ProcessClass>,
    ) -> Result<(), ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Rules(RulesMsg::Classify(
            reference.to_owned(),
            class,
            reply,
        )))
        .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }
}

impl<C: Clock + 'static, S: ProcessSource + 'static, P: PortProbe + 'static> Actor<C, S, P> {
    pub(super) fn handle_rules(&mut self, message: RulesMsg) {
        match message {
            RulesMsg::DockerContext(context) => {
                self.engine.set_docker_context(&context);
                self.dirty = true;
            }
            RulesMsg::Preview(reference, reply) => {
                let _ = reply.send(self.engine.rule_preview(&reference));
            }
            RulesMsg::List(reply) => {
                let _ = reply.send(self.engine.ignored_rules());
            }
            RulesMsg::Classify(reference, class, reply) => {
                let result = self.engine.set_classification(&reference, class);
                self.dirty = true;
                let _ = reply.send(result);
            }
            RulesMsg::Restore(id, reply) => {
                let result = self.engine.restore_ignored(&id);
                self.dirty = true;
                let _ = reply.send(result);
            }
        }
    }
}
