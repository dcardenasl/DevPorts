//! `config.yml` and `shared.yml` (`MANIFIESTO.md`, sections 2 and 6).
//!
//! Both live in `~/.config/devports/` and both are optional: a missing file means
//! the defaults. Like manifests, they are data: reading them never executes
//! anything, and problems come back as line-numbered [`Diagnostic`]s.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use devports_proto::duration::parse_duration;
use serde::Deserialize;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::discovery::expand_tilde;
use crate::manifest::{RawService, ServiceFile, ServiceSpec, unknown_keys, validate_services};
use crate::yaml_keys;

/// The folder of the user-level files, under `home`.
#[must_use]
pub fn config_dir(home: &Path) -> PathBuf {
    home.join(".config").join("devports")
}

/// Why a configuration file could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// The file exists but could not be read.
    #[error("{}: no se pudo leer: {reason}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The operating system's explanation.
        reason: String,
    },
    /// The file has errors.
    #[error("{}: {}", path.display(), first_error(diagnostics))]
    Invalid {
        /// The file.
        path: PathBuf,
        /// Every problem found.
        diagnostics: Vec<Diagnostic>,
    },
}

fn first_error(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == crate::diagnostic::Severity::Error)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// How long temporary instances live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseConfig {
    /// Initial lifetime of a Temporal.
    pub default: Duration,
    /// Most a Temporal may be leased in total.
    pub max_total: Duration,
    /// Default amount added by `extend`.
    pub extend_by: Duration,
    /// How long before the end the warning is raised.
    pub warn_before: Duration,
    /// Temporals allowed at once per service.
    pub max_per_service: usize,
}

/// How the host is watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationConfig {
    /// Time between scans of the host's listeners.
    pub host_interval: Duration,
    /// Range Temporals draw their ports from, both ends included.
    pub temporal_port_range: (u16, u16),
    /// How long a reserved port waits for its process before it is released.
    pub reservation_grace: Duration,
}

/// `config.yml` with every default filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Folders searched for projects.
    pub roots: Vec<PathBuf>,
    /// How many levels below a root are searched.
    pub max_depth: usize,
    /// Also search the folders `~/.ci4-ops/projects.yml` lists.
    pub import_ops_registry: bool,
    /// Lifetimes of Temporals.
    pub lease: LeaseConfig,
    /// One notification per project in this window.
    pub notification_dedupe_window: Duration,
    /// Watching the host.
    pub observation: ObservationConfig,
    /// Days the activity log is kept.
    pub events_retention_days: u32,
    /// Process-name to friendly-label table for the launcher chain.
    pub known_agents: BTreeMap<String, String>,
}

impl Config {
    /// The documented defaults.
    #[must_use]
    pub fn defaults(home: &Path) -> Self {
        Self {
            roots: crate::discovery::default_roots(home),
            max_depth: crate::discovery::DEFAULT_MAX_DEPTH,
            import_ops_registry: false,
            lease: LeaseConfig {
                default: Duration::from_mins(30),
                max_total: Duration::from_hours(4),
                extend_by: Duration::from_mins(30),
                warn_before: Duration::from_mins(1),
                max_per_service: 2,
            },
            notification_dedupe_window: Duration::from_mins(10),
            observation: ObservationConfig {
                host_interval: Duration::from_millis(1500),
                temporal_port_range: (49152, 65535),
                reservation_grace: Duration::from_mins(1),
            },
            events_retention_days: 30,
            known_agents: [
                ("claude", "Claude Code"),
                ("cursor", "Cursor"),
                ("codex", "Codex"),
                ("Code", "VS Code"),
            ]
            .into_iter()
            .map(|(name, label)| (name.to_owned(), label.to_owned()))
            .collect(),
        }
    }

    /// Reads `path`; a missing file gives the defaults. Returns the warnings too.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] if the file is unreadable or has errors.
    pub fn load(path: &Path, home: &Path) -> Result<(Self, Vec<Diagnostic>), ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path, home),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok((Self::defaults(home), Vec::new()))
            }
            Err(error) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }),
        }
    }

    /// Parses `config.yml` text. `path` only labels errors.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Invalid`] if anything is wrong.
    pub fn parse(
        text: &str,
        path: &Path,
        home: &Path,
    ) -> Result<(Self, Vec<Diagnostic>), ConfigError> {
        let invalid = |diagnostics| ConfigError::Invalid {
            path: path.to_path_buf(),
            diagnostics,
        };
        let raw: RawConfig = match serde_saphyr::from_str(text) {
            Ok(raw) => raw,
            Err(error) => {
                let line = error.location().map(|location| location.line());
                return Err(invalid(vec![Diagnostic::error(line, error.to_string())]));
            }
        };
        let keys = yaml_keys::index(text);
        let mut diagnostics = Vec::new();
        unknown_keys(&keys, &[], &CONFIG_KEYS, &mut diagnostics);
        unknown_keys(&keys, &["lease"], &LEASE_KEYS, &mut diagnostics);
        unknown_keys(
            &keys,
            &["notifications"],
            &["dedupe_window"],
            &mut diagnostics,
        );
        unknown_keys(&keys, &["observation"], &OBSERVATION_KEYS, &mut diagnostics);

        let mut config = Self::defaults(home);
        let mut reader = Reader {
            keys: &keys,
            diagnostics: &mut diagnostics,
        };
        if let Some(version) = raw.version
            && version != 1
        {
            reader.error(
                &["version"],
                format!("versión {version} no soportada; actualiza devports"),
            );
        }
        if let Some(roots) = raw.roots {
            config.roots = roots.iter().map(|root| expand_tilde(root, home)).collect();
        }
        if let Some(depth) = raw.max_depth {
            match usize::try_from(depth).ok().filter(|depth| *depth <= 8) {
                Some(depth) => config.max_depth = depth,
                None => reader.error(
                    &["max_depth"],
                    format!("«max_depth» debe estar entre 0 y 8, no {depth}"),
                ),
            }
        }
        if let Some(flag) = raw.import_ops_registry {
            config.import_ops_registry = flag;
        }
        if let Some(lease) = raw.lease {
            reader.lease(&lease, &mut config.lease);
        }
        if let Some(notifications) = raw.notifications
            && let Some(window) = notifications.dedupe_window
            && let Some(window) = reader.duration(&["notifications", "dedupe_window"], &window)
        {
            config.notification_dedupe_window = window;
        }
        if let Some(observation) = raw.observation {
            reader.observation(&observation, &mut config.observation);
        }
        if let Some(days) = raw.events_retention_days {
            match u32::try_from(days).ok().filter(|days| *days >= 1) {
                Some(days) => config.events_retention_days = days,
                None => reader.error(
                    &["events_retention_days"],
                    format!("«events_retention_days» debe ser al menos 1, no {days}"),
                ),
            }
        }
        if let Some(agents) = raw.known_agents {
            config.known_agents.extend(agents);
        }
        reader.cross_checks(&config);

        if has_errors(&diagnostics) {
            return Err(invalid(diagnostics));
        }
        Ok((config, diagnostics))
    }
}

const CONFIG_KEYS: [&str; 9] = [
    "version",
    "roots",
    "max_depth",
    "import_ops_registry",
    "lease",
    "notifications",
    "observation",
    "events_retention_days",
    "known_agents",
];
const LEASE_KEYS: [&str; 5] = [
    "default",
    "max_total",
    "extend_by",
    "warn_before",
    "max_per_service",
];
const OBSERVATION_KEYS: [&str; 3] = ["host_interval", "temporal_port_range", "reservation_grace"];

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    version: Option<i64>,
    roots: Option<Vec<String>>,
    max_depth: Option<i64>,
    import_ops_registry: Option<bool>,
    lease: Option<RawLease>,
    notifications: Option<RawNotifications>,
    observation: Option<RawObservation>,
    events_retention_days: Option<i64>,
    known_agents: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawLease {
    default: Option<String>,
    max_total: Option<String>,
    extend_by: Option<String>,
    warn_before: Option<String>,
    max_per_service: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct RawNotifications {
    dedupe_window: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawObservation {
    host_interval: Option<String>,
    temporal_port_range: Option<Vec<i64>>,
    reservation_grace: Option<String>,
}

struct Reader<'a> {
    keys: &'a yaml_keys::KeyIndex,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl Reader<'_> {
    fn error(&mut self, path: &[&str], message: String) {
        self.diagnostics
            .push(Diagnostic::error(self.keys.line(path), message));
    }

    fn duration(&mut self, path: &[&str], text: &str) -> Option<Duration> {
        match parse_duration(text) {
            Ok(duration) => Some(duration),
            Err(error) => {
                self.error(path, format!("«{}»: {error}", path.join(".")));
                None
            }
        }
    }

    fn lease(&mut self, raw: &RawLease, lease: &mut LeaseConfig) {
        for (key, text, target) in [
            ("default", &raw.default, &mut lease.default),
            ("max_total", &raw.max_total, &mut lease.max_total),
            ("extend_by", &raw.extend_by, &mut lease.extend_by),
            ("warn_before", &raw.warn_before, &mut lease.warn_before),
        ] {
            if let Some(text) = text
                && let Some(duration) = self.duration(&["lease", key], text)
            {
                *target = duration;
            }
        }
        if let Some(count) = raw.max_per_service {
            match usize::try_from(count)
                .ok()
                .filter(|count| (1..=16).contains(count))
            {
                Some(count) => lease.max_per_service = count,
                None => self.error(
                    &["lease", "max_per_service"],
                    format!("«lease.max_per_service» debe estar entre 1 y 16, no {count}"),
                ),
            }
        }
    }

    fn observation(&mut self, raw: &RawObservation, observation: &mut ObservationConfig) {
        if let Some(text) = &raw.host_interval
            && let Some(interval) = self.duration(&["observation", "host_interval"], text)
        {
            if (Duration::from_millis(250)..=Duration::from_secs(10)).contains(&interval) {
                observation.host_interval = interval;
            } else {
                self.error(
                    &["observation", "host_interval"],
                    "«observation.host_interval» debe estar entre 250ms y 10s".to_owned(),
                );
            }
        }
        if let Some(text) = &raw.reservation_grace
            && let Some(grace) = self.duration(&["observation", "reservation_grace"], text)
        {
            observation.reservation_grace = grace;
        }
        if let Some(range) = &raw.temporal_port_range {
            let ports: Option<Vec<u16>> =
                range.iter().map(|port| u16::try_from(*port).ok()).collect();
            match ports.as_deref() {
                Some([start, end]) if *start >= 1024 && start < end => {
                    observation.temporal_port_range = (*start, *end);
                }
                _ => self.error(
                    &["observation", "temporal_port_range"],
                    "«observation.temporal_port_range» debe ser [inicio, fin] con 1024 <= inicio < fin <= 65535".to_owned(),
                ),
            }
        }
    }

    /// Rules that relate two values, checked on the final configuration.
    fn cross_checks(&mut self, config: &Config) {
        let lease = config.lease;
        if lease.default > lease.max_total {
            self.error(
                &["lease", "default"],
                "«lease.default» no puede superar «lease.max_total»".to_owned(),
            );
        }
        if lease.extend_by > lease.max_total {
            self.error(
                &["lease", "extend_by"],
                "«lease.extend_by» no puede superar «lease.max_total»".to_owned(),
            );
        }
        if lease.warn_before >= lease.default {
            self.error(
                &["lease", "warn_before"],
                "«lease.warn_before» debe ser menor que «lease.default»".to_owned(),
            );
        }
    }
}

/// `shared.yml`: services used by several projects, such as the one MySQL.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SharedServices {
    /// The services, by the name projects cite in `uses`.
    pub services: BTreeMap<String, ServiceSpec>,
}

impl SharedServices {
    /// The names projects may cite.
    #[must_use]
    pub fn names(&self) -> BTreeSet<String> {
        self.services.keys().cloned().collect()
    }

    /// Reads `path`; a missing file means there are none.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] if the file is unreadable or has errors.
    pub fn load(path: &Path) -> Result<(Self, Vec<Diagnostic>), ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok((Self::default(), Vec::new()))
            }
            Err(error) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }),
        }
    }

    /// Parses `shared.yml` text. `path` only labels errors.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Invalid`] if anything is wrong.
    pub fn parse(text: &str, path: &Path) -> Result<(Self, Vec<Diagnostic>), ConfigError> {
        #[derive(Default, Deserialize)]
        struct Raw {
            version: Option<i64>,
            services: Option<BTreeMap<String, RawService>>,
        }
        let invalid = |diagnostics| ConfigError::Invalid {
            path: path.to_path_buf(),
            diagnostics,
        };
        let raw: Raw = match serde_saphyr::from_str(text) {
            Ok(raw) => raw,
            Err(error) => {
                let line = error.location().map(|location| location.line());
                return Err(invalid(vec![Diagnostic::error(line, error.to_string())]));
            }
        };
        let keys = yaml_keys::index(text);
        let mut diagnostics = Vec::new();
        unknown_keys(&keys, &[], &["version", "services"], &mut diagnostics);
        if raw.version != Some(1) {
            diagnostics.push(Diagnostic::error(
                keys.line(&["version"]),
                "«version» debe ser 1",
            ));
        }
        let services = validate_services(
            &raw.services.unwrap_or_default(),
            &keys,
            ServiceFile::Shared,
            &mut diagnostics,
        );
        if has_errors(&diagnostics) {
            return Err(invalid(diagnostics));
        }
        Ok((Self { services }, diagnostics))
    }
}

#[cfg(test)]
mod tests;
