//! The project manifest, `.devports.yml` (`MANIFIESTO.md`, section 1), with its
//! validation.
//!
//! Reading a manifest never executes anything. Problems come back as
//! [`Diagnostic`]s with the line they are on: *errors* mean the manifest is not
//! loaded, *warnings* mean it is loaded but probably not as intended. A broken
//! manifest never takes the others down with it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::yaml_keys::{self, KeyIndex};

/// Name of the manifest file inside a project folder.
pub const MANIFEST_FILE: &str = ".devports.yml";

/// The only schema version this build understands.
pub const SUPPORTED_VERSION: u32 = 1;

/// Keys that mean something at the top level of a manifest.
const TOP_LEVEL_KEYS: [&str; 4] = ["version", "project", "uses", "services"];
/// Keys that mean something inside a service.
pub(crate) const SERVICE_KEYS: [&str; 7] =
    ["cwd", "docker", "start", "port", "health", "kind", "match"];

/// A validated `.devports.yml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Schema version; always [`SUPPORTED_VERSION`].
    pub version: u32,
    /// Project name shown to people and accepted by `--project`.
    pub project: String,
    /// Names of the shared services (`shared.yml`) this project depends on.
    pub uses: Vec<String>,
    /// The project's own services, by name.
    pub services: BTreeMap<String, ServiceSpec>,
}

/// One service as written in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceSpec {
    /// Working directory, relative to the manifest. Host services.
    pub cwd: Option<String>,
    /// Container name. Container services.
    pub docker: Option<String>,
    /// Command that starts the service; may contain `${PORT}`.
    pub start: Option<String>,
    /// Preferred port of the main instance. Not part of the identity.
    pub port: Option<u16>,
    /// How to tell the service is alive.
    pub health: Health,
    /// `app` (default) or `data`; data services are never stopped automatically.
    pub kind: ServiceKind,
    /// Fingerprint of the service's command line.
    pub match_text: Option<String>,
}

/// How a service's health is checked (`MANIFIESTO.md`, section 5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Health {
    /// A TCP connection to the port succeeds.
    #[default]
    Tcp,
    /// `GET <path>` answers 200 to 399.
    Http(String),
}

/// Whether a service holds state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceKind {
    /// A stateless application server.
    #[default]
    App,
    /// A database, cache or other stateful service.
    Data,
}

impl ServiceKind {
    /// The word used in the files and the interface.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Data => "data",
        }
    }
}

/// Why a manifest could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// The file could not be read.
    #[error("{}: no se pudo leer el manifiesto: {reason}", path.display())]
    Read {
        /// The manifest file.
        path: PathBuf,
        /// The operating system's explanation.
        reason: String,
    },
    /// The file has errors; it is not loaded.
    #[error("{}: {}", path.display(), summary(diagnostics))]
    Invalid {
        /// The manifest file.
        path: PathBuf,
        /// Every problem found, errors and warnings.
        diagnostics: Vec<Diagnostic>,
    },
}

fn summary(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == crate::diagnostic::Severity::Error)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// A manifest that loaded, with the warnings it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// The manifest.
    pub manifest: Manifest,
    /// Problems that did not stop it loading.
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Default, Deserialize)]
struct RawManifest {
    version: Option<i64>,
    project: Option<String>,
    uses: Option<Vec<String>>,
    services: Option<BTreeMap<String, RawService>>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawService {
    cwd: Option<String>,
    docker: Option<String>,
    start: Option<String>,
    port: Option<i64>,
    health: Option<String>,
    kind: Option<String>,
    #[serde(rename = "match")]
    match_text: Option<String>,
}

/// Validates manifest text. `known_shared` is the set of names defined in
/// `shared.yml`; when given, `uses` entries outside it are warned about.
///
/// # Errors
///
/// [`ManifestError::Invalid`] if there is any error-level problem.
pub fn parse_checked(
    text: &str,
    path: &Path,
    known_shared: Option<&BTreeSet<String>>,
) -> Result<Loaded, ManifestError> {
    let invalid = |diagnostics| ManifestError::Invalid {
        path: path.to_path_buf(),
        diagnostics,
    };
    let raw: RawManifest = match serde_saphyr::from_str(text) {
        Ok(raw) => raw,
        Err(error) => {
            let line = error.location().map(|location| location.line());
            return Err(invalid(vec![Diagnostic::error(line, error.to_string())]));
        }
    };
    let keys = yaml_keys::index(text);
    let mut diagnostics = Vec::new();

    unknown_keys(&keys, &[], &TOP_LEVEL_KEYS, &mut diagnostics);

    let version_line = keys.line(&["version"]);
    match raw.version {
        None => diagnostics.push(Diagnostic::error(None, "falta «version»; debe ser 1")),
        Some(version) if version == i64::from(SUPPORTED_VERSION) => {}
        Some(version) => diagnostics.push(Diagnostic::error(
            version_line,
            format!("versión {version} no soportada; actualiza devports"),
        )),
    }

    let project = match raw.project {
        Some(name) if is_valid_name(&name) => Some(name),
        Some(name) => {
            diagnostics.push(Diagnostic::error(
                keys.line(&["project"]),
                format!("el nombre de proyecto «{name}» no es válido: usa minúsculas, números y guiones, hasta 63 caracteres, sin empezar por guion"),
            ));
            None
        }
        None => {
            diagnostics.push(Diagnostic::error(None, "falta «project»"));
            None
        }
    };

    let uses = raw.uses.unwrap_or_default();
    if let Some(shared) = known_shared {
        for name in &uses {
            if !shared.contains(name) {
                diagnostics.push(Diagnostic::warning(
                    keys.line(&["uses"]),
                    format!("«{name}» no está definido en shared.yml"),
                ));
            }
        }
    }

    let services = if let Some(services) = raw.services {
        validate_services(&services, &keys, ServiceFile::Project, &mut diagnostics)
    } else {
        diagnostics.push(Diagnostic::error(
            None,
            "falta «services» (puede estar vacío: `services: {}`)",
        ));
        BTreeMap::new()
    };

    if has_errors(&diagnostics) {
        return Err(invalid(diagnostics));
    }
    match project {
        Some(project) => Ok(Loaded {
            manifest: Manifest {
                version: SUPPORTED_VERSION,
                project,
                uses,
                services,
            },
            warnings: diagnostics,
        }),
        None => Err(invalid(diagnostics)),
    }
}

/// Parses manifest text and returns the manifest, discarding warnings.
///
/// # Errors
///
/// See [`parse_checked`].
pub fn parse(text: &str, path: &Path) -> Result<Manifest, ManifestError> {
    parse_checked(text, path, None).map(|loaded| loaded.manifest)
}

/// Reads and validates the manifest at `path`.
///
/// # Errors
///
/// [`ManifestError::Read`] if the file cannot be read, otherwise as [`parse_checked`].
pub fn load(path: &Path, known_shared: Option<&BTreeSet<String>>) -> Result<Loaded, ManifestError> {
    let text = std::fs::read_to_string(path).map_err(|error| ManifestError::Read {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    parse_checked(&text, path, known_shared)
}

/// Which file a service came from; the rules for `cwd` differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceFile {
    /// `.devports.yml`: `cwd` is relative to the manifest and stays inside the project.
    Project,
    /// `shared.yml`: `cwd` is absolute (or starts with `~/`).
    Shared,
}

/// Validates the `services` mapping shared by manifests and `shared.yml`.
pub(crate) fn validate_services(
    raw: &BTreeMap<String, RawService>,
    keys: &KeyIndex,
    file: ServiceFile,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, ServiceSpec> {
    let mut services = BTreeMap::new();
    let mut ports: BTreeMap<u16, String> = BTreeMap::new();
    for (name, service) in raw {
        let line = keys.line(&["services", name]);
        if !is_valid_name(name) {
            diagnostics.push(Diagnostic::error(
                line,
                format!(
                    "el nombre de servicio «{name}» no es válido: usa minúsculas, números y guiones"
                ),
            ));
        }
        unknown_keys(keys, &["services", name], &SERVICE_KEYS, diagnostics);
        let check = ServiceCheck { name, keys, file };
        let mut spec = ServiceSpec::default();
        check.target(service, &mut spec, diagnostics);
        check.start(service, &mut spec, diagnostics);
        check.port(service, &mut spec, &mut ports, diagnostics);
        check.health_and_kind(service, &mut spec, diagnostics);
        check.match_text(service, &mut spec, diagnostics);
        services.insert(name.clone(), spec);
    }
    services
}

/// The context of the service being validated.
struct ServiceCheck<'a> {
    name: &'a str,
    keys: &'a KeyIndex,
    file: ServiceFile,
}

impl ServiceCheck<'_> {
    fn at(&self, field: &str) -> Option<u64> {
        self.keys.line(&["services", self.name, field])
    }

    fn target(
        &self,
        service: &RawService,
        spec: &mut ServiceSpec,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let name = self.name;
        let line = self.keys.line(&["services", name]);
        match (&service.cwd, &service.docker) {
            (Some(_), Some(_)) => diagnostics.push(Diagnostic::error(
                line,
                format!("«{name}» tiene «cwd» y «docker»: debe tener exactamente uno"),
            )),
            (None, None) => diagnostics.push(Diagnostic::error(
                line,
                format!("«{name}» necesita «cwd» (servicio del host) o «docker» (contenedor)"),
            )),
            (Some(cwd), None) => {
                check_cwd(name, cwd, self.file, self.at("cwd"), diagnostics);
                spec.cwd = Some(cwd.clone());
            }
            (None, Some(container)) => {
                if container.trim().is_empty() {
                    diagnostics.push(Diagnostic::error(
                        self.at("docker"),
                        format!("«docker» de «{name}» está vacío"),
                    ));
                }
                spec.docker = Some(container.clone());
            }
        }
    }

    fn start(
        &self,
        service: &RawService,
        spec: &mut ServiceSpec,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(start) = &service.start else { return };
        if start.contains('\n') || start.contains('\r') {
            diagnostics.push(Diagnostic::error(
                self.at("start"),
                format!(
                    "«start» de «{}» debe ser un solo comando, sin saltos de línea; para varios pasos usa un script",
                    self.name
                ),
            ));
        }
        spec.start = Some(start.clone());
    }

    fn port(
        &self,
        service: &RawService,
        spec: &mut ServiceSpec,
        ports: &mut BTreeMap<u16, String>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let (name, Some(port)) = (self.name, service.port) else {
            return;
        };
        let Some(port) = u16::try_from(port).ok().filter(|port| *port >= 1) else {
            diagnostics.push(Diagnostic::error(
                self.at("port"),
                format!("«port» de «{name}» debe estar entre 1 y 65535, no {port}"),
            ));
            return;
        };
        if port < 1024 {
            diagnostics.push(Diagnostic::warning(
                self.at("port"),
                format!("«port» de «{name}» es {port}, menor que 1024: puede requerir privilegios"),
            ));
        }
        if let Some(other) = ports.insert(port, name.to_owned()) {
            diagnostics.push(Diagnostic::error(
                self.at("port"),
                format!("el puerto {port} ya lo usa «{other}»: dentro de un proyecto cada puerto preferido es único"),
            ));
        }
        spec.port = Some(port);
    }

    fn health_and_kind(
        &self,
        service: &RawService,
        spec: &mut ServiceSpec,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let name = self.name;
        if let Some(health) = &service.health {
            match parse_health(health) {
                Some(parsed) => spec.health = parsed,
                None => diagnostics.push(Diagnostic::error(
                    self.at("health"),
                    format!("«health» de «{name}» debe ser `tcp` o `http:/ruta`, no «{health}»"),
                )),
            }
        }
        if let Some(kind) = &service.kind {
            match kind.as_str() {
                "app" => spec.kind = ServiceKind::App,
                "data" => spec.kind = ServiceKind::Data,
                other => diagnostics.push(Diagnostic::error(
                    self.at("kind"),
                    format!("«kind» de «{name}» debe ser `app` o `data`, no «{other}»"),
                )),
            }
        }
    }

    fn match_text(
        &self,
        service: &RawService,
        spec: &mut ServiceSpec,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(match_text) = &service.match_text else {
            return;
        };
        if match_text.trim().is_empty() {
            diagnostics.push(Diagnostic::warning(
                self.at("match"),
                format!(
                    "«match» de «{}» está vacío: no reconocerá ningún proceso",
                    self.name
                ),
            ));
        }
        spec.match_text = Some(match_text.clone());
    }
}

fn parse_health(text: &str) -> Option<Health> {
    if text == "tcp" {
        Some(Health::Tcp)
    } else {
        text.strip_prefix("http:")
            .filter(|path| path.starts_with('/'))
            .map(|path| Health::Http(path.to_owned()))
    }
}

fn check_cwd(
    name: &str,
    cwd: &str,
    file: ServiceFile,
    line: Option<u64>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if cwd.trim().is_empty() {
        diagnostics.push(Diagnostic::error(
            line,
            format!("«cwd» de «{name}» está vacío"),
        ));
        return;
    }
    let absolute = cwd.starts_with('/') || cwd.starts_with("~/") || cwd == "~";
    match file {
        ServiceFile::Shared => {
            if !absolute {
                diagnostics.push(Diagnostic::error(
                    line,
                    format!("«cwd» de «{name}» debe ser una ruta absoluta (o empezar por ~/) en shared.yml"),
                ));
            }
        }
        ServiceFile::Project => {
            if absolute {
                diagnostics.push(Diagnostic::error(
                    line,
                    format!("«cwd» de «{name}» debe ser relativa al manifiesto, no «{cwd}»"),
                ));
            } else if escapes(cwd) {
                diagnostics.push(Diagnostic::error(
                    line,
                    format!("«cwd» de «{name}» sale de la carpeta del proyecto: «{cwd}»"),
                ));
            }
        }
    }
}

/// Whether a relative path climbs above its starting point.
fn escapes(path: &str) -> bool {
    let mut depth: usize = 0;
    for component in Path::new(path).components() {
        match component {
            Component::ParentDir => {
                if depth == 0 {
                    return true;
                }
                depth = depth.saturating_sub(1);
            }
            Component::Normal(_) => depth = depth.saturating_add(1),
            _ => {}
        }
    }
    false
}

/// Warns about keys under `parent` that are not in `known`.
pub(crate) fn unknown_keys(
    keys: &KeyIndex,
    parent: &[&str],
    known: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (key, line) in keys.children(parent) {
        if known.contains(&key.as_str()) {
            continue;
        }
        let message = if key == "test" {
            "«test» ignorada: devports no gestiona configuración de pruebas (ADR-010)".to_owned()
        } else {
            format!("clave desconocida «{key}»; se ignora")
        };
        diagnostics.push(Diagnostic::warning(Some(line), message));
    }
}

/// `^[a-z0-9][a-z0-9-]{0,62}$`.
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    name.len() <= 63
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[cfg(test)]
mod tests;
