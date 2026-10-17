//! Problems found while reading a configuration file.

use std::fmt;

/// How serious a problem is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The file cannot be used.
    Error,
    /// The file is used, but something is probably not what the author meant.
    Warning,
}

/// One problem, with the line it was found on when that is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// 1-based line in the file.
    pub line: Option<u64>,
    /// What is wrong, in the user's language.
    pub message: String,
}

impl Diagnostic {
    /// An error.
    #[must_use]
    pub fn error(line: Option<u64>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            line,
            message: message.into(),
        }
    }

    /// A warning.
    #[must_use]
    pub fn warning(line: Option<u64>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "línea {line}: {}", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

/// Whether any of `diagnostics` is an error.
#[must_use]
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
