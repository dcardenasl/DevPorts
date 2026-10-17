//! Frozen, validated adoption proposals. Only explicit application operations apply them.

mod book;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use devports_proto::views::InstanceView;
use serde::{Deserialize, Serialize};

use super::apply::{Applied, insert_service, unused_name};
use super::{Proposal, ProposedService, service_for, slug};
use crate::config::SharedServices;
use crate::document::{EditError, FileSnapshot};
use crate::manifest::{self, MANIFEST_FILE};

pub(crate) use book::{AdoptionBook, BeginApply};

const MAX_START_BYTES: usize = 16_384;

/// The destination selected by a person, resolved again by the core at preview time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AdoptionTarget {
    /// A known project, including one whose manifest has not been created yet.
    Project(String),
    /// A project at the process's actual repository root (or its folder without Git).
    New(Option<String>),
    /// The shared services document; only containers qualify.
    Shared,
}

/// Editable fields; every other service fact stays bound to the frozen observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ServiceEdits {
    /// Exact service name; validation never silently adds a suffix on apply.
    pub name: String,
    /// Exact shell command to store, not execute; absent when no command is known.
    pub start: Option<String>,
}

/// Read-only eligibility for one destination.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AdoptionDestination {
    /// Selection to send to preview.
    pub target: AdoptionTarget,
    /// Whether this destination can currently be edited.
    pub eligible: bool,
    /// Why it cannot be used, without hiding it as an unexplained missing option.
    pub reason: Option<String>,
    /// Actual project directory, when it can be resolved.
    pub root: Option<String>,
    /// Actual manifest destination, when it can be resolved.
    pub manifest: Option<String>,
    /// Whether saving explicitly creates a manifest for a provisional project.
    pub creates_manifest: bool,
}

/// Destinations for one observed process/container. Querying never writes files.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AdoptionOptions {
    /// Exact short reference being adopted.
    pub reference: String,
    /// Destinations with reasons for rejected choices.
    pub destinations: Vec<AdoptionDestination>,
}

/// A reviewable proposal bound to a file version and one target observation.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AdoptionPreview {
    /// Opaque in-memory review handle; it expires rather than being recalculated silently.
    pub id: String,
    /// Instance reference.
    pub reference: String,
    /// Selected destination.
    pub target: AdoptionTarget,
    /// Final project name or shared destination.
    pub project: String,
    /// Canonical project directory.
    pub root: String,
    /// Canonical document destination.
    pub manifest: String,
    /// This save would create the file, explicitly including provisional projects.
    pub creates_manifest: bool,
    /// Port observed in the frozen proposal.
    pub observed_port: Option<u16>,
    /// The exact proposed service, including editable defaults.
    pub service: ProposedService,
    /// Warnings requiring review; not authorization to skip the review when empty.
    pub warnings: Vec<String>,
    /// Fully validated candidate document, for review only.
    pub yaml: String,
}

/// Typed failures for form validation, stale targets and optimistic file edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum AdoptionError {
    /// A destination is not eligible.
    #[error("{message}")]
    Unsuitable {
        /// Reason to display beside the choice.
        message: String,
    },
    /// One editable value is invalid.
    #[error("{message}")]
    InvalidField {
        /// Field name (`name` or `start`).
        field: String,
        /// Validation explanation.
        message: String,
    },
    /// A known external file change requires a new preview.
    #[error("{path} cambió desde la revisión; vuelve a revisarlo")]
    DocumentChanged {
        /// The affected document.
        path: String,
    },
    /// The process identity, membership or observed command changed.
    #[error("la instancia cambió desde la revisión; vuelve a elegirla")]
    TargetChanged,
    /// The bounded review registry no longer has the handle.
    #[error("la revisión caducó; vuelve a preparar la propuesta")]
    PreviewExpired,
    /// The same handle was already applied or is applying different fields.
    #[error("esa revisión ya pertenece a otros valores; prepara una nueva")]
    RequestChanged,
    /// All registry slots are applying, so none may be evicted.
    #[error("hay demasiadas revisiones guardándose; espera a que terminen")]
    AtCapacity,
    /// The file cannot be safely edited in its current representation.
    #[error("{message}")]
    Unsupported {
        /// Safe editing limitation.
        message: String,
    },
    /// A file or worker operation failed.
    #[error("{message}")]
    Io {
        /// OS or runtime explanation.
        message: String,
    },
}

/// Facts supplied by the actor; never accepted as a client-controlled path.
pub(crate) struct AdoptionContext {
    pub view: InstanceView,
    pub projects: Vec<(String, PathBuf)>,
    pub shared: BTreeSet<String>,
    pub home: PathBuf,
}

/// Immutable proposal material; kept only until the review expires or succeeds.
#[derive(Debug, Clone)]
pub(crate) struct PreparedAdoption {
    id: String,
    reference: String,
    expected: crate::engine::StopExpectation,
    source_cwd: Option<String>,
    argv_digest: String,
    target: AdoptionTarget,
    root: PathBuf,
    project: String,
    pub(crate) file: FileSnapshot,
    service: ProposedService,
    shared: BTreeSet<String>,
}

/// Lists all choices, marking unsuitable destinations instead of offering them as valid.
pub(crate) fn options(context: &AdoptionContext) -> AdoptionOptions {
    let targets = context
        .projects
        .iter()
        .map(|(name, _)| AdoptionTarget::Project(name.clone()))
        .chain([AdoptionTarget::New(None), AdoptionTarget::Shared]);
    let destinations = targets
        .map(|target| match prepare(context, target.clone()) {
            Ok(prepared) => AdoptionDestination {
                target,
                eligible: true,
                reason: None,
                root: Some(prepared.root.display().to_string()),
                manifest: Some(prepared.file.path().display().to_string()),
                creates_manifest: prepared.file.text().ok().flatten().is_none(),
            },
            Err(error) => AdoptionDestination {
                target,
                eligible: false,
                reason: Some(error.to_string()),
                root: None,
                manifest: None,
                creates_manifest: false,
            },
        })
        .collect();
    AdoptionOptions {
        reference: context.view.instance_ref.clone(),
        destinations,
    }
}

/// Freezes the selected root, file version and service; never writes or restarts.
pub(crate) fn prepare(
    context: &AdoptionContext,
    target: AdoptionTarget,
) -> Result<PreparedAdoption, AdoptionError> {
    let mut view = context.view.clone();
    if view.target_identity.is_none()
        || view.service.is_some()
        || matches!(
            view.state,
            devports_proto::views::InstanceState::Down
                | devports_proto::views::InstanceState::Unknown
        )
    {
        return Err(unsuitable("esa instancia no está disponible para adopción"));
    }
    if matches!(target, AdoptionTarget::Shared) && view.container.is_none() {
        return Err(unsuitable(
            "solo los contenedores pueden declararse como compartidos",
        ));
    }
    if let Some(cwd) = &view.cwd
        && view.container.is_none()
    {
        view.cwd = Some(
            std::fs::canonicalize(cwd)
                .map_err(|error| unsuitable(&format!("no se puede resolver la carpeta: {error}")))?
                .display()
                .to_string(),
        );
    }
    let (project, path) = resolve(context, &view, &target)?;
    let file = FileSnapshot::capture(&path).map_err(document_error)?;
    let root = file
        .path()
        .parent()
        .ok_or_else(|| unsuitable("el destino no tiene raíz"))?
        .to_path_buf();
    if !matches!(target, AdoptionTarget::Shared) && file.text().map_err(document_error)?.is_none() {
        let home =
            std::fs::canonicalize(&context.home).map_err(|error| unsuitable(&error.to_string()))?;
        if root == home || root.components().count() <= 2 {
            return Err(unsuitable(
                "esa carpeta es demasiado amplia para crear un proyecto",
            ));
        }
    }
    if matches!(target, AdoptionTarget::New(_)) && file.text().map_err(document_error)?.is_some() {
        return Err(unsuitable(
            "ya hay un manifiesto en la raíz; elige ese proyecto y revisa su contenido",
        ));
    }
    let mut service = service_for(&root, &view)
        .ok_or_else(|| unsuitable("la carpeta del proceso está fuera de la raíz del proyecto"))?;
    if let Some(text) = file.text().map_err(document_error)? {
        if matches!(target, AdoptionTarget::Shared) {
            let (before, _) = SharedServices::parse(text, file.path())
                .map_err(|error| unsuitable(&error.to_string()))?;
            service.name = unused_name(&service.name, &before.services);
        } else {
            let before = manifest::parse_checked(text, file.path(), Some(&context.shared))
                .map_err(|error| unsuitable(&error.to_string()))?;
            if before.manifest.project != project {
                return Err(AdoptionError::DocumentChanged {
                    path: file.path().display().to_string(),
                });
            }
            service.name = unused_name(&service.name, &before.manifest.services);
        }
    }
    if service
        .start
        .as_ref()
        .is_some_and(|start| start.len() > MAX_START_BYTES || start.contains(['\n', '\r', '\0']))
    {
        service.start = None;
        service.review = Some("el comando observado no se representa como una línea; indica un comando o script estable".to_owned());
    }
    let prepared = PreparedAdoption {
        id: String::new(),
        reference: context.view.instance_ref.clone(),
        expected: crate::engine::StopExpectation::from_view(&context.view),
        source_cwd: context.view.cwd.clone(),
        argv_digest: argv_digest(&context.view.argv)?,
        target,
        root,
        project,
        file,
        service,
        shared: context.shared.clone(),
    };
    prepared.validate(&prepared.defaults())?;
    Ok(prepared)
}

impl PreparedAdoption {
    pub(crate) fn defaults(&self) -> ServiceEdits {
        ServiceEdits {
            name: self.service.name.clone(),
            start: self.service.start.clone(),
        }
    }

    pub(crate) fn matches(&self, view: &InstanceView) -> bool {
        self.expected == crate::engine::StopExpectation::from_view(view)
            && self.source_cwd == view.cwd
            && argv_digest(&view.argv).is_ok_and(|digest| digest == self.argv_digest)
    }

    pub(crate) fn destination_exists(&self, projects: &[(String, PathBuf)]) -> bool {
        match &self.target {
            AdoptionTarget::Project(name) => projects
                .iter()
                .any(|(candidate, root)| candidate == name && *root == self.root),
            AdoptionTarget::New(_) | AdoptionTarget::Shared => true,
        }
    }

    pub(crate) fn validate(&self, edits: &ServiceEdits) -> Result<AdoptionPreview, AdoptionError> {
        if !manifest::is_valid_name(&edits.name) {
            return Err(invalid(
                "name",
                "usa de 1 a 63 letras minúsculas, números o guiones",
            ));
        }
        if edits.start.as_ref().is_some_and(|start| {
            start.len() > MAX_START_BYTES
                || start.trim().is_empty()
                || start.contains(['\0', '\n', '\r'])
        }) {
            return Err(invalid(
                "start",
                "el comando debe tener una sola línea, sin NUL, entre 1 y 16384 bytes",
            ));
        }
        let mut service = self.service.clone();
        service.name.clone_from(&edits.name);
        service.start.clone_from(&edits.start);
        let mut warnings: Vec<String> = service.review.iter().cloned().collect();
        if service.start.is_none() {
            warnings.push(
                "no se pudo proponer un comando de inicio; puedes declararlo sin inicio automático"
                    .to_owned(),
            );
        }
        let before = self.file.text().map_err(document_error)?;
        let yaml = if matches!(self.target, AdoptionTarget::Shared) {
            let text = before.unwrap_or("version: 1\nservices: {}\n");
            let (parsed, _) = SharedServices::parse(text, self.file.path())
                .map_err(|error| unsuitable(&error.to_string()))?;
            if parsed.services.contains_key(&service.name) {
                return Err(invalid("name", "ya existe un servicio con ese nombre"));
            }
            let candidate = insert_service(text, &service.yaml_lines()).map_err(apply_error)?;
            SharedServices::parse(&candidate, self.file.path())
                .map_err(|error| unsuitable(&error.to_string()))?;
            candidate
        } else {
            let candidate = if let Some(text) = before {
                let parsed = manifest::parse_checked(text, self.file.path(), Some(&self.shared))
                    .map_err(|error| unsuitable(&error.to_string()))?;
                if parsed.manifest.services.contains_key(&service.name) {
                    return Err(invalid("name", "ya existe un servicio con ese nombre"));
                }
                insert_service(text, &service.yaml_lines()).map_err(apply_error)?
            } else {
                Proposal {
                    project: self.project.clone(),
                    root: self.root.clone(),
                    services: vec![service.clone()],
                }
                .to_yaml()
            };
            let loaded = manifest::parse_checked(&candidate, self.file.path(), Some(&self.shared))
                .map_err(|error| unsuitable(&error.to_string()))?;
            warnings.extend(loaded.warnings.iter().map(ToString::to_string));
            candidate
        };
        if yaml.len() > crate::document::MAX_DOCUMENT_BYTES {
            return Err(unsuitable(
                "el resultado supera el límite de edición automática",
            ));
        }
        Ok(AdoptionPreview {
            id: self.id.clone(),
            reference: self.reference.clone(),
            target: self.target.clone(),
            project: self.project.clone(),
            root: self.root.display().to_string(),
            manifest: self.file.path().display().to_string(),
            creates_manifest: before.is_none(),
            observed_port: self.expected.port,
            service,
            warnings,
            yaml,
        })
    }

    pub(crate) fn commit(&self, edits: &ServiceEdits) -> Result<Applied, AdoptionError> {
        if matches!(self.target, AdoptionTarget::New(_)) {
            let cwd = self
                .source_cwd
                .as_deref()
                .ok_or(AdoptionError::TargetChanged)?;
            let cwd = std::fs::canonicalize(cwd).map_err(|_| AdoptionError::TargetChanged)?;
            let root = crate::gitinfo::read(&cwd).map_or(cwd, |git| git.worktree);
            if root != self.root {
                return Err(AdoptionError::TargetChanged);
            }
        }
        let preview = self.validate(edits)?;
        self.file.commit(&preview.yaml).map_err(document_error)?;
        Ok(Applied {
            manifest: self.file.path().to_path_buf(),
            project: self.project.clone(),
            service: edits.name.clone(),
        })
    }
}

fn resolve(
    context: &AdoptionContext,
    view: &InstanceView,
    target: &AdoptionTarget,
) -> Result<(String, PathBuf), AdoptionError> {
    let (root, project) = match target {
        AdoptionTarget::Project(name) => {
            let root = context
                .projects
                .iter()
                .find(|(candidate, _)| candidate == name)
                .map(|(_, root)| root)
                .ok_or_else(|| unsuitable("ese proyecto ya no está en el catálogo"))?;
            (
                std::fs::canonicalize(root).map_err(|error| unsuitable(&error.to_string()))?,
                name.clone(),
            )
        }
        AdoptionTarget::New(name) => {
            if view.container.is_some() {
                return Err(unsuitable(
                    "para ese contenedor elige un proyecto existente o Compartidos",
                ));
            }
            let cwd = view
                .cwd
                .as_deref()
                .map(Path::new)
                .ok_or_else(|| unsuitable("el proceso no tiene carpeta conocida"))?;
            let root =
                crate::gitinfo::read(cwd).map_or_else(|| cwd.to_path_buf(), |git| git.worktree);
            let project = name.as_deref().map_or_else(
                || {
                    slug(
                        root.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("proyecto"),
                    )
                },
                slug,
            );
            (root, project)
        }
        AdoptionTarget::Shared => {
            let root = crate::config::config_dir(&context.home);
            return Ok(("compartidos".to_owned(), root.join("shared.yml")));
        }
    };
    if matches!(target, AdoptionTarget::New(_)) {
        let home =
            std::fs::canonicalize(&context.home).map_err(|error| unsuitable(&error.to_string()))?;
        if root == home || root.components().count() <= 2 {
            return Err(unsuitable(
                "esa carpeta es demasiado amplia para ser un proyecto",
            ));
        }
    }
    let path = root.join(MANIFEST_FILE);
    Ok((project, path))
}

fn argv_digest(argv: &[String]) -> Result<String, AdoptionError> {
    serde_json::to_vec(argv)
        .map(|bytes| crate::hash::sha256_hex(&bytes))
        .map_err(|error| AdoptionError::Io {
            message: error.to_string(),
        })
}

fn unsuitable(message: &str) -> AdoptionError {
    AdoptionError::Unsuitable {
        message: message.to_owned(),
    }
}
fn invalid(field: &str, message: &str) -> AdoptionError {
    AdoptionError::InvalidField {
        field: field.to_owned(),
        message: message.to_owned(),
    }
}
fn apply_error(error: super::apply::ApplyError) -> AdoptionError {
    match error {
        super::apply::ApplyError::Unsupported(message) => AdoptionError::Unsupported { message },
        other => unsuitable(&other.to_string()),
    }
}
pub(crate) fn document_error(error: EditError) -> AdoptionError {
    match error {
        EditError::Conflict(path) => AdoptionError::DocumentChanged {
            path: path.display().to_string(),
        },
        EditError::Unsupported(message) => AdoptionError::Unsupported { message },
        other => AdoptionError::Io {
            message: other.to_string(),
        },
    }
}
