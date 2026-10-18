//! The person's appearance and language preferences, independent of any webview.
use devports_proto::Preferences;
use tokio::sync::oneshot;

use super::{ActionError, Actor, CoreHandle, Msg, gone};
use crate::clock::Clock;
use crate::ports::PortProbe;
use crate::process::ProcessSource;

pub(super) enum PreferencesMsg {
    Get(oneshot::Sender<Result<Preferences, ActionError>>),
    Set(
        Preferences,
        oneshot::Sender<Result<Preferences, ActionError>>,
    ),
}

impl CoreHandle {
    /// The saved preferences; `system` for anything never chosen.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] when the store cannot be read.
    pub async fn preferences(&self) -> Result<Preferences, ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Preferences(PreferencesMsg::Get(reply)))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Saves the preferences and returns what is now stored.
    ///
    /// # Errors
    ///
    /// [`ActionError::Refused`] when the store cannot be written.
    pub async fn set_preferences(
        &self,
        preferences: Preferences,
    ) -> Result<Preferences, ActionError> {
        let (reply, answer) = oneshot::channel();
        self.send_action(Msg::Preferences(PreferencesMsg::Set(preferences, reply)))
            .await?;
        answer.await.unwrap_or_else(|_| Err(gone()))
    }
}

impl<C: Clock + 'static, S: ProcessSource + 'static, P: PortProbe + 'static> Actor<C, S, P> {
    pub(super) fn handle_preferences(&mut self, message: PreferencesMsg) {
        match message {
            PreferencesMsg::Get(reply) => {
                let _ = reply.send(self.engine.preferences());
            }
            PreferencesMsg::Set(preferences, reply) => {
                let _ = reply.send(self.engine.set_preferences(preferences));
            }
        }
    }
}
