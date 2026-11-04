//! Human-requested operations that outlive the ephemeral panel webview.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use devports_core::adopt::review::{AdoptionError, AdoptionTarget as AdoptTarget, ServiceEdits};
use devports_core::classify::ProcessClass;
use devports_core::engine::{ActionError, StopExpectation};
use devports_core::reasons::CommandError;
use devports_core::runtime::CoreHandle;
use devports_core::settings::SettingsError;
use devports_core::uses::{UseOutcome, UseReport, UseRequest, UsesError};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::panel::PANEL_LABEL;

const MAX_OPERATIONS: usize = 48;
pub(crate) const MAX_TARGETS: usize = 64;

/// Event carrying an operation snapshot to an open panel.
pub const OPERATION_EVENT: &str = "operation";

mod failure;
mod runner;
mod store;
#[cfg(test)]
mod tests;
mod types;
mod validation;

pub use failure::*;
pub use runner::*;
pub use store::*;
pub use types::*;
