//! Writing a manifest: the part of adoption that changes files.
//!
//! Only the application calls this, on a person's click (RF-05, ADR-009); no
//! socket operation reaches it. Existing manifests are edited as text so the
//! comments and layout the person wrote survive, and every result is validated
//! with the same parser the core reads manifests with before it replaces the
//! file; a result that does not validate is never written.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use devports_proto::views::InstanceView;

use super::{ProposedService, propose, service_for, slug};
use crate::manifest::{self, MANIFEST_FILE};

/// What was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The manifest that now describes the service.
    pub manifest: PathBuf,
    /// Its project.
    pub project: String,
    /// The service's name in it.
    pub service: String,
}

/// Why adopting failed. Nothing is written in any of these cases.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    /// A manifest already exists where a new one would go.
    #[error("ya existe {}", .0.display())]
    Exists(PathBuf),
    /// The process or container cannot be described inside that project.
    #[error("{0}")]
    Unsuitable(String),
    /// The service name is already used in the manifest.
    #[error("el manifiesto ya tiene un servicio llamado «{0}»")]
    Duplicate(String),
    /// The manifest could not be edited safely.
    #[error("{0}")]
    Unsupported(String),
    /// The manifest, before or after the edit, does not validate.
    #[error("el manifiesto no es válido: {0}")]
    Invalid(String),
    /// The file could not be read or written.
    #[error("{0}")]
    Io(String),
}

/// Adds `view` as a service to the manifest of the project rooted at `root`.
///
/// # Errors
///
/// See [`ApplyError`].
pub fn add_service(
    root: &Path,
    view: &InstanceView,
    known_shared: Option<&BTreeSet<String>>,
) -> Result<Applied, ApplyError> {
    let path = root.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| ApplyError::Io(format!("{}: {error}", path.display())))?;
    let before = manifest::parse_checked(&text, &path, known_shared)
        .map_err(|error| ApplyError::Invalid(error.to_string()))?;
    let service = service_for(root, view).ok_or_else(|| {
        ApplyError::Unsuitable(
            "la carpeta del proceso no está dentro del proyecto: créalo como proyecto nuevo"
                .to_owned(),
        )
    })?;
    let name = unused_name(&service.name, &before.manifest.services);
    let service = ProposedService { name, ..service };
    let edited = insert_service(&text, &service.yaml_lines())?;

    let after = manifest::parse_checked(&edited, &path, known_shared)
        .map_err(|error| ApplyError::Invalid(error.to_string()))?;
    if !after.manifest.services.contains_key(&service.name) {
        return Err(ApplyError::Invalid(
            "el servicio no quedó en el manifiesto".to_owned(),
        ));
    }
    write_atomic(&path, &edited)?;
    Ok(Applied {
        manifest: path,
        project: after.manifest.project,
        service: service.name,
    })
}

/// Declares the container `view` as a shared service in `shared.yml` (created
/// when missing).
///
/// # Errors
///
/// See [`ApplyError`].
pub fn add_shared(path: &Path, view: &InstanceView) -> Result<Applied, ApplyError> {
    if view.container.is_none() {
        return Err(ApplyError::Unsuitable(
            "solo un contenedor puede ser un servicio compartido".to_owned(),
        ));
    }
    let service = container_service_for(view)?;
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "version: 1\nservices: {}\n".to_owned()
        }
        Err(error) => return Err(ApplyError::Io(format!("{}: {error}", path.display()))),
    };
    let (before, _) = crate::config::SharedServices::parse(&existing, path)
        .map_err(|error| ApplyError::Invalid(error.to_string()))?;
    let name = unused_name(&service.name, &before.services);
    let service = ProposedService { name, ..service };
    let edited = insert_service(&existing, &service.yaml_lines())?;
    let (after, _) = crate::config::SharedServices::parse(&edited, path)
        .map_err(|error| ApplyError::Invalid(error.to_string()))?;
    if !after.services.contains_key(&service.name) {
        return Err(ApplyError::Invalid(
            "el servicio no quedó en shared.yml".to_owned(),
        ));
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| ApplyError::Io(format!("{}: {error}", folder.display())))?;
    }
    write_atomic(path, &edited)?;
    Ok(Applied {
        manifest: path.to_path_buf(),
        project: "compartidos".to_owned(),
        service: service.name,
    })
}

fn container_service_for(view: &InstanceView) -> Result<ProposedService, ApplyError> {
    service_for(Path::new("/"), view)
        .ok_or_else(|| ApplyError::Unsuitable("no se pudo describir ese contenedor".to_owned()))
}

/// Creates a manifest in `root` for the project `name` (the folder's name by
/// default) with `view` as its first service.
///
/// # Errors
///
/// See [`ApplyError`].
pub fn create_project(
    root: &Path,
    name: Option<&str>,
    view: &InstanceView,
    known_shared: Option<&BTreeSet<String>>,
) -> Result<Applied, ApplyError> {
    let path = root.join(MANIFEST_FILE);
    if path.exists() {
        return Err(ApplyError::Exists(path));
    }
    let mut proposal = propose(root, std::slice::from_ref(&with_no_service(view)));
    if proposal.services.is_empty() {
        return Err(ApplyError::Unsuitable(
            "no se pudo describir ese proceso desde esa carpeta".to_owned(),
        ));
    }
    if let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) {
        proposal.project = slug(name);
    }
    let text = proposal.to_yaml();
    let loaded = manifest::parse_checked(&text, &path, known_shared)
        .map_err(|error| ApplyError::Invalid(error.to_string()))?;
    let service = proposal
        .services
        .first()
        .map(|service| service.name.clone())
        .unwrap_or_default();
    write_atomic(&path, &text)?;
    Ok(Applied {
        manifest: path,
        project: loaded.manifest.project,
        service,
    })
}

fn with_no_service(view: &InstanceView) -> InstanceView {
    InstanceView {
        service: None,
        ..view.clone()
    }
}

/// `name`, or `name-2`, `name-3`… when taken.
fn unused_name<V>(name: &str, taken: &std::collections::BTreeMap<String, V>) -> String {
    if !taken.contains_key(name) {
        return name.to_owned();
    }
    (2_u32..=u32::MAX)
        .map(|number| format!("{name}-{number}"))
        .find(|candidate| !taken.contains_key(candidate))
        .unwrap_or_else(|| name.to_owned())
}

/// Appends `block` (a service written with two- and four-space indentation) to
/// the `services:` mapping of `text`, matching the indentation the file uses.
fn insert_service(text: &str, block: &[String]) -> Result<String, ApplyError> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let services_at = lines.iter().position(|line| {
        line.strip_prefix("services:")
            .is_some_and(|rest| rest.trim().is_empty() || rest.trim_start().starts_with('#'))
            || line.trim_end() == "services: {}"
    });
    let Some(services_at) = services_at else {
        // A valid manifest always has `services`; this is its flow-style spelling.
        return Err(ApplyError::Unsupported(
            "el manifiesto escribe `services` en una forma que DevPorts no edita; agrega el servicio a mano"
                .to_owned(),
        ));
    };

    if let Some(line) = lines.get_mut(services_at)
        && line.trim_end() == "services: {}"
    {
        "services:".clone_into(line);
    }
    // The mapping ends before the next line that starts at column zero (a new
    // top-level key); comments and blank lines at its end stay after the block.
    let mut end = services_at.saturating_add(1);
    let mut last_content = services_at;
    let mut child_indent: Option<usize> = None;
    while let Some(line) = lines.get(end) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            end = end.saturating_add(1);
            continue;
        }
        let indent = line.len().saturating_sub(line.trim_start().len());
        if indent == 0 {
            break;
        }
        child_indent.get_or_insert(indent);
        last_content = end;
        end = end.saturating_add(1);
    }
    let unit = child_indent.unwrap_or(2);
    let reindented: Vec<String> = block
        .iter()
        .map(|line| {
            let own = line.len().saturating_sub(line.trim_start().len());
            let level = own.checked_div(2).unwrap_or(0);
            format!(
                "{}{}",
                " ".repeat(unit.saturating_mul(level)),
                line.trim_start()
            )
        })
        .collect();
    let at = last_content.saturating_add(1);
    for (offset, line) in reindented.into_iter().enumerate() {
        lines.insert(at.saturating_add(offset), line);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

/// Writes through a temporary file in the same folder and renames it over the
/// target, so a crash never leaves a half-written manifest.
fn write_atomic(path: &Path, text: &str) -> Result<(), ApplyError> {
    let temp = path.with_extension("yml.devports-tmp");
    std::fs::write(&temp, text)
        .and_then(|()| std::fs::rename(&temp, path))
        .map_err(|error| {
            let _ = std::fs::remove_file(&temp);
            ApplyError::Io(format!("{}: {error}", path.display()))
        })
}

#[cfg(test)]
mod tests;
