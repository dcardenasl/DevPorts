//! Human edits of the scan folders in `config.yml`, independent of any webview.
use std::sync::Arc;

use super::{CoreHandle, Msg, adoption::load_catalog};
use crate::settings::{self, ScanFolders, ScanPreview, SettingsError, SettingsOutcome};

impl CoreHandle {
    /// The scan folders in force, with what each holds and whether they can be edited.
    ///
    /// # Errors
    /// Returns a typed error when the file cannot be read at all or the worker fails.
    pub async fn scan_folders(&self) -> Result<ScanFolders, SettingsError> {
        let home = Arc::clone(&self.home);
        tokio::task::spawn_blocking(move || settings::read(&home))
            .await
            .map_err(|error| worker_error(&error))?
    }

    /// What saving `roots` would add to or remove from the project list. Writes nothing.
    ///
    /// # Errors
    /// Returns the validation or eligibility problem that would stop a save.
    pub async fn preview_scan_folders(
        &self,
        roots: Vec<String>,
    ) -> Result<ScanPreview, SettingsError> {
        let home = Arc::clone(&self.home);
        tokio::task::spawn_blocking(move || settings::preview(&home, &roots))
            .await
            .map_err(|error| worker_error(&error))?
    }

    /// Saves the scan folders and applies them: discovery is re-run at once rather than
    /// at the next periodic pass. Call from the shell's operation runner so it survives
    /// closing the panel. A failed reload never turns a completed write into an error.
    ///
    /// # Errors
    /// Returns a typed validation, conflict or I/O error; nothing is written in any of them.
    pub async fn set_scan_folders(
        &self,
        roots: Vec<String>,
        base: String,
    ) -> Result<SettingsOutcome, SettingsError> {
        let home = Arc::clone(&self.home);
        let (outcome, update) = tokio::task::spawn_blocking(move || {
            let outcome = settings::apply(&home, &roots, &base)?;
            let update = (outcome == SettingsOutcome::Saved)
                .then(|| load_catalog(&home).ok())
                .flatten();
            Ok::<_, SettingsError>((outcome, update))
        })
        .await
        .map_err(|error| worker_error(&error))??;
        if let Some(update) = update {
            let _ = self.messages.send(Msg::Catalog(Box::new(update))).await;
        }
        Ok(outcome)
    }
}

fn worker_error(error: &tokio::task::JoinError) -> SettingsError {
    SettingsError::Io {
        message: error.to_string(),
    }
}
