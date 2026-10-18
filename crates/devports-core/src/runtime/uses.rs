//! Human edits of which projects declare a shared service, independent of any webview.
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::oneshot;

use super::{Actor, CoreHandle, Msg, adoption::load_catalog};
use crate::clock::Clock;
use crate::ports::PortProbe;
use crate::process::ProcessSource;
use crate::uses::{self, ConsumerOptions, UseReport, UseRequest, UsesError};

/// What the actor knows that a consumers edit needs; paths never come from the client.
pub(super) struct UsesContext {
    projects: Vec<(String, PathBuf)>,
    shared: BTreeSet<String>,
}

impl CoreHandle {
    async fn uses_context(&self) -> Result<UsesContext, UsesError> {
        let (reply, answer) = oneshot::channel();
        self.messages
            .send(Msg::UsesContext(reply))
            .await
            .map_err(|_| closing())?;
        answer.await.map_err(|_| closing())
    }

    /// Every project with whether its manifest declares `shared` in `uses`.
    /// Read-only: projects that cannot be edited stay listed with their reason.
    ///
    /// # Errors
    /// Returns a typed error when the runtime is closing or the worker fails.
    pub async fn shared_consumers(&self, shared: &str) -> Result<ConsumerOptions, UsesError> {
        let context = self.uses_context().await?;
        let name = shared.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut consumers: Vec<_> = context
                .projects
                .iter()
                .map(|(project, root)| uses::inspect(project, root, &name, &context.shared))
                .collect();
            consumers.sort_by(|a, b| a.project.cmp(&b.project));
            ConsumerOptions {
                defined: context.shared.contains(&name),
                shared: name,
                consumers,
            }
        })
        .await
        .map_err(|error| worker_error(&error))
    }

    /// Declares or removes `shared` in the confirmed projects' manifests.
    ///
    /// Validates every manifest first and writes nothing if any fails. After
    /// that each file reports alone; completed writes are never reverted. Repeat
    /// calls re-read the files, so a project already in the requested state is
    /// `Unchanged` and nothing is declared twice. Call from the shell's operation
    /// runner so it survives closing the panel.
    ///
    /// # Errors
    /// Returns a typed error when the runtime is closing or the worker fails.
    pub async fn set_shared_consumers(
        &self,
        shared: &str,
        requests: Vec<UseRequest>,
    ) -> Result<Vec<UseReport>, UsesError> {
        let context = self.uses_context().await?;
        let name = shared.to_owned();
        let home = Arc::clone(&self.home);
        let (reports, update) = tokio::task::spawn_blocking(move || {
            let reports = uses::apply(&name, &context.shared, &context.projects, &requests);
            let saved = reports
                .iter()
                .any(|report| report.result == Ok(uses::UseOutcome::Saved));
            // A failed reload never turns a completed write into an error: the
            // periodic catalog refresh recovers it.
            let update = saved.then(|| load_catalog(&home).ok()).flatten();
            (reports, update)
        })
        .await
        .map_err(|error| worker_error(&error))?;
        if let Some(update) = update {
            let _ = self.messages.send(Msg::Catalog(Box::new(update))).await;
        }
        Ok(reports)
    }
}

impl<C: Clock + 'static, S: ProcessSource + 'static, P: PortProbe + 'static> Actor<C, S, P> {
    pub(super) fn uses_context(&self) -> UsesContext {
        UsesContext {
            projects: self
                .engine
                .catalog()
                .projects()
                .iter()
                .map(|project| (project.name.clone(), project.root.clone()))
                .collect(),
            shared: self.engine.shared_names(),
        }
    }
}

fn closing() -> UsesError {
    UsesError::Io {
        message: "DevPorts se está cerrando".to_owned(),
    }
}

fn worker_error(error: &tokio::task::JoinError) -> UsesError {
    UsesError::Io {
        message: error.to_string(),
    }
}
