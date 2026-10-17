//! `devports run`: what the CLI asks and what the core answers.
//!
//! The core decides everything (project resolution, which command to run, which
//! port, whether to accept) so the CLI stays a thin client that depends on
//! nothing but this crate. The CLI's only decisions are mechanical: print the
//! message, set the environment and `exec`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::views::{ChainLink, InstanceRole, InstanceView};

/// Which kind of instance the caller asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunRole {
    /// The official instance (no `--as`).
    Principal,
    /// A test instance (`--as test`).
    Test,
}

/// The process that is asking, captured by the CLI while it is still alive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherInfo {
    /// The CLI's own pid. `exec` keeps it, so the server will be this process
    /// or one of its descendants.
    pub pid: u32,
    /// Its ancestors, nearest first.
    pub chain: Vec<ChainLink>,
}

/// Arguments of `run.register`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRegister {
    /// `--project`; resolved from `cwd` when absent.
    pub project: Option<String>,
    /// The service to run.
    pub service: String,
    /// The folder the CLI was run from.
    pub cwd: String,
    /// Principal or test.
    pub role: RunRole,
    /// Why, for a test instance (`--reason`).
    pub reason: Option<String>,
    /// `--ttl`, in seconds.
    pub ttl_seconds: Option<u64>,
    /// `--reuse`: do not start anything if a healthy Principal exists.
    pub reuse: bool,
    /// The command given after `--`, as separate arguments. `{PORT}` is
    /// replaced by the core with the assigned port.
    pub command: Option<Vec<String>>,
    /// Who is asking.
    pub launcher: LauncherInfo,
}

/// Why a request was refused. Each has a fixed exit code (`CLI.md`, section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectKind {
    /// A Principal already exists, possibly without current observation.
    Conflict,
    /// The Principal exists but is not answering.
    Unhealthy,
    /// The service already has the maximum number of Temporals.
    Limit,
    /// There is no command to run.
    NoCommand,
    /// The manifest or service was not found.
    NotFound,
    /// The arguments are invalid.
    BadArgs,
    /// The preferred port is held by a process that is not this service.
    PortBusy,
    /// No free port could be assigned.
    NoPort,
}

impl RejectKind {
    /// The process exit code the CLI uses for this refusal.
    #[must_use]
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Conflict => 10,
            Self::Unhealthy => 11,
            Self::Limit => 12,
            Self::NoCommand => 13,
            Self::NotFound => 14,
            Self::BadArgs => 16,
            Self::PortBusy => 18,
            Self::NoPort => 19,
        }
    }
}

/// A way out of a refusal, offered to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alternative {
    /// What it does.
    pub kind: AlternativeKind,
    /// The command to run, when there is one.
    pub command: Option<String>,
    /// Whether a person has to do it in the application.
    pub needs_human: bool,
}

/// The ways out of a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlternativeKind {
    /// Use the instance that exists (`--reuse`).
    Reuse,
    /// Start a test instance (`--as test --reason ...`).
    Test,
    /// Stop the Principal from the application, then run again (ADR-023).
    StopInApp,
}

/// A request that was accepted: the CLI prints the banner, sets the environment
/// and replaces itself with `sh -c <shell>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    /// Short reference of the new instance.
    pub instance_ref: String,
    /// Project it belongs to.
    pub project: String,
    /// Service it runs.
    pub service: String,
    /// Principal or Temporal.
    pub role: InstanceRole,
    /// The assigned port.
    pub port: u16,
    /// `http://localhost:<port>`.
    pub url: String,
    /// Lease end for a Temporal.
    pub expires_at: Option<String>,
    /// The stop token, shown once, for a Temporal.
    pub token: Option<String>,
    /// Folder to run in.
    pub run_dir: String,
    /// The complete shell command, starting with `exec`.
    pub shell: String,
    /// Environment variables to set (`DEVPORTS_*`, `PORT`).
    pub env: BTreeMap<String, String>,
    /// Lines for stderr, already in the user's language.
    pub banner: Vec<String>,
}

/// `--reuse` found a healthy Principal: nothing is started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reused {
    /// The instance that already runs.
    pub instance: InstanceView,
    /// Line for stdout.
    pub message: String,
}

/// A request that was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    /// Why.
    pub error: RejectKind,
    /// Complete explanation for people, with the alternatives, in the user's language.
    pub message: String,
    /// Project involved, when known.
    pub project: Option<String>,
    /// Service involved, when known.
    pub service: Option<String>,
    /// The instance in the way.
    pub existing: Option<InstanceView>,
    /// Ways out.
    pub alternatives: Vec<Alternative>,
}

/// The answer to `run.register`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunOutcome {
    /// Start the command.
    Accepted(Accepted),
    /// Something healthy already runs; start nothing.
    Reused(Reused),
    /// Refused.
    Rejected(Rejected),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected(kind: RejectKind) -> RunOutcome {
        RunOutcome::Rejected(Rejected {
            error: kind,
            message: "conflicto".to_owned(),
            project: Some("tienda-demo".to_owned()),
            service: Some("api".to_owned()),
            existing: None,
            alternatives: vec![
                Alternative {
                    kind: AlternativeKind::Reuse,
                    command: Some("devports run api --reuse".to_owned()),
                    needs_human: false,
                },
                Alternative {
                    kind: AlternativeKind::StopInApp,
                    command: None,
                    needs_human: true,
                },
            ],
        })
    }

    #[test]
    fn exit_codes_are_the_documented_ones_and_fifteen_stays_unassigned() {
        let codes: Vec<u8> = [
            RejectKind::Conflict,
            RejectKind::Unhealthy,
            RejectKind::Limit,
            RejectKind::NoCommand,
            RejectKind::NotFound,
            RejectKind::BadArgs,
            RejectKind::PortBusy,
            RejectKind::NoPort,
        ]
        .into_iter()
        .map(RejectKind::exit_code)
        .collect();
        assert_eq!(codes, [10, 11, 12, 13, 14, 16, 18, 19]);
        assert!(!codes.contains(&15), "ADR-023 retired code 15");
    }

    #[test]
    fn an_outcome_names_its_status_on_the_wire() {
        let value = serde_json::to_value(rejected(RejectKind::Conflict)).unwrap();
        assert_eq!(value["status"], "rejected");
        assert_eq!(value["error"], "conflict");
        assert_eq!(value["alternatives"][1]["kind"], "stop_in_app");
        assert_eq!(value["alternatives"][1]["needs_human"], true);
    }

    #[test]
    fn every_outcome_round_trips() {
        let accepted = RunOutcome::Accepted(Accepted {
            instance_ref: "c3d4".to_owned(),
            project: "tienda-demo".to_owned(),
            service: "api".to_owned(),
            role: InstanceRole::Temporal,
            port: 51234,
            url: "http://localhost:51234".to_owned(),
            expires_at: Some("2026-10-16T14:50:00-03:00".to_owned()),
            token: Some("9f1c".to_owned()),
            run_dir: "/work/tienda-api".to_owned(),
            shell: "exec php spark serve --port 51234".to_owned(),
            env: BTreeMap::from([("PORT".to_owned(), "51234".to_owned())]),
            banner: vec!["devports: api como TEMPORAL".to_owned()],
        });
        for outcome in [accepted, rejected(RejectKind::Limit)] {
            let text = serde_json::to_string(&outcome).unwrap();
            assert_eq!(serde_json::from_str::<RunOutcome>(&text).unwrap(), outcome);
        }
    }

    #[test]
    fn a_register_request_round_trips() {
        let request = RunRegister {
            project: None,
            service: "api".to_owned(),
            cwd: "/work/tienda-api".to_owned(),
            role: RunRole::Test,
            reason: Some("QA login".to_owned()),
            ttl_seconds: Some(1800),
            reuse: false,
            command: Some(vec![
                "php".to_owned(),
                "spark".to_owned(),
                "{PORT}".to_owned(),
            ]),
            launcher: LauncherInfo {
                pid: 4000,
                chain: Vec::new(),
            },
        };
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<RunRegister>(&text).unwrap(), request);
    }
}
