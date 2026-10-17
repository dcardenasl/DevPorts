//! The project manifest, `.devports.yml` (`MANIFIESTO.md`, section 1).
//!
//! This module reads the file into typed data and rejects only what makes it
//! unusable: unreadable, not valid YAML or an unsupported `version`. The full
//! rule set (formats, uniqueness, one of `cwd` or `docker`, warnings) belongs to
//! the validator of task T1.2, which builds on these types.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Name of the manifest file inside a project folder.
pub const MANIFEST_FILE: &str = ".devports.yml";

/// The only schema version this build understands.
pub const SUPPORTED_VERSION: u32 = 1;

/// A parsed `.devports.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    /// Schema version; always [`SUPPORTED_VERSION`] once loaded.
    pub version: u32,
    /// Project name shown to people and accepted by `--project`.
    pub project: String,
    /// Names of the shared services (`shared.yml`) this project depends on.
    #[serde(default)]
    pub uses: Vec<String>,
    /// The project's own services, by name.
    #[serde(default)]
    pub services: BTreeMap<String, ServiceSpec>,
}

/// One service as written in the manifest. Unknown keys are ignored, so a newer
/// manifest still loads (the validator reports them as warnings).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct ServiceSpec {
    /// Working directory, relative to the manifest. Host services.
    pub cwd: Option<String>,
    /// Container name. Container services.
    pub docker: Option<String>,
    /// Command that starts the service; may contain `${PORT}`.
    pub start: Option<String>,
    /// Preferred port of the main instance. Not part of the identity.
    pub port: Option<u16>,
    /// Health check: `tcp` or `http:/path`.
    pub health: Option<String>,
    /// `app` (default) or `data`; data services are never stopped automatically.
    #[serde(default)]
    pub kind: ServiceKind,
    /// Fingerprint of the service's command line.
    #[serde(rename = "match")]
    pub match_text: Option<String>,
}

/// Whether a service holds state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceKind {
    /// A stateless application server.
    #[default]
    App,
    /// A database, cache or other stateful service.
    Data,
}

/// Why a manifest could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// The file could not be read.
    #[error("{}: could not read the manifest: {reason}", path.display())]
    Read {
        /// The manifest file.
        path: PathBuf,
        /// The operating system's explanation.
        reason: String,
    },
    /// The file is not valid YAML or does not have the expected shape.
    #[error("{}: {message}", path.display())]
    Parse {
        /// The manifest file.
        path: PathBuf,
        /// The parser's message, which includes the line and column.
        message: String,
    },
    /// The file declares a schema version this build does not understand.
    #[error("{}: unsupported manifest version {found}; update devports", path.display())]
    UnsupportedVersion {
        /// The manifest file.
        path: PathBuf,
        /// The version found in the file.
        found: u32,
    },
}

/// Parses manifest text. `path` is only used to label errors.
///
/// # Errors
///
/// [`ManifestError::Parse`] for invalid YAML or a wrong shape, and
/// [`ManifestError::UnsupportedVersion`] for a `version` other than 1.
pub fn parse(text: &str, path: &Path) -> Result<Manifest, ManifestError> {
    let manifest: Manifest =
        serde_saphyr::from_str(text).map_err(|error| ManifestError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if manifest.version == SUPPORTED_VERSION {
        Ok(manifest)
    } else {
        Err(ManifestError::UnsupportedVersion {
            path: path.to_path_buf(),
            found: manifest.version,
        })
    }
}

/// Reads and parses the manifest at `path`.
///
/// # Errors
///
/// [`ManifestError::Read`] if the file cannot be read, otherwise as [`parse`].
pub fn load(path: &Path) -> Result<Manifest, ManifestError> {
    let text = std::fs::read_to_string(path).map_err(|error| ManifestError::Read {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    parse(&text, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = "\
version: 1
project: tienda-demo
uses: [mysql]
services:
  api:
    cwd: ./tienda-api
    start: php spark serve --port ${PORT}
    port: 8080
    health: http:/health
    match: php spark serve
  cache:
    docker: tienda-redis
    kind: data
    port: 6379
    extra_key: ignored
";

    #[test]
    fn parses_the_documented_example() {
        let manifest = parse(FULL, Path::new("m.yml")).unwrap();
        assert_eq!(manifest.project, "tienda-demo");
        assert_eq!(manifest.uses, ["mysql"]);
        let api = &manifest.services["api"];
        assert_eq!(api.cwd.as_deref(), Some("./tienda-api"));
        assert_eq!(api.port, Some(8080));
        assert_eq!(api.match_text.as_deref(), Some("php spark serve"));
        assert_eq!(api.kind, ServiceKind::App);
        let cache = &manifest.services["cache"];
        assert_eq!(cache.docker.as_deref(), Some("tienda-redis"));
        assert_eq!(cache.kind, ServiceKind::Data);
    }

    #[test]
    fn a_manifest_without_services_is_valid() {
        let manifest = parse("version: 1\nproject: empty\n", Path::new("m.yml")).unwrap();
        assert!(manifest.services.is_empty());
        assert!(manifest.uses.is_empty());
    }

    #[test]
    fn another_version_is_refused() {
        let error = parse("version: 2\nproject: x\n", Path::new("m.yml")).unwrap_err();
        assert!(matches!(
            error,
            ManifestError::UnsupportedVersion { found: 2, .. }
        ));
    }

    #[test]
    fn invalid_yaml_reports_where_it_failed() {
        let error = parse("version: 1\nproject: [oops\n", Path::new("m.yml")).unwrap_err();
        let ManifestError::Parse { message, .. } = error else {
            panic!("expected a parse error, got {error:?}");
        };
        assert!(message.contains("line"), "no position in: {message}");
    }

    #[test]
    fn a_missing_required_field_is_a_parse_error() {
        let error = parse("version: 1\n", Path::new("m.yml")).unwrap_err();
        assert!(matches!(error, ManifestError::Parse { .. }));
    }

    #[test]
    fn a_missing_file_is_a_read_error() {
        let error = load(Path::new("/definitely/not/here/.devports.yml")).unwrap_err();
        assert!(matches!(error, ManifestError::Read { .. }));
    }
}
