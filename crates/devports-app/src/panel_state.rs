//! Volatile panel context that survives destruction of the webview.

use std::sync::{Mutex, PoisonError};

use devports_core::reasons::ReviewNote;
use serde::{Deserialize, Serialize};

/// Contract version of the serialized panel context.
pub const PANEL_STATE_VERSION: u32 = 3;
/// Most projects either manual list may hold.
const MAX_FOLDED_PROJECTS: usize = 256;

/// The panel context kept by the application shell, never written to disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelState {
    /// Contract version, incremented when the serialized shape changes.
    pub version: u32,
    /// Last selected panel tab.
    pub tab: PanelTab,
    /// Projects the person opened by hand; they stay open whatever their activity.
    pub open_projects: Vec<String>,
    /// Projects the person folded by hand; they stay folded whatever their activity.
    pub closed_projects: Vec<String>,
    /// Activity filter selection.
    pub activity_filter: ActivityFilter,
    /// A non-destructive, resumable form draft.
    pub draft: Option<PanelDraft>,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            version: PANEL_STATE_VERSION,
            tab: PanelTab::Attention,
            open_projects: Vec::new(),
            closed_projects: Vec::new(),
            activity_filter: ActivityFilter::All,
            draft: None,
        }
    }
}

/// Available panel sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelTab {
    /// Attention and active alerts.
    Attention,
    /// Projects and their services.
    Projects,
    /// Shared resources.
    Shared,
    /// Services without a project.
    Orphans,
    /// Recent activity.
    Activity,
}

/// Available activity filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityFilter {
    /// All activity.
    All,
    /// Rejected commands.
    Rejections,
    /// Temporary services.
    Temporals,
    /// Alerts.
    Alerts,
}

/// Draft data that can safely resume after recreating the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PanelDraft {
    /// Adoption destination form.
    Adopt {
        /// Stable instance reference; checked against the live snapshot on restore.
        reference: String,
        /// Active adoption step.
        mode: AdoptMode,
        /// Uncommitted new project name.
        name: String,
        /// Last substep visited, so focus can return to the same option.
        last_step: Option<AdoptMode>,
        /// Review handle and editable fields, without copying observed argv or YAML.
        #[serde(default)]
        review: Option<AdoptionDraftReview>,
        /// Accepted/requested operation key, used only to query its result on restore.
        #[serde(default)]
        operation_id: Option<String>,
    },
    /// Projects selected to consume one shared service. Holds names only: the
    /// manifests, their versions and the delta are read again on restore.
    Consumers {
        /// The shared service being managed.
        shared: String,
        /// Projects the person ticked.
        selected: Vec<String>,
        /// Whether the person is looking at the file-by-file review.
        reviewing: bool,
        /// Project whose row should receive focus, when opened from its card.
        focus: Option<String>,
        /// Operation key sent for this selection, used only to query its result on restore.
        #[serde(default)]
        operation_id: Option<String>,
    },
    /// The scan-folder list being edited. Holds only the text typed: the files, their
    /// version and the effect on discovery are read again on restore.
    Settings {
        /// The folders as typed, in order.
        roots: Vec<String>,
        /// Whether the person is looking at the effect review.
        reviewing: bool,
        /// Operation key sent for this list, used only to query its result on restore.
        #[serde(default)]
        operation_id: Option<String>,
    },
}

/// Most projects a consumers draft may name, matching one operation's capacity.
const MAX_DRAFT_PROJECTS: usize = 64;
/// Longest project or shared-service name (`^[a-z0-9][a-z0-9-]{0,62}$`).
const MAX_NAME_BYTES: usize = 63;
/// Longest folder text a settings draft may hold.
const MAX_ROOT_BYTES: usize = 1024;

impl PanelDraft {
    fn is_bounded(&self) -> bool {
        match self {
            Self::Adopt { .. } => true,
            Self::Settings {
                roots,
                operation_id,
                ..
            } => {
                roots.len() <= devports_core::settings::MAX_ROOTS
                    && roots.iter().all(|root| root.len() <= MAX_ROOT_BYTES)
                    && operation_id.as_ref().is_none_or(|id| id.len() <= 64)
            }
            Self::Consumers {
                shared,
                selected,
                focus,
                operation_id,
                ..
            } => {
                shared.len() <= MAX_NAME_BYTES
                    && selected.len() <= MAX_DRAFT_PROJECTS
                    && selected.iter().all(|name| name.len() <= MAX_NAME_BYTES)
                    && focus
                        .as_ref()
                        .is_none_or(|name| name.len() <= MAX_NAME_BYTES)
                    && operation_id.as_ref().is_none_or(|id| id.len() <= 64)
            }
        }
    }
}

/// Minimal editable review context retained only while the application runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptionDraftReview {
    /// Core-issued review handle.
    pub id: String,
    /// Selected destination, revalidated explicitly before a save.
    pub target: devports_core::adopt::review::AdoptionTarget,
    /// User-editable fields required to recover the form.
    pub edits: devports_core::adopt::review::ServiceEdits,
    /// Display-only facts; saving always uses the core's immutable review.
    pub context: AdoptionDraftContext,
}

/// Display-only facts needed to recover a review whose fields may be invalid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptionDraftContext {
    /// Canonical project directory shown in the review.
    pub root: String,
    /// Canonical manifest path shown in the review.
    pub manifest: String,
    /// Actual project label.
    pub project: String,
    /// Whether the proposed document is new.
    pub creates_manifest: bool,
    /// Port observed when the review was prepared.
    pub observed_port: Option<u16>,
    /// Proposal warnings shown before confirmation, as the core described them.
    pub warnings: Vec<ReviewNote>,
}

/// Adoption dialog step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdoptMode {
    /// Destination choices.
    Options,
    /// Existing project list.
    Others,
    /// New project name form.
    New,
    /// Review editable service fields before explicitly saving.
    Review,
}

fn within_limits(names: &[String]) -> bool {
    names.len() <= MAX_FOLDED_PROJECTS && names.iter().all(|name| name.len() <= MAX_NAME_BYTES)
}

/// Thread-safe owner for the volatile panel context.
#[derive(Debug, Default)]
pub struct PanelStateStore(Mutex<PanelState>);

impl PanelStateStore {
    /// Returns the current context.
    pub fn get(&self) -> PanelState {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Replaces the current context, accepting only the current contract version.
    pub fn set(&self, state: PanelState) -> Result<(), &'static str> {
        if state.version != PANEL_STATE_VERSION {
            return Err("unsupported panel state version");
        }
        if !within_limits(&state.open_projects) || !within_limits(&state.closed_projects) {
            return Err("project list exceeds its limits");
        }
        if state
            .draft
            .as_ref()
            .is_some_and(|draft| !draft.is_bounded())
        {
            return Err("panel draft exceeds its limits");
        }
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_context_round_trips_through_json() {
        let state = PanelState {
            tab: PanelTab::Projects,
            open_projects: vec!["shop".to_owned()],
            closed_projects: vec!["blog".to_owned()],
            activity_filter: ActivityFilter::Alerts,
            draft: Some(PanelDraft::Adopt {
                reference: "instance-1".to_owned(),
                mode: AdoptMode::New,
                name: "new-project".to_owned(),
                last_step: Some(AdoptMode::Others),
                review: None,
                operation_id: None,
            }),
            ..PanelState::default()
        };
        let encoded = serde_json::to_string(&state).expect("serialize panel state");
        let decoded: PanelState = serde_json::from_str(&encoded).expect("deserialize panel state");
        assert_eq!(decoded, state);
    }

    #[test]
    fn consumers_draft_round_trips_and_is_bounded() {
        let draft = PanelDraft::Consumers {
            shared: "mysql".to_owned(),
            selected: vec!["shop".to_owned(), "blog".to_owned()],
            reviewing: true,
            focus: Some("shop".to_owned()),
            operation_id: None,
        };
        let state = PanelState {
            draft: Some(draft.clone()),
            ..PanelState::default()
        };
        let encoded = serde_json::to_string(&state).expect("serialize");
        assert!(encoded.contains("\"kind\":\"consumers\""));
        assert_eq!(
            serde_json::from_str::<PanelState>(&encoded).expect("deserialize"),
            state
        );
        let store = PanelStateStore::default();
        assert_eq!(store.set(state.clone()), Ok(()));
        let PanelDraft::Consumers { shared, .. } = draft else {
            return;
        };
        let oversized = PanelState {
            draft: Some(PanelDraft::Consumers {
                shared,
                selected: (0..65).map(|index| format!("p{index}")).collect(),
                reviewing: false,
                focus: None,
                operation_id: None,
            }),
            ..PanelState::default()
        };
        assert_eq!(store.set(oversized), Err("panel draft exceeds its limits"));
        assert_eq!(
            store.get(),
            state,
            "a rejected draft keeps the previous state"
        );
    }

    #[test]
    fn settings_draft_round_trips_and_is_bounded() {
        let state = PanelState {
            draft: Some(PanelDraft::Settings {
                roots: vec!["~/developer".to_owned(), "/Volumes/work".to_owned()],
                reviewing: true,
                operation_id: None,
            }),
            ..PanelState::default()
        };
        let encoded = serde_json::to_string(&state).expect("serialize");
        assert!(encoded.contains("\"kind\":\"settings\""));
        assert_eq!(
            serde_json::from_str::<PanelState>(&encoded).expect("deserialize"),
            state
        );
        let store = PanelStateStore::default();
        assert_eq!(store.set(state.clone()), Ok(()));
        let oversized = PanelState {
            draft: Some(PanelDraft::Settings {
                roots: vec!["x".repeat(2000)],
                reviewing: false,
                operation_id: None,
            }),
            ..PanelState::default()
        };
        assert_eq!(store.set(oversized), Err("panel draft exceeds its limits"));
        assert_eq!(store.get(), state);
    }

    #[test]
    fn project_lists_are_bounded() {
        let store = PanelStateStore::default();
        let too_many = PanelState {
            open_projects: (0..257).map(|index| format!("p{index}")).collect(),
            ..PanelState::default()
        };
        assert_eq!(store.set(too_many), Err("project list exceeds its limits"));
        let too_long = PanelState {
            closed_projects: vec!["x".repeat(64)],
            ..PanelState::default()
        };
        assert_eq!(store.set(too_long), Err("project list exceeds its limits"));
        assert_eq!(store.get(), PanelState::default());
    }

    #[test]
    fn store_rejects_unknown_versions_without_replacing_state() {
        let store = PanelStateStore::default();
        let unknown = PanelState {
            version: PANEL_STATE_VERSION + 1,
            ..PanelState::default()
        };
        assert_eq!(store.set(unknown), Err("unsupported panel state version"));
        assert_eq!(store.get(), PanelState::default());
    }

    #[test]
    fn state_has_no_place_for_a_stop_confirmation() {
        let encoded = serde_json::to_string(&PanelState::default()).expect("serialize state");
        assert!(!encoded.contains("stop"));
        assert!(!encoded.contains("invoker"));
    }
}
