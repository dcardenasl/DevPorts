//! Read-only views of instances and activity, as the CLI and the interface show
//! them. They never contain a stop token (`CLI.md`, section 4).

use serde::{Deserialize, Serialize};

/// What kind of instance this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceRole {
    /// The official instance of a service (ADR-005). It may be unregistered
    /// ("de hecho") when it was started by hand.
    Principal,
    /// A test instance with a lease.
    Temporal,
    /// A listener that matches a service but was never registered and is not
    /// the Principal: a duplicate.
    Unregistered,
}

/// How an instance is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    /// Alive and answering its health check.
    Healthy,
    /// Alive but not answering: hung.
    Unhealthy,
    /// Declared, but nothing is running.
    Down,
    /// Port reserved, waiting for the process to open it.
    Starting,
    /// Being stopped.
    Stopping,
    /// Its lease ended but it would not die ("Olvidados").
    Forgotten,
}

/// One process in the chain that started an instance, nearest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLink {
    /// Process id.
    pub pid: u32,
    /// The process name as the system reports it.
    pub name: String,
    /// A friendly label from the `known_agents` table, if the name matches one.
    pub label: Option<String>,
}

/// An instance, for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceView {
    /// Short public reference.
    #[serde(rename = "ref")]
    pub instance_ref: String,
    /// Project name, absent when the process belongs to none.
    pub project: Option<String>,
    /// Service name, absent when it matches no service.
    pub service: Option<String>,
    /// Principal, Temporal or unregistered.
    pub role: InstanceRole,
    /// Health and lifecycle.
    pub state: InstanceState,
    /// `app` or `data`.
    pub kind: String,
    /// Port it listens on.
    pub port: Option<u16>,
    /// `http://localhost:<port>`, when it has a port.
    pub url: Option<String>,
    /// Process id, for a host process.
    pub pid: Option<u32>,
    /// Container name, for a container.
    pub container: Option<String>,
    /// Git branch of its working directory when it started.
    pub branch: Option<String>,
    /// Git worktree folder.
    pub worktree: Option<String>,
    /// Working directory.
    pub cwd: Option<String>,
    /// When it started, ISO 8601 with offset.
    pub started_at: Option<String>,
    /// When its lease ends (Temporals only).
    pub expires_at: Option<String>,
    /// Why a Temporal was started.
    pub reason: Option<String>,
    /// Whether it went through `devports run`.
    pub registered: bool,
    /// Who started it, nearest first. Informative only (ADR-004).
    pub launcher_chain: Vec<ChainLink>,
    /// The reference of the Principal this duplicates, if it is a duplicate.
    pub duplicate_of: Option<String>,
    /// A warning that it may be running old code ("arrancó antes del último commit").
    pub stale: Option<String>,
    /// Whether it is a shared service (`shared.yml`) rather than one of a project.
    #[serde(default)]
    pub shared: bool,
    /// The `docker compose` project it belongs to, for a compose container that
    /// no DevPorts project claims.
    #[serde(default)]
    pub compose: Option<String>,
    /// The command line of a host process, `argv[0]` first.
    #[serde(default)]
    pub argv: Vec<String>,
    /// The image of a container.
    #[serde(default)]
    pub image: Option<String>,
    /// The person chose to keep this unregistered duplicate: it no longer asks for attention.
    #[serde(default)]
    pub ignored: bool,
}

/// A project as the interface lays it out: repos with services inside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    /// Project name.
    pub name: String,
    /// Absolute folder of its manifest.
    pub root: String,
    /// Shared services it depends on.
    pub uses: Vec<String>,
    /// Repositories (or plain folders) its services live in.
    pub repos: Vec<RepoSummary>,
    /// Its declared services.
    pub services: Vec<ServiceSummary>,
}

/// A repository inside a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSummary {
    /// Folder name.
    pub name: String,
    /// Absolute path.
    pub path: String,
    /// Current branch, when it is a repository.
    pub branch: Option<String>,
}

/// A declared service and where it lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSummary {
    /// Service name.
    pub name: String,
    /// Absolute path of its repository (or the project folder).
    pub repo: String,
    /// Preferred port.
    pub port: Option<u16>,
    /// `app` or `data`.
    pub kind: String,
}

/// Why something is in the "Atención" tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    /// An unregistered duplicate of a Principal.
    Duplicate,
    /// A Temporal that is about to expire.
    Expiring,
    /// A service that stopped answering.
    Hung,
}

/// One thing that waits for a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionItem {
    /// What kind of thing it is.
    pub kind: AttentionKind,
    /// The instance's reference.
    pub instance_ref: String,
}

/// One line of the activity log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// When it happened, ISO 8601 with offset.
    pub at: String,
    /// The event kind, in `snake_case` (`run_rejected`, `lease_expired`, ...).
    pub kind: String,
    /// Project it concerns.
    pub project: Option<String>,
    /// Service it concerns.
    pub service: Option<String>,
    /// A sentence for people, already in the user's language.
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> InstanceView {
        InstanceView {
            instance_ref: "a1b2".to_owned(),
            project: Some("tienda-demo".to_owned()),
            service: Some("api".to_owned()),
            role: InstanceRole::Principal,
            state: InstanceState::Healthy,
            kind: "app".to_owned(),
            port: Some(8080),
            url: Some("http://localhost:8080".to_owned()),
            pid: Some(4121),
            container: None,
            branch: Some("feature/x".to_owned()),
            worktree: None,
            cwd: Some("/work/tienda-api".to_owned()),
            started_at: Some("2026-10-16T12:00:00-03:00".to_owned()),
            expires_at: None,
            reason: None,
            registered: true,
            launcher_chain: vec![ChainLink {
                pid: 4000,
                name: "zsh".to_owned(),
                label: None,
            }],
            duplicate_of: None,
            stale: None,
            shared: false,
            compose: None,
            argv: Vec::new(),
            image: None,
            ignored: false,
        }
    }

    #[test]
    fn the_reference_field_is_called_ref_on_the_wire() {
        let value = serde_json::to_value(sample()).unwrap();
        assert_eq!(value["ref"], "a1b2");
        assert!(value.get("instance_ref").is_none());
        assert_eq!(value["role"], "principal");
        assert_eq!(value["state"], "healthy");
    }

    #[test]
    fn no_view_can_carry_a_token() {
        let text = serde_json::to_string(&sample()).unwrap().to_lowercase();
        assert!(!text.contains("token"));
    }

    #[test]
    fn a_view_round_trips() {
        let view = sample();
        let text = serde_json::to_string(&view).unwrap();
        assert_eq!(serde_json::from_str::<InstanceView>(&text).unwrap(), view);
    }
}
