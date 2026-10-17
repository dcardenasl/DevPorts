//! Actor-owned review lifecycle: bounded previews, coalesced saves and success receipts.
use std::collections::VecDeque;

use super::{AdoptionError, AdoptionPreview, PreparedAdoption, ServiceEdits};
use crate::adopt::apply::Applied;
use devports_proto::views::InstanceView;
use std::path::PathBuf;

const MAX_REVIEWS: usize = 8;

#[derive(Debug)]
enum State {
    Ready(Box<PreparedAdoption>),
    Running {
        prepared: Box<PreparedAdoption>,
        digest: String,
    },
    Applied {
        result: Applied,
        digest: String,
    },
}

#[derive(Debug)]
struct Entry {
    id: String,
    state: State,
}

/// Review registry owned exclusively by the runtime actor.
#[derive(Default)]
pub(crate) struct AdoptionBook {
    entries: VecDeque<Entry>,
}

/// What accepting an explicit save needs from the runtime.
pub(crate) enum BeginApply {
    /// One worker must commit this exact candidate.
    Work(Box<PreparedAdoption>),
    /// An identical save is running; attach a result waiter, not another worker.
    Running,
    /// The exact request already succeeded, even if its response was lost.
    Applied(Applied),
}

impl AdoptionBook {
    pub(crate) fn remember(
        &mut self,
        mut prepared: PreparedAdoption,
    ) -> Result<AdoptionPreview, AdoptionError> {
        let id = loop {
            let entropy = crate::ids::system_entropy().map_err(|error| AdoptionError::Io {
                message: error.to_string(),
            })?;
            let id = crate::ids::token_text(&entropy);
            if !self.entries.iter().any(|entry| entry.id == id) {
                break id;
            }
        };
        prepared.id.clone_from(&id);
        let view = prepared.validate(&prepared.defaults())?;
        while self.entries.len() >= MAX_REVIEWS {
            let index = self
                .entries
                .iter()
                .position(|entry| !matches!(entry.state, State::Running { .. }))
                .ok_or(AdoptionError::AtCapacity)?;
            self.entries.remove(index);
        }
        self.entries.push_back(Entry {
            id,
            state: State::Ready(Box::new(prepared)),
        });
        Ok(view)
    }

    pub(crate) fn read(&self, id: &str) -> Result<Box<PreparedAdoption>, AdoptionError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(AdoptionError::PreviewExpired)?;
        match &entry.state {
            State::Ready(prepared) => Ok(prepared.clone()),
            State::Running { .. } | State::Applied { .. } => Err(AdoptionError::RequestChanged),
        }
    }

    pub(crate) fn reference(&self, id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| match &entry.state {
                State::Ready(prepared) | State::Running { prepared, .. } => {
                    Some(prepared.reference.as_str())
                }
                State::Applied { .. } => None,
            })
    }

    pub(crate) fn begin(
        &mut self,
        id: &str,
        edits: &ServiceEdits,
        current: Option<&InstanceView>,
        projects: &[(String, PathBuf)],
    ) -> Result<BeginApply, AdoptionError> {
        let digest = request_digest(edits)?;
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or(AdoptionError::PreviewExpired)?;
        match &entry.state {
            State::Applied {
                result,
                digest: prior,
            } if *prior == digest => Ok(BeginApply::Applied(result.clone())),
            State::Running { digest: prior, .. } if *prior == digest => Ok(BeginApply::Running),
            State::Running { .. } | State::Applied { .. } => Err(AdoptionError::RequestChanged),
            State::Ready(prepared) => {
                if !current.is_some_and(|view| prepared.matches(view))
                    || !prepared.destination_exists(projects)
                {
                    return Err(AdoptionError::TargetChanged);
                }
                prepared.validate(edits)?;
                let work = prepared.clone();
                entry.state = State::Running {
                    prepared: prepared.clone(),
                    digest,
                };
                Ok(BeginApply::Work(work))
            }
        }
    }

    pub(crate) fn finish(&mut self, id: &str, result: &Result<Applied, AdoptionError>) {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) else {
            return;
        };
        if let State::Running { prepared, digest } = &entry.state {
            entry.state = match result {
                Ok(applied) => State::Applied {
                    result: applied.clone(),
                    digest: digest.clone(),
                },
                Err(_) => State::Ready(prepared.clone()),
            };
        }
    }
}

fn request_digest(edits: &ServiceEdits) -> Result<String, AdoptionError> {
    // Bound even a conflicting replay before allocating/serializing its content.
    if edits.name.len() > 63 {
        return Err(super::invalid(
            "name",
            "el nombre supera el límite permitido",
        ));
    }
    if edits
        .start
        .as_ref()
        .is_some_and(|start| start.len() > super::MAX_START_BYTES)
    {
        return Err(super::invalid(
            "start",
            "los campos superan el límite permitido",
        ));
    }
    let bytes = serde_json::to_vec(edits).map_err(|error| AdoptionError::Io {
        message: error.to_string(),
    })?;
    Ok(crate::hash::sha256_hex(&bytes))
}
