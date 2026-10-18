//! Human adoption reviews, independent of the lifetime of any webview response.
use super::{
    Actor, AdoptTarget, CatalogUpdate, CoreHandle, Description, MAX_ENGINE_PROBLEMS, Msg,
    RuntimeError, discover_all, discovery_problems, load_settings,
};
use crate::adopt::review::{
    self, AdoptionContext, AdoptionError, AdoptionOptions, AdoptionPreview, BeginApply,
    PreparedAdoption, ServiceEdits,
};
use crate::{adopt::apply::Applied, clock::Clock, ports::PortProbe, process::ProcessSource};
use std::sync::Arc;
use tokio::sync::oneshot;

pub(super) enum AdoptionMsg {
    Remember(
        Box<PreparedAdoption>,
        oneshot::Sender<Result<AdoptionPreview, AdoptionError>>,
    ),
    Read(
        String,
        oneshot::Sender<Result<Box<PreparedAdoption>, AdoptionError>>,
    ),
    Apply(
        String,
        ServiceEdits,
        oneshot::Sender<Result<Applied, AdoptionError>>,
    ),
    Finished(
        String,
        Result<Applied, AdoptionError>,
        Option<Box<CatalogUpdate>>,
    ),
}

impl CoreHandle {
    pub(super) async fn describe_for_adoption(
        &self,
        reference: &str,
    ) -> Result<Description, AdoptionError> {
        let (reply, answer) = oneshot::channel();
        self.messages
            .send(Msg::Describe(reference.to_owned(), reply))
            .await
            .map_err(|_| closing())?;
        answer
            .await
            .map_err(|_| closing())?
            .ok_or(AdoptionError::TargetChanged)
    }

    async fn adoption_context(&self, reference: &str) -> Result<AdoptionContext, AdoptionError> {
        let Description {
            view,
            projects,
            shared,
        } = self.describe_for_adoption(reference).await?;
        let home = Arc::clone(&self.home);
        let config =
            tokio::task::spawn_blocking(move || load_settings(&home).map(|(config, _)| config))
                .await
                .map_err(|error| worker_error(&error))?
                .map_err(|error| AdoptionError::Io {
                    message: error.to_string(),
                })?;
        Ok(AdoptionContext {
            view,
            projects,
            shared,
            home: self.home.as_ref().clone(),
            roots: config.roots,
            max_depth: config.max_depth,
        })
    }

    /// Read-only eligible destinations and rejection reasons for an observed instance.
    ///
    /// # Errors
    /// Returns a typed error for a missing target or unavailable runtime.
    pub async fn adoption_options(
        &self,
        reference: &str,
    ) -> Result<AdoptionOptions, AdoptionError> {
        let context = self.adoption_context(reference).await?;
        tokio::task::spawn_blocking(move || review::options(&context))
            .await
            .map_err(|error| worker_error(&error))
    }

    /// Freezes an exact observation, destination and file version for explicit review.
    ///
    /// # Errors
    /// Returns eligibility, file validation or registry-capacity errors without writing.
    pub async fn preview_adoption(
        &self,
        reference: &str,
        target: AdoptTarget,
    ) -> Result<AdoptionPreview, AdoptionError> {
        let context = self.adoption_context(reference).await?;
        let prepared = tokio::task::spawn_blocking(move || review::prepare(&context, target))
            .await
            .map_err(|error| worker_error(&error))??;
        let (reply, answer) = oneshot::channel();
        self.messages
            .send(Msg::Adoption(AdoptionMsg::Remember(
                Box::new(prepared),
                reply,
            )))
            .await
            .map_err(|_| closing())?;
        answer.await.map_err(|_| closing())?
    }

    /// Validates editable fields against the frozen base and returns the exact candidate.
    ///
    /// # Errors
    /// Returns typed field errors, expired review or external-file conflicts; never writes.
    pub async fn validate_adoption(
        &self,
        id: &str,
        edits: ServiceEdits,
    ) -> Result<AdoptionPreview, AdoptionError> {
        let (reply, answer) = oneshot::channel();
        self.messages
            .send(Msg::Adoption(AdoptionMsg::Read(id.to_owned(), reply)))
            .await
            .map_err(|_| closing())?;
        let prepared = answer.await.map_err(|_| closing())??;
        tokio::task::spawn_blocking(move || {
            prepared.file.check().map_err(review::document_error)?;
            prepared.validate(&edits)
        })
        .await
        .map_err(|error| worker_error(&error))?
    }

    /// Applies reviewed fields once. Identical in-flight saves coalesce and successful
    /// replays return their receipt, without selecting another service or adding a suffix.
    /// Call from the shell's operation runner so it survives closing the panel.
    ///
    /// # Errors
    /// Returns typed validation, stale-target, external-conflict or I/O errors.
    pub async fn apply_adoption(
        &self,
        id: &str,
        edits: ServiceEdits,
    ) -> Result<Applied, AdoptionError> {
        let (reply, answer) = oneshot::channel();
        self.messages
            .send(Msg::Adoption(AdoptionMsg::Apply(
                id.to_owned(),
                edits,
                reply,
            )))
            .await
            .map_err(|_| closing())?;
        answer.await.map_err(|_| closing())?
    }
}

impl<C: Clock + 'static, S: ProcessSource + 'static, P: PortProbe + 'static> Actor<C, S, P> {
    pub(super) fn describe_adoption(&self, reference: &str) -> Option<Description> {
        self.engine
            .views()
            .iter()
            .find(|view| view.instance_ref == reference)
            .cloned()
            .map(|view| Description {
                view,
                projects: self
                    .engine
                    .catalog()
                    .projects()
                    .iter()
                    .map(|project| (project.name.clone(), project.root.clone()))
                    .collect(),
                shared: self.engine.shared_names(),
            })
    }

    pub(super) fn handle_adoption(&mut self, message: AdoptionMsg) {
        match message {
            AdoptionMsg::Remember(prepared, reply) => {
                let _ = reply.send(self.adoptions.remember(*prepared));
            }
            AdoptionMsg::Read(id, reply) => {
                let _ = reply.send(self.adoptions.read(&id));
            }
            AdoptionMsg::Apply(id, edits, reply) => self.begin_adoption(id, edits, reply),
            AdoptionMsg::Finished(id, result, catalog) => {
                self.adoptions.finish(&id, &result);
                if let Ok(applied) = &result {
                    if let Some(update) = catalog {
                        let CatalogUpdate {
                            catalog,
                            shared,
                            problems,
                        } = *update;
                        self.engine.set_catalog(catalog, shared);
                        self.catalog_problems = problems;
                    } else {
                        let warning = "la declaración se guardó; el catálogo se actualizará en el próximo sondeo".to_owned();
                        if !self.engine_problems.contains(&warning) {
                            self.engine_problems.push(warning);
                            self.engine_problems.truncate(MAX_ENGINE_PROBLEMS);
                        }
                    }
                    self.engine
                        .record_adopted(&applied.project, &applied.service);
                    self.projects_at = 0;
                    self.dirty = true;
                }
                for waiter in self.adoption_waiters.remove(&id).unwrap_or_default() {
                    let _ = waiter.send(result.clone());
                }
            }
        }
    }

    fn begin_adoption(
        &mut self,
        id: String,
        edits: ServiceEdits,
        reply: oneshot::Sender<Result<Applied, AdoptionError>>,
    ) {
        if let Some(waiters) = self.adoption_waiters.get_mut(&id) {
            waiters.retain(|waiter| !waiter.is_closed());
            if waiters.len() >= 64 {
                let _ = reply.send(Err(AdoptionError::AtCapacity));
                return;
            }
        }
        let current = self
            .adoptions
            .reference(&id)
            .and_then(|reference| {
                self.engine
                    .views()
                    .iter()
                    .find(|view| view.instance_ref == reference)
            })
            .cloned();
        let projects: Vec<_> = self
            .engine
            .catalog()
            .projects()
            .iter()
            .map(|project| (project.name.clone(), project.root.clone()))
            .collect();
        match self
            .adoptions
            .begin(&id, &edits, current.as_ref(), &projects)
        {
            Ok(BeginApply::Applied(result)) => {
                let _ = reply.send(Ok(result));
            }
            Ok(BeginApply::Running) => {
                self.adoption_waiters.entry(id).or_default().push(reply);
            }
            Ok(BeginApply::Work(prepared)) => {
                self.adoption_waiters
                    .entry(id.clone())
                    .or_default()
                    .push(reply);
                let messages = self.messages.clone();
                let home = Arc::clone(&self.home);
                tokio::spawn(async move {
                    let (result, update) = tokio::task::spawn_blocking(move || {
                        let result = load_settings(&home)
                            .map_err(|error| AdoptionError::Io {
                                message: error.to_string(),
                            })
                            .and_then(|(config, _)| {
                                prepared.check_discovery(&config.roots, config.max_depth)
                            })
                            .and_then(|()| prepared.commit(&edits));
                        let update = result
                            .as_ref()
                            .ok()
                            .and_then(|_| load_catalog(&home).ok())
                            .map(Box::new);
                        (result, update)
                    })
                    .await
                    .unwrap_or_else(|error| (Err(worker_error(&error)), None));
                    let _ = messages
                        .send(Msg::Adoption(AdoptionMsg::Finished(id, result, update)))
                        .await;
                });
            }
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

pub(super) fn load_catalog(home: &std::path::Path) -> Result<CatalogUpdate, RuntimeError> {
    let (config, shared) = load_settings(home)?;
    let discovery = discover_all(&config, &shared);
    let problems = discovery_problems(&discovery.issues);
    Ok(CatalogUpdate {
        catalog: discovery.catalog,
        shared,
        problems,
    })
}
fn closing() -> AdoptionError {
    AdoptionError::Io {
        message: "DevPorts se está cerrando".to_owned(),
    }
}
fn worker_error(error: &tokio::task::JoinError) -> AdoptionError {
    AdoptionError::Io {
        message: error.to_string(),
    }
}
