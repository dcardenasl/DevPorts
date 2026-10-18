//! Why the core refuses or cannot do something to a file, as data (ADR-041).
//!
//! The interface writes these in the language in force from the code and its parameters, so
//! none of them carries a sentence. Each one still has a Spanish `Display` (what logs, the
//! socket and the CLI say), and it is the single place that sentence is written.

use serde::{Deserialize, Serialize};

use crate::config::ConfigError;
use crate::diagnostic::{Diagnostic, Severity};
use crate::manifest::ManifestError;

/// The key of a document a human edit works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FormKey {
    /// `roots` of `config.yml`.
    Roots,
    /// `uses` of a manifest.
    Uses,
    /// `services` of a manifest.
    Services,
    /// `project` of a manifest, where `uses` is inserted after.
    Project,
}

impl FormKey {
    /// The key as it is written in the file.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Roots => "roots",
            Self::Uses => "uses",
            Self::Services => "services",
            Self::Project => "project",
        }
    }
}

/// What is wrong with the way a key is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FormProblem {
    /// The key is not where it was expected.
    NotFound,
    /// It is not a list.
    NotAList,
    /// It is a list, but of more than plain values.
    NotSimpleList,
    /// A flow list that spans several lines.
    MultiLine,
    /// The items of a block list do not share an indentation.
    MixedIndent,
    /// An item is empty.
    EmptyItem,
    /// An item is not a plain name.
    InvalidValue,
    /// It is written in a way DevPorts does not edit.
    NotEditable,
}

/// The kind of file an edit produced, when it turned out not to be valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    /// `config.yml`.
    Config,
    /// A project manifest.
    Manifest,
}

/// Why a file cannot be edited safely by DevPorts: the person has to do it by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum Limitation {
    /// It is a symbolic link or not a regular file.
    NotRegularFile,
    /// It has more than one hard link.
    HardLinked,
    /// It is not UTF-8 text.
    NotUtf8,
    /// It is larger than the editable size.
    TooLarge {
        /// The largest editable size, in bytes.
        max: u32,
    },
    /// Its owner or group cannot be kept when it is replaced.
    OwnerNotKept,
    /// The destination is not an absolute path without `..`.
    TargetNotAbsolute,
    /// The destination names no file.
    NoTargetFile,
    /// The destination has no folder.
    NoTargetFolder,
    /// The folder of the destination is not a regular directory.
    TargetFolderNotDirectory,
    /// No folder of the destination exists.
    TargetFolderNotFound,
    /// The folder of the destination cannot be resolved.
    TargetFolderUnresolvable,
    /// A key is written in a form DevPorts does not edit.
    Form {
        /// The key.
        key: FormKey,
        /// What is wrong with it.
        problem: FormProblem,
    },
    /// The edit did not produce a valid document.
    EditInvalid {
        /// Which kind of file.
        document: DocumentKind,
        /// What the reader found wrong with it.
        problem: FileProblem,
    },
    /// The edit would have changed more than the key it targets.
    EditTouchedMore {
        /// The key the edit targeted.
        key: FormKey,
    },
    /// Anything else the file system or a library said, as it said it.
    Other {
        /// The explanation.
        detail: String,
    },
}

/// The Spanish sentence of each limitation: what logs, the socket and the CLI say.
impl std::fmt::Display for Limitation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRegularFile => {
                formatter.write_str("solo se editan archivos regulares, sin enlaces simbólicos")
            }
            Self::HardLinked => {
                formatter.write_str("no se reemplazan manifiestos con enlaces físicos")
            }
            Self::NotUtf8 => formatter.write_str("el archivo no es texto UTF-8"),
            Self::TooLarge { max } => write!(
                formatter,
                "la edición automática admite como máximo {max} bytes"
            ),
            Self::OwnerNotKept => {
                formatter.write_str("no se puede conservar el propietario o grupo del archivo")
            }
            Self::TargetNotAbsolute => {
                formatter.write_str("el destino debe ser absoluto y no contener «..»")
            }
            Self::NoTargetFile => formatter.write_str("falta el archivo destino"),
            Self::NoTargetFolder => formatter.write_str("falta la carpeta destino"),
            Self::TargetFolderNotDirectory => {
                formatter.write_str("la carpeta destino no es un directorio regular")
            }
            Self::TargetFolderNotFound => formatter.write_str("no se encontró la carpeta destino"),
            Self::TargetFolderUnresolvable => {
                formatter.write_str("no se puede resolver la carpeta destino")
            }
            Self::Form { key, problem } => formatter.write_str(&form_message(*key, *problem)),
            Self::EditInvalid { document, problem } => {
                formatter.write_str(&invalid_edit_message(*document, problem))
            }
            Self::EditTouchedMore { key } => write!(
                formatter,
                "la edición habría cambiado algo más que «{}»; edítalo a mano",
                key.name()
            ),
            Self::Other { detail } => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for Limitation {}

fn form_message(key: FormKey, problem: FormProblem) -> String {
    let name = key.name();
    match problem {
        FormProblem::NotFound => format!("no se encontró «{name}»"),
        FormProblem::NotAList => format!("«{name}» no es una lista; edítalo a mano"),
        FormProblem::NotSimpleList => {
            format!("«{name}» no es una lista de valores simples; edítalo a mano")
        }
        FormProblem::MultiLine => format!("«{name}» abarca varias líneas; edítalo a mano"),
        FormProblem::MixedIndent => format!("«{name}» mezcla sangrías; edítalo a mano"),
        FormProblem::EmptyItem => format!("«{name}» tiene un elemento vacío"),
        FormProblem::InvalidValue => {
            format!("«{name}» contiene valores que no son simples; edítalo a mano")
        }
        FormProblem::NotEditable => {
            format!("«{name}» está escrito en una forma que DevPorts no edita; edítalo a mano")
        }
    }
}

fn invalid_edit_message(document: DocumentKind, problem: &FileProblem) -> String {
    match document {
        DocumentKind::Config => {
            format!("la edición no produjo una configuración válida: {problem}")
        }
        DocumentKind::Manifest => format!("la edición no produjo un manifiesto válido: {problem}"),
    }
}

/// Why a configuration file or a manifest could not be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum FileProblem {
    /// The file could not be read.
    Unreadable {
        /// The file.
        path: String,
        /// What the system said, as it said it.
        detail: String,
    },
    /// The file has errors.
    Invalid {
        /// The file.
        path: String,
        /// Every problem found, errors and warnings.
        diagnostics: Vec<Diagnostic>,
    },
}

impl std::fmt::Display for FileProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { path, detail } => {
                write!(formatter, "{path}: no se pudo leer: {detail}")
            }
            Self::Invalid { path, diagnostics } => {
                let errors: Vec<String> = diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == Severity::Error)
                    .map(ToString::to_string)
                    .collect();
                write!(formatter, "{path}: {}", errors.join("; "))
            }
        }
    }
}

impl From<ConfigError> for FileProblem {
    fn from(error: ConfigError) -> Self {
        match error {
            ConfigError::Read { path, reason } => Self::Unreadable {
                path: path.display().to_string(),
                detail: reason,
            },
            ConfigError::Invalid { path, diagnostics } => Self::Invalid {
                path: path.display().to_string(),
                diagnostics,
            },
        }
    }
}

impl From<ManifestError> for FileProblem {
    fn from(error: ManifestError) -> Self {
        match error {
            ManifestError::Read { path, reason } => Self::Unreadable {
                path: path.display().to_string(),
                detail: reason,
            },
            ManifestError::Invalid { path, diagnostics } => Self::Invalid {
                path: path.display().to_string(),
                diagnostics,
            },
        }
    }
}

/// Why a file cannot be edited at all right now: nothing was changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum Ineligibility {
    /// `config.yml` has errors of its own.
    #[error("config.yml tiene errores; corrígelos a mano antes de editar: {problem}")]
    ConfigInvalid {
        /// What the reader found.
        problem: FileProblem,
    },
    /// The project has no manifest yet.
    #[error("el proyecto aún no tiene .devports.yml; adopta primero uno de sus servicios")]
    NoManifest,
    /// The manifest is not valid.
    #[error("el manifiesto no es válido: {problem}")]
    ManifestInvalid {
        /// What the reader found.
        problem: FileProblem,
    },
    /// The file forbids writing.
    #[error("{path} es de solo lectura")]
    ReadOnly {
        /// The file.
        path: String,
    },
    /// `roots` is written in a form DevPorts does not edit.
    #[error("config.yml escribe «roots» en una forma que DevPorts no edita; edítalo a mano")]
    RootsNotEditable,
}

impl Ineligibility {
    /// The file is read-only.
    #[must_use]
    pub fn read_only(path: &std::path::Path) -> Self {
        Self::ReadOnly {
            path: path.display().to_string(),
        }
    }
}

/// Why stopping an instance did not work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum StopFailure {
    /// The process is DevPorts itself, one of its ancestors or `launchd`.
    #[error("el proceso {pid} está protegido (DevPorts o su entorno)")]
    Protected {
        /// Its process id.
        pid: u32,
    },
    /// The process belongs to another user.
    #[error("el proceso {pid} pertenece a otro usuario")]
    OtherUser {
        /// Its process id.
        pid: u32,
    },
    /// Something survived `SIGKILL`.
    #[error("la instancia sigue activa tras intentar detenerla")]
    StillAlive,
    /// The process table could not be read.
    #[error("no se pudo leer la tabla de procesos")]
    ProcessTableUnreadable,
    /// Docker does not answer.
    #[error("Docker no está disponible")]
    DockerUnavailable,
    /// Docker (or the system) refused, in its own words.
    #[error("{detail}")]
    Failed {
        /// What it said.
        detail: String,
    },
}

/// Why a request from the panel to the shell failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum CommandError {
    /// A single operation cannot name that many instances.
    #[error("hay demasiadas instancias para una sola operación; detén servicios individualmente")]
    TooManyTargets {
        /// The most one operation takes.
        max: u32,
    },
    /// Only local addresses are opened.
    #[error("solo se abren direcciones locales")]
    OnlyLocalUrls,
    /// The panel context was refused.
    #[error("{detail}")]
    InvalidPanelState {
        /// Why, in English: it is for a bug report.
        detail: String,
    },
    /// The shape of an operation request or its identifiers is invalid.
    #[error("la solicitud de operación no es válida")]
    OperationInvalid,
    /// An id was reused for a different request.
    #[error("ese identificador ya pertenece a otra operación")]
    OperationIdConflict,
    /// The bounded history is full of operations still running.
    #[error("hay demasiadas operaciones en curso")]
    OperationAtCapacity,
    /// The operation is not known to this process.
    #[error("la operación no existe")]
    OperationNotFound,
    /// There is nothing failed to retry.
    #[error("la operación no tiene objetivos fallidos que reintentar")]
    NothingToRetry,
    /// The core refused the action.
    #[error("{error}")]
    Action {
        /// What it said.
        error: crate::engine::ActionError,
    },
    /// The system could not do it.
    #[error("{detail}")]
    Io {
        /// What the system said.
        detail: String,
    },
}

impl From<crate::engine::ActionError> for CommandError {
    fn from(error: crate::engine::ActionError) -> Self {
        Self::Action { error }
    }
}

/// Something the core wants the person to know about, kept in the snapshot until it is gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum Problem {
    /// A folder could not be listed during discovery.
    FolderUnreadable {
        /// The folder.
        path: String,
        /// What the system said, as it said it.
        detail: String,
    },
    /// A manifest could not be loaded; its project is not registered.
    ManifestUnusable {
        /// What is wrong with it.
        problem: FileProblem,
    },
    /// A manifest loaded, with things that are probably not as intended.
    ManifestWarnings {
        /// The manifest.
        path: String,
        /// The warnings.
        diagnostics: Vec<Diagnostic>,
    },
    /// Two projects declare the same name, so `--project <name>` is ambiguous.
    DuplicateProjectName {
        /// The repeated name.
        name: String,
        /// The folders of the projects that use it.
        roots: Vec<String>,
    },
    /// The activity log could not be written.
    ActivityLogWrite {
        /// What the store said.
        detail: String,
    },
    /// The activity log could not be pruned.
    ActivityLogPrune {
        /// What the store said.
        detail: String,
    },
    /// An adoption was saved, but the catalog is not up to date yet.
    CatalogNotRefreshed,
    /// An instance could not be saved.
    InstanceNotSaved {
        /// Its reference.
        reference: String,
        /// What the store said.
        detail: String,
    },
}

/// The Spanish sentence of each problem: what logs and the CLI keep saying.
impl std::fmt::Display for Problem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FolderUnreadable { path, detail } => {
                write!(formatter, "{path}: no se pudo leer: {detail}")
            }
            Self::ManifestUnusable { problem } => write!(formatter, "{problem}"),
            Self::ManifestWarnings { path, diagnostics } => {
                let first = diagnostics
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                write!(formatter, "{path}: {first}")
            }
            Self::DuplicateProjectName { name, roots } => write!(
                formatter,
                "el nombre de proyecto «{name}» se repite en {}",
                roots.join(", ")
            ),
            Self::ActivityLogWrite { detail } => write!(
                formatter,
                "no se pudo escribir en el registro de actividad: {detail}"
            ),
            Self::ActivityLogPrune { detail } => write!(
                formatter,
                "no se pudo limpiar el registro de actividad: {detail}"
            ),
            Self::CatalogNotRefreshed => formatter.write_str(
                "la declaración se guardó; el catálogo se actualizará en el próximo sondeo",
            ),
            Self::InstanceNotSaved { reference, detail } => write!(
                formatter,
                "no se pudo guardar la instancia {reference}: {detail}"
            ),
        }
    }
}

/// Why Docker is not answering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum DockerProblem {
    /// The connection that was working was lost.
    ConnectionLost,
    /// Docker could not be reached, in the client's own words.
    Unreachable {
        /// What it said.
        detail: String,
    },
}

impl std::fmt::Display for DockerProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionLost => formatter.write_str("se perdió la conexión con Docker"),
            Self::Unreachable { detail } => formatter.write_str(detail),
        }
    }
}

/// Why a process or container cannot be adopted into a destination right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum Unsuitability {
    /// The instance is gone, already a service or not observed.
    #[error("esa instancia no está disponible para adopción")]
    InstanceUnavailable,
    /// Only containers can be declared as shared.
    #[error("solo los contenedores pueden declararse como compartidos")]
    SharedNeedsContainer,
    /// A container goes into an existing project or into shared services.
    #[error("para ese contenedor elige un proyecto existente o Compartidos")]
    ContainerNeedsProject,
    /// The process folder cannot be resolved.
    #[error("no se puede resolver la carpeta: {detail}")]
    FolderUnresolvable {
        /// What the file system said.
        detail: String,
    },
    /// The destination has no root folder.
    #[error("el destino no tiene raíz")]
    NoRoot,
    /// The process has no known folder.
    #[error("el proceso no tiene carpeta conocida")]
    NoKnownFolder,
    /// The folder is the home or a top-level one.
    #[error(
        "esa carpeta es demasiado amplia para ser un proyecto; adopta el proceso desde la carpeta de su repositorio"
    )]
    FolderTooBroad,
    /// A manifest already exists where a new project would go.
    #[error("ya hay un manifiesto en la raíz; elige ese proyecto y revisa su contenido")]
    ManifestExists,
    /// The process runs outside the project's folder.
    #[error("la carpeta del proceso está fuera de la raíz del proyecto")]
    OutsideProjectRoot,
    /// The root is not inside the scan folders.
    #[error(
        "esta raíz no está en el alcance de exploración; añade su carpeta a config.yml antes de adoptar"
    )]
    OutOfScanScope,
    /// The project left the catalog.
    #[error("ese proyecto ya no está en el catálogo")]
    ProjectNotInCatalog,
    /// The result would be larger than an automatic edit allows.
    #[error("el resultado supera el límite de edición automática")]
    ResultTooLarge,
    /// The container or process cannot be described from where it is.
    #[error("no se pudo describir ese proceso o contenedor desde esa carpeta")]
    NotDescribable,
    /// A file is not valid.
    #[error("{problem}")]
    FileInvalid {
        /// What the reader found.
        problem: FileProblem,
    },
    /// Anything else the file system said.
    #[error("{detail}")]
    Other {
        /// The explanation, as it was said.
        detail: String,
    },
}

/// What is wrong with an editable field of the adoption form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FieldProblem {
    /// Not 1 to 63 lowercase letters, digits or hyphens.
    #[error("usa de 1 a 63 letras minúsculas, números o guiones")]
    InvalidName,
    /// Not one line, or empty, or too long.
    #[error("el comando debe tener una sola línea, sin NUL, entre 1 y 16384 bytes")]
    InvalidStart,
    /// The manifest already has a service with that name.
    #[error("ya existe un servicio con ese nombre")]
    NameTaken,
    /// The value is longer than a review accepts.
    #[error("el valor supera el límite permitido")]
    TooLong,
}

/// Something about a proposal that needs a person's eyes before saving.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ReviewNote {
    /// The observed program usually launches the real server as a child.
    #[error("«{program}» suele lanzar el servidor real como proceso hijo: revisa el comando")]
    LauncherProgram {
        /// The program's name.
        program: String,
    },
    /// The observed command does not fit on one line.
    #[error(
        "el comando observado no se representa como una línea; indica un comando o script estable"
    )]
    NotOneLine,
    /// No start command could be proposed.
    #[error("no se pudo proponer un comando de inicio; puedes declararlo sin inicio automático")]
    NoStartCommand,
    /// What the manifest reader warned about.
    #[error("{diagnostic}")]
    ManifestWarning {
        /// The warning.
        diagnostic: Diagnostic,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: UXR-9.7
    #[test]
    fn a_limitation_says_in_spanish_what_it_always_said() {
        assert_eq!(
            Limitation::Form {
                key: FormKey::Roots,
                problem: FormProblem::MixedIndent
            }
            .to_string(),
            "«roots» mezcla sangrías; edítalo a mano"
        );
        assert_eq!(
            Limitation::EditTouchedMore { key: FormKey::Uses }.to_string(),
            "la edición habría cambiado algo más que «uses»; edítalo a mano"
        );
        assert_eq!(
            Limitation::EditInvalid {
                document: DocumentKind::Config,
                problem: FileProblem::Unreadable {
                    path: "c".to_owned(),
                    detail: "x".to_owned()
                }
            }
            .to_string(),
            "la edición no produjo una configuración válida: c: no se pudo leer: x"
        );
    }

    // covers: UXR-9.7
    #[test]
    fn reasons_travel_as_a_code_and_their_parameters() {
        let json = serde_json::to_string(&Limitation::TooLarge { max: 262_144 }).unwrap();
        assert_eq!(json, r#"{"code":"tooLarge","max":262144}"#);
        let json = serde_json::to_string(&Limitation::Form {
            key: FormKey::Uses,
            problem: FormProblem::NotAList,
        })
        .unwrap();
        assert_eq!(json, r#"{"code":"form","key":"uses","problem":"notAList"}"#);
        let json = serde_json::to_string(&Ineligibility::NoManifest).unwrap();
        assert_eq!(json, r#"{"code":"noManifest"}"#);
    }
}
