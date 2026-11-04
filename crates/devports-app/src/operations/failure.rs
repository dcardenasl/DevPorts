//! The typed form of an operation failure, for the sheet that started it.
use super::{ActionError, AdoptionError, CommandError, OperationError, SettingsError, UsesError};

/// The typed form of a failure, for the sheet that started the operation.
pub(super) enum TypedFailure {
    Adoption(AdoptionError),
    Uses(UsesError),
    Settings(SettingsError),
    Action(ActionError),
}

/// Safe failure payload retained by the shell, without command arguments. The typed
/// part is boxed so a `Result` carrying it stays small.
pub struct OperationFailure {
    pub(super) message: String,
    pub(super) typed: Option<Box<TypedFailure>>,
}

impl OperationFailure {
    fn typed(message: String, typed: TypedFailure) -> Self {
        Self {
            message,
            typed: Some(Box::new(typed)),
        }
    }
}

impl From<String> for OperationFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            typed: None,
        }
    }
}

impl From<AdoptionError> for OperationFailure {
    fn from(error: AdoptionError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Adoption(error))
    }
}

impl From<SettingsError> for OperationFailure {
    fn from(error: SettingsError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Settings(error))
    }
}

impl From<ActionError> for OperationFailure {
    fn from(error: ActionError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Action(error))
    }
}

/// A request that reached the shell without what its kind needs (the panel checks first).
pub(super) fn missing(what: &str) -> OperationFailure {
    OperationFailure::from(ActionError::Internal {
        detail: format!("the request is missing {what}"),
    })
}

impl From<OperationError> for CommandError {
    fn from(error: OperationError) -> Self {
        match error {
            OperationError::InvalidRequest => Self::OperationInvalid,
            OperationError::IdConflict => Self::OperationIdConflict,
            OperationError::AtCapacity => Self::OperationAtCapacity,
            OperationError::NotFound => Self::OperationNotFound,
            OperationError::NothingToRetry => Self::NothingToRetry,
        }
    }
}

impl From<UsesError> for OperationFailure {
    fn from(error: UsesError) -> Self {
        Self::typed(error.to_string(), TypedFailure::Uses(error))
    }
}
