//! Finding the Docker daemon the user is actually using.
//!
//! The Docker CLI decides where to connect from, in order: `DOCKER_HOST`, the
//! context named by `DOCKER_CONTEXT`, the `currentContext` of
//! `~/.docker/config.json`, and finally the `default` context
//! (`/var/run/docker.sock`). Docker Desktop, OrbStack and Colima each register
//! their own context, so following the same rules reaches whichever one is
//! active without any hard-coded path (ADR-012).
//!
//! `ARQUITECTURA.md` names `docker context inspect` for this. The context store
//! is a plain directory of JSON files, so it is read directly: no process is
//! spawned and the `docker` binary need not be installed (ADR-025, point 3).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::hash::sha256_hex;

/// Name of the context Docker uses when nothing else is configured.
const DEFAULT_CONTEXT: &str = "default";
/// Where the `default` context points when `DOCKER_HOST` is not set.
const DEFAULT_SOCKET: &str = "/var/run/docker.sock";

/// Where to reach a Docker daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// A Unix socket on this machine.
    Unix(PathBuf),
    /// Any other transport (`tcp://`, `ssh://`), kept as written. Not supported.
    Other(String),
}

impl Endpoint {
    /// Parses a Docker host string such as `unix:///var/run/docker.sock`.
    #[must_use]
    pub fn parse(host: &str) -> Self {
        match host.strip_prefix("unix://") {
            Some(path) if !path.is_empty() => Self::Unix(PathBuf::from(path)),
            _ => Self::Other(host.to_owned()),
        }
    }
}

/// The context that was chosen and where it points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    /// Context name (`default`, `desktop-linux`, `orbstack`, ...).
    pub name: String,
    /// Where its daemon listens.
    pub endpoint: Endpoint,
}

/// Everything the resolution depends on, passed in so it is testable.
#[derive(Debug, Clone, Default)]
pub struct ContextEnv {
    /// `DOCKER_HOST`.
    pub docker_host: Option<String>,
    /// `DOCKER_CONTEXT`.
    pub docker_context: Option<String>,
    /// `DOCKER_CONFIG`, the directory that holds `config.json` and `contexts/`.
    pub docker_config: Option<PathBuf>,
    /// The user's home directory, for the default `~/.docker`.
    pub home: Option<PathBuf>,
}

impl ContextEnv {
    /// Reads the variables from the current process environment.
    #[must_use]
    pub fn from_process() -> Self {
        let text = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        Self {
            docker_host: text("DOCKER_HOST"),
            docker_context: text("DOCKER_CONTEXT"),
            docker_config: text("DOCKER_CONFIG").map(PathBuf::from),
            home: std::env::var_os("HOME").map(PathBuf::from),
        }
    }

    fn config_dir(&self) -> Option<PathBuf> {
        self.docker_config
            .clone()
            .or_else(|| self.home.as_ref().map(|home| home.join(".docker")))
    }
}

/// Why no context could be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContextError {
    /// There is no Docker configuration directory to read.
    #[error("no Docker configuration directory (HOME and DOCKER_CONFIG are unset)")]
    NoConfigDir,
    /// The named context has no entry in the context store.
    #[error("Docker context `{0}` does not exist")]
    UnknownContext(String),
    /// A file of the context store could not be read or understood.
    #[error("{}: {reason}", path.display())]
    Unreadable {
        /// The file.
        path: PathBuf,
        /// What went wrong.
        reason: String,
    },
    /// The context has no `docker` endpoint.
    #[error("Docker context `{0}` has no docker endpoint")]
    NoEndpoint(String),
}

/// Resolves the active Docker context the way the Docker CLI does.
///
/// # Errors
///
/// Fails when the named context is missing or its files are unreadable.
pub fn resolve(env: &ContextEnv) -> Result<ResolvedContext, ContextError> {
    if let Some(host) = &env.docker_host {
        return Ok(ResolvedContext {
            name: DEFAULT_CONTEXT.to_owned(),
            endpoint: Endpoint::parse(host),
        });
    }

    let name = match &env.docker_context {
        Some(name) => name.clone(),
        None => current_context(env)?.unwrap_or_else(|| DEFAULT_CONTEXT.to_owned()),
    };
    if name == DEFAULT_CONTEXT {
        return Ok(ResolvedContext {
            name,
            endpoint: Endpoint::Unix(PathBuf::from(DEFAULT_SOCKET)),
        });
    }

    let meta_path = env
        .config_dir()
        .ok_or(ContextError::NoConfigDir)?
        .join("contexts")
        .join("meta")
        .join(context_directory(&name))
        .join("meta.json");
    let meta: Meta =
        read_json(&meta_path)?.ok_or_else(|| ContextError::UnknownContext(name.clone()))?;
    let host = meta
        .endpoints
        .docker
        .map(|endpoint| endpoint.host)
        .ok_or_else(|| ContextError::NoEndpoint(name.clone()))?;
    Ok(ResolvedContext {
        name,
        endpoint: Endpoint::parse(&host),
    })
}

/// Name of the directory that holds a context in the store: the SHA-256 of its
/// name, in lowercase hexadecimal.
#[must_use]
pub fn context_directory(name: &str) -> String {
    sha256_hex(name.as_bytes())
}

/// `currentContext` of `config.json`. A missing config file is not an error: it
/// just means the default context.
fn current_context(env: &ContextEnv) -> Result<Option<String>, ContextError> {
    let Some(dir) = env.config_dir() else {
        return Ok(None);
    };
    let config: Option<Config> = read_json(&dir.join("config.json"))?;
    Ok(config
        .and_then(|config| config.current_context)
        .filter(|name| !name.is_empty()))
}

/// Reads and parses a JSON file. `Ok(None)` means the file does not exist.
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, ContextError> {
    let unreadable = |reason: String| ContextError::Unreadable {
        path: path.to_path_buf(),
        reason,
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(unreadable(error.to_string())),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| unreadable(error.to_string()))
}

#[derive(Deserialize)]
struct Config {
    #[serde(rename = "currentContext")]
    current_context: Option<String>,
}

#[derive(Deserialize)]
struct Meta {
    #[serde(rename = "Endpoints")]
    endpoints: Endpoints,
}

#[derive(Deserialize)]
struct Endpoints {
    docker: Option<DockerEndpoint>,
}

#[derive(Deserialize)]
struct DockerEndpoint {
    #[serde(rename = "Host")]
    host: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn store_context(config_dir: &Path, name: &str, host: &str) {
        let dir = config_dir
            .join("contexts")
            .join("meta")
            .join(context_directory(name));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("meta.json"),
            format!(
                r#"{{"Name":"{name}","Metadata":{{}},"Endpoints":{{"docker":{{"Host":"{host}","SkipTLSVerify":false}}}}}}"#
            ),
        )
        .unwrap();
    }

    fn env_in(config_dir: &TempDir) -> ContextEnv {
        ContextEnv {
            docker_config: Some(config_dir.path().to_path_buf()),
            ..ContextEnv::default()
        }
    }

    /// The directory name Docker Desktop really uses for `desktop-linux`
    /// (checked against `~/.docker/contexts/meta` on a Mac).
    #[test]
    fn context_directories_are_the_sha256_of_the_name() {
        assert_eq!(
            context_directory("desktop-linux"),
            "fe9c6bd7a66301f49ca9b6a70b217107cd1284598bfc254700c989b916da791e"
        );
    }

    #[test]
    fn docker_host_wins_over_everything() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.json"), r#"{"currentContext":"x"}"#).unwrap();
        let env = ContextEnv {
            docker_host: Some("unix:///tmp/custom.sock".to_owned()),
            docker_context: Some("also-ignored".to_owned()),
            ..env_in(&dir)
        };
        let resolved = resolve(&env).unwrap();
        assert_eq!(resolved.endpoint, Endpoint::Unix("/tmp/custom.sock".into()));
    }

    #[test]
    fn without_any_configuration_the_default_socket_is_used() {
        let dir = TempDir::new().unwrap();
        let resolved = resolve(&env_in(&dir)).unwrap();
        assert_eq!(resolved.name, "default");
        assert_eq!(resolved.endpoint, Endpoint::Unix(DEFAULT_SOCKET.into()));
    }

    #[test]
    fn a_config_without_current_context_means_default() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("config.json"),
            r#"{"credsStore":"desktop"}"#,
        )
        .unwrap();
        assert_eq!(resolve(&env_in(&dir)).unwrap().name, "default");
    }

    #[test]
    fn the_current_context_of_config_json_is_followed() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("config.json"),
            r#"{"currentContext":"orbstack"}"#,
        )
        .unwrap();
        store_context(
            dir.path(),
            "orbstack",
            "unix:///Users/me/.orbstack/run/docker.sock",
        );

        let resolved = resolve(&env_in(&dir)).unwrap();

        assert_eq!(resolved.name, "orbstack");
        assert_eq!(
            resolved.endpoint,
            Endpoint::Unix("/Users/me/.orbstack/run/docker.sock".into())
        );
    }

    #[test]
    fn docker_context_overrides_the_config_file() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("config.json"),
            r#"{"currentContext":"orbstack"}"#,
        )
        .unwrap();
        store_context(dir.path(), "orbstack", "unix:///a.sock");
        store_context(dir.path(), "colima", "unix:///b.sock");
        let env = ContextEnv {
            docker_context: Some("colima".to_owned()),
            ..env_in(&dir)
        };
        assert_eq!(
            resolve(&env).unwrap().endpoint,
            Endpoint::Unix("/b.sock".into())
        );
    }

    #[test]
    fn an_explicit_default_context_needs_no_store_entry() {
        let dir = TempDir::new().unwrap();
        let env = ContextEnv {
            docker_context: Some("default".to_owned()),
            ..env_in(&dir)
        };
        assert_eq!(
            resolve(&env).unwrap().endpoint,
            Endpoint::Unix(DEFAULT_SOCKET.into())
        );
    }

    #[test]
    fn a_context_missing_from_the_store_is_a_clear_error() {
        let dir = TempDir::new().unwrap();
        let env = ContextEnv {
            docker_context: Some("ghost".to_owned()),
            ..env_in(&dir)
        };
        assert_eq!(
            resolve(&env),
            Err(ContextError::UnknownContext("ghost".to_owned()))
        );
    }

    #[test]
    fn a_corrupt_store_file_is_reported_not_guessed() {
        let dir = TempDir::new().unwrap();
        let meta = dir
            .path()
            .join("contexts/meta")
            .join(context_directory("broken"));
        fs::create_dir_all(&meta).unwrap();
        fs::write(meta.join("meta.json"), "{ not json").unwrap();
        let env = ContextEnv {
            docker_context: Some("broken".to_owned()),
            ..env_in(&dir)
        };
        assert!(matches!(
            resolve(&env),
            Err(ContextError::Unreadable { .. })
        ));
    }

    #[test]
    fn a_context_without_a_docker_endpoint_is_an_error() {
        let dir = TempDir::new().unwrap();
        let meta = dir
            .path()
            .join("contexts/meta")
            .join(context_directory("kube-only"));
        fs::create_dir_all(&meta).unwrap();
        fs::write(
            meta.join("meta.json"),
            r#"{"Name":"kube-only","Endpoints":{}}"#,
        )
        .unwrap();
        let env = ContextEnv {
            docker_context: Some("kube-only".to_owned()),
            ..env_in(&dir)
        };
        assert_eq!(
            resolve(&env),
            Err(ContextError::NoEndpoint("kube-only".to_owned()))
        );
    }

    #[test]
    fn non_unix_endpoints_are_kept_as_other() {
        assert_eq!(
            Endpoint::parse("tcp://192.168.64.2:2375"),
            Endpoint::Other("tcp://192.168.64.2:2375".to_owned())
        );
        assert_eq!(
            Endpoint::parse("unix://"),
            Endpoint::Other("unix://".to_owned())
        );
    }

    #[test]
    fn without_home_or_config_only_the_default_context_works() {
        let env = ContextEnv::default();
        assert_eq!(resolve(&env).unwrap().name, "default");
        let named = ContextEnv {
            docker_context: Some("desktop-linux".to_owned()),
            ..ContextEnv::default()
        };
        assert_eq!(resolve(&named), Err(ContextError::NoConfigDir));
    }
}
