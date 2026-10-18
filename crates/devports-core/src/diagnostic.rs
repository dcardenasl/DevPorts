//! Problems found while reading a configuration file.
//!
//! A problem is a [`DiagnosticKind`] (a code and its parameters) and the line it was found
//! on, not a sentence: the interface writes each one in the language in force (ADR-041).
//! `Display` gives the Spanish sentence that logs, the socket and the CLI keep using.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How serious a problem is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// The file cannot be used.
    Error,
    /// The file is used, but something is probably not what the author meant.
    Warning,
}

/// What is wrong, with the values it is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum DiagnosticKind {
    /// The YAML parser could not read the file, in its own words.
    Syntax {
        /// What it said.
        detail: String,
    },
    /// A key this version does not know.
    UnknownKey {
        /// The key.
        key: String,
    },
    /// `test` is not managed by DevPorts (ADR-010).
    TestIgnored,
    /// `version` is missing.
    VersionMissing,
    /// `version` is not one this build reads.
    UnsupportedVersion {
        /// What the file says, as written.
        version: String,
    },
    /// `version` must be 1.
    VersionMustBeOne,
    /// `project` is missing.
    ProjectMissing,
    /// The project name does not match the allowed shape.
    InvalidProjectName {
        /// The name.
        name: String,
    },
    /// `uses` names a service `shared.yml` does not define.
    UnknownShared {
        /// The name.
        name: String,
    },
    /// `services` is missing.
    ServicesMissing,
    /// A service name does not match the allowed shape.
    InvalidServiceName {
        /// The name.
        name: String,
    },
    /// A service has both `cwd` and `docker`.
    CwdAndDocker {
        /// The service.
        service: String,
    },
    /// A service has neither `cwd` nor `docker`.
    NoCwdNorDocker {
        /// The service.
        service: String,
    },
    /// `docker` is empty.
    EmptyDocker {
        /// The service.
        service: String,
    },
    /// `cwd` is empty.
    EmptyCwd {
        /// The service.
        service: String,
    },
    /// `cwd` of a shared service is not absolute.
    CwdNotAbsolute {
        /// The service.
        service: String,
    },
    /// `cwd` of a project service is absolute.
    CwdAbsolute {
        /// The service.
        service: String,
        /// What the file says.
        cwd: String,
    },
    /// `cwd` climbs out of the project folder.
    CwdEscapes {
        /// The service.
        service: String,
        /// What the file says.
        cwd: String,
    },
    /// `start` spans several lines.
    StartMultiline {
        /// The service.
        service: String,
    },
    /// `port` is outside 1 to 65535.
    PortOutOfRange {
        /// The service.
        service: String,
        /// What the file says, as written.
        port: String,
    },
    /// `port` is below 1024.
    PortPrivileged {
        /// The service.
        service: String,
        /// The port.
        port: u16,
    },
    /// Two services of a project prefer the same port.
    PortTaken {
        /// The port.
        port: u16,
        /// The service that had it first.
        other: String,
    },
    /// `health` is neither `tcp` nor `http:/path`.
    InvalidHealth {
        /// The service.
        service: String,
        /// What the file says.
        health: String,
    },
    /// `kind` is neither `app` nor `data`.
    InvalidKind {
        /// The service.
        service: String,
        /// What the file says.
        kind: String,
    },
    /// `match` is empty.
    EmptyMatch {
        /// The service.
        service: String,
    },
    /// A value of `config.yml` could not be read as a duration.
    InvalidDuration {
        /// The key, dotted.
        key: String,
        /// What the duration reader said.
        detail: String,
    },
    /// `max_depth` is outside 0 to 8.
    MaxDepthRange {
        /// What the file says, as written.
        value: String,
    },
    /// `events_retention_days` is below 1.
    RetentionRange {
        /// What the file says, as written.
        value: String,
    },
    /// `lease.max_per_service` is outside 1 to 16.
    MaxPerServiceRange {
        /// What the file says, as written.
        value: String,
    },
    /// `observation.host_interval` is outside 250ms to 10s.
    HostIntervalRange,
    /// `observation.temporal_port_range` is not `[start, end]` within the allowed range.
    TemporalPortRange,
    /// `lease.default` is longer than `lease.max_total`.
    LeaseDefaultOverMax,
    /// `lease.extend_by` is longer than `lease.max_total`.
    LeaseExtendOverMax,
    /// `lease.warn_before` is not shorter than `lease.default`.
    LeaseWarnOverDefault,
}

impl fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { detail } => f.write_str(detail),
            Self::UnknownKey { key } => write!(f, "clave desconocida «{key}»; se ignora"),
            Self::TestIgnored => f.write_str(
                "«test» ignorada: devports no gestiona configuración de pruebas (ADR-010)",
            ),
            Self::VersionMissing => f.write_str("falta «version»; debe ser 1"),
            Self::UnsupportedVersion { version } => {
                write!(f, "versión {version} no soportada; actualiza devports")
            }
            Self::VersionMustBeOne => f.write_str("«version» debe ser 1"),
            Self::ProjectMissing => f.write_str("falta «project»"),
            Self::InvalidProjectName { name } => write!(
                f,
                "el nombre de proyecto «{name}» no es válido: usa minúsculas, números y guiones, hasta 63 caracteres, sin empezar por guion"
            ),
            Self::UnknownShared { name } => write!(f, "«{name}» no está definido en shared.yml"),
            Self::ServicesMissing => {
                f.write_str("falta «services» (puede estar vacío: `services: {}`)")
            }
            Self::InvalidServiceName { name } => write!(
                f,
                "el nombre de servicio «{name}» no es válido: usa minúsculas, números y guiones"
            ),
            Self::CwdAndDocker { service } => write!(
                f,
                "«{service}» tiene «cwd» y «docker»: debe tener exactamente uno"
            ),
            Self::NoCwdNorDocker { service } => write!(
                f,
                "«{service}» necesita «cwd» (servicio del host) o «docker» (contenedor)"
            ),
            Self::EmptyDocker { service } => write!(f, "«docker» de «{service}» está vacío"),
            Self::EmptyCwd { service } => write!(f, "«cwd» de «{service}» está vacío"),
            Self::CwdNotAbsolute { service } => write!(
                f,
                "«cwd» de «{service}» debe ser una ruta absoluta (o empezar por ~/) en shared.yml"
            ),
            Self::CwdAbsolute { service, cwd } => write!(
                f,
                "«cwd» de «{service}» debe ser relativa al manifiesto, no «{cwd}»"
            ),
            Self::CwdEscapes { service, cwd } => write!(
                f,
                "«cwd» de «{service}» sale de la carpeta del proyecto: «{cwd}»"
            ),
            Self::StartMultiline { service } => write!(
                f,
                "«start» de «{service}» debe ser un solo comando, sin saltos de línea; para varios pasos usa un script"
            ),
            Self::PortOutOfRange { service, port } => {
                write!(f, "«port» de «{service}» debe estar entre 1 y 65535, no {port}")
            }
            Self::PortPrivileged { service, port } => write!(
                f,
                "«port» de «{service}» es {port}, menor que 1024: puede requerir privilegios"
            ),
            Self::PortTaken { port, other } => write!(
                f,
                "el puerto {port} ya lo usa «{other}»: dentro de un proyecto cada puerto preferido es único"
            ),
            Self::InvalidHealth { service, health } => write!(
                f,
                "«health» de «{service}» debe ser `tcp` o `http:/ruta`, no «{health}»"
            ),
            Self::InvalidKind { service, kind } => write!(
                f,
                "«kind» de «{service}» debe ser `app` o `data`, no «{kind}»"
            ),
            Self::EmptyMatch { service } => write!(
                f,
                "«match» de «{service}» está vacío: no reconocerá ningún proceso"
            ),
            Self::InvalidDuration { key, detail } => write!(f, "«{key}»: {detail}"),
            Self::MaxDepthRange { value } => {
                write!(f, "«max_depth» debe estar entre 0 y 8, no {value}")
            }
            Self::RetentionRange { value } => {
                write!(f, "«events_retention_days» debe ser al menos 1, no {value}")
            }
            Self::MaxPerServiceRange { value } => {
                write!(f, "«lease.max_per_service» debe estar entre 1 y 16, no {value}")
            }
            Self::HostIntervalRange => {
                f.write_str("«observation.host_interval» debe estar entre 250ms y 10s")
            }
            Self::TemporalPortRange => f.write_str(
                "«observation.temporal_port_range» debe ser [inicio, fin] con 1024 <= inicio < fin <= 65535",
            ),
            Self::LeaseDefaultOverMax => {
                f.write_str("«lease.default» no puede superar «lease.max_total»")
            }
            Self::LeaseExtendOverMax => {
                f.write_str("«lease.extend_by» no puede superar «lease.max_total»")
            }
            Self::LeaseWarnOverDefault => {
                f.write_str("«lease.warn_before» debe ser menor que «lease.default»")
            }
        }
    }
}

/// One problem, with the line it was found on when that is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
pub struct Diagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// 1-based line in the file.
    pub line: Option<u32>,
    /// What is wrong.
    pub kind: DiagnosticKind,
}

impl Diagnostic {
    /// An error.
    #[must_use]
    pub fn error(line: Option<u64>, kind: DiagnosticKind) -> Self {
        Self {
            severity: Severity::Error,
            line: line.and_then(|line| u32::try_from(line).ok()),
            kind,
        }
    }

    /// A warning.
    #[must_use]
    pub fn warning(line: Option<u64>, kind: DiagnosticKind) -> Self {
        Self {
            severity: Severity::Warning,
            line: line.and_then(|line| u32::try_from(line).ok()),
            kind,
        }
    }
}

impl Diagnostic {
    /// The Spanish sentence, without the line.
    #[must_use]
    pub fn message(&self) -> String {
        self.kind.to_string()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "línea {line}: {}", self.kind),
            None => write!(formatter, "{}", self.kind),
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
