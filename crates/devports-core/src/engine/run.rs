//! `devports run`: deciding who may start what (`CLI.md`, section 3).
//!
//! The decision is deterministic and never interactive (ADR-008): the same
//! situation always gives the same answer, and a refusal always says what exists
//! and what to do instead.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use devports_proto::run::{
    Accepted, RejectKind, Rejected, Reused, RunOutcome, RunRegister, RunRole,
};
use devports_proto::views::{ChainLink, InstanceRole, InstanceState, InstanceView};
use serde_json::json;

use super::{Effect, Engine};
use crate::clock::Clock;
use crate::ids::{new_reference, token_hash, token_text};
use crate::manifest::ServiceKind;
use crate::messages;
use crate::ports::{PortProbe, pick_in_range};
use crate::process::ProcessSource;
use crate::store::{InstanceRecord, Role, State};

/// The service a request resolved to, with the facts the policy needs.
struct Resolved {
    project: Option<String>,
    service: String,
    port: Option<u16>,
    kind: ServiceKind,
    start: Option<String>,
    /// Absolute working directory of a host service.
    cwd: Option<PathBuf>,
    docker: Option<String>,
}

/// How the command to run was given.
enum Command {
    /// The arguments after `--`.
    Args(Vec<String>),
    /// The `start` of the manifest, as one shell command.
    Start(String),
}

impl<C: Clock, S: ProcessSource, P: PortProbe> Engine<C, S, P> {
    /// Decides a `run` request and, if it is accepted, reserves the port.
    pub fn run_register(&mut self, request: &RunRegister) -> (RunOutcome, Vec<Effect>) {
        let now = self.now();
        let launcher = self.launcher_label(&request.launcher.chain);

        let resolved = match self.resolve(request) {
            Ok(resolved) => resolved,
            Err(message) => {
                return self.refuse(
                    RejectKind::NotFound,
                    request.project.as_deref(),
                    &request.service,
                    None,
                    &message,
                    &launcher,
                );
            }
        };
        let project = resolved.project.as_deref();

        if request.role == RunRole::Test {
            if request
                .reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
            {
                return self.refuse(
                    RejectKind::BadArgs,
                    project,
                    &resolved.service,
                    None,
                    "`--as test` exige `--reason \"<motivo>\"`",
                    &launcher,
                );
            }
            if let Some(ttl) = request.ttl_seconds
                && (ttl == 0 || Duration::from_secs(ttl) > self.config.lease.max_total)
            {
                let message = format!(
                    "`--ttl` debe estar entre 1 s y {} (máximo acumulado por instancia)",
                    messages::span(self.config.lease.max_total)
                );
                return self.refuse(
                    RejectKind::BadArgs,
                    project,
                    &resolved.service,
                    None,
                    &message,
                    &launcher,
                );
            }
        }

        let command = match (&request.command, &resolved.start) {
            (Some(args), _) if !args.is_empty() => Command::Args(args.clone()),
            (_, Some(start)) if !start.trim().is_empty() => Command::Start(start.clone()),
            _ => {
                return self.refuse(
                    RejectKind::NoCommand,
                    project,
                    &resolved.service,
                    None,
                    "",
                    &launcher,
                );
            }
        };

        let existing = self
            .views
            .iter()
            .find(|view| {
                view.role == InstanceRole::Principal
                    && view.project.as_deref() == project
                    && view.service.as_deref() == Some(resolved.service.as_str())
                    && (matches!(
                        view.state,
                        InstanceState::Healthy | InstanceState::Unhealthy | InstanceState::Starting
                    ) || (view.state == InstanceState::Unknown
                        && view.target_identity.is_some()))
            })
            .cloned();

        match (request.role, existing) {
            (RunRole::Test, _) => self.accept_temporal(request, &resolved, command, &launcher, now),
            (RunRole::Principal, None) => {
                self.accept_principal(request, &resolved, command, &launcher, now)
            }
            (RunRole::Principal, Some(existing)) if existing.state == InstanceState::Unhealthy => {
                self.conflict(RejectKind::Unhealthy, &resolved, existing, &launcher, now)
            }
            (RunRole::Principal, Some(existing)) if request.reuse => {
                let message = messages::reuse(&existing);
                self.event(
                    "reuse",
                    project,
                    Some(&resolved.service),
                    Some(&existing.instance_ref),
                    &json!({"launcher": launcher}),
                );
                (
                    RunOutcome::Reused(Reused {
                        instance: existing,
                        message,
                    }),
                    Vec::new(),
                )
            }
            (RunRole::Principal, Some(existing)) => {
                self.conflict(RejectKind::Conflict, &resolved, existing, &launcher, now)
            }
        }
    }

    fn resolve(&self, request: &RunRegister) -> Result<Resolved, String> {
        let cwd = Path::new(&request.cwd);
        let project = match &request.project {
            Some(name) => Some(
                self.catalog
                    .project_named(name)
                    .ok_or_else(|| format!("no existe el proyecto «{name}»; revisa `.devports.yml` o la carpeta raíz de configuración"))?,
            ),
            None => self.catalog.project_containing(cwd),
        };
        if let Some(project) = project
            && let Some(service) = project
                .services
                .iter()
                .find(|service| service.name == request.service)
        {
            return Ok(Resolved {
                project: Some(project.name.clone()),
                service: service.name.clone(),
                port: service.port,
                kind: service.kind,
                start: service.start.clone(),
                cwd: service.cwd.clone(),
                docker: service.docker.clone(),
            });
        }
        let cited = project.is_none_or(|project| project.uses.contains(&request.service));
        if cited && let Some(spec) = self.shared.services.get(&request.service) {
            return Ok(Resolved {
                project: None,
                service: request.service.clone(),
                port: spec.port,
                kind: spec.kind,
                start: spec.start.clone(),
                cwd: spec
                    .cwd
                    .as_deref()
                    .map(|dir| crate::discovery::expand_tilde(dir, &self.home_dir())),
                docker: spec.docker.clone(),
            });
        }
        Err(match project {
            Some(project) => format!(
                "el proyecto «{}» no tiene un servicio «{}» (ni lo cita en `uses`)",
                project.name, request.service
            ),
            None => format!(
                "no se encontró un `.devports.yml` desde «{}»; ejecuta desde la carpeta de un proyecto o usa `--project`",
                request.cwd
            ),
        })
    }

    pub(crate) fn home_dir(&self) -> PathBuf {
        self.config
            .roots
            .first()
            .and_then(|root| root.parent())
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    fn launcher_label(&self, chain: &[ChainLink]) -> String {
        let mut names: Vec<String> = chain
            .iter()
            .take(4)
            .map(|link| link.label.clone().unwrap_or_else(|| link.name.clone()))
            .collect();
        names.reverse();
        names.join(" > ")
    }

    fn refuse(
        &mut self,
        kind: RejectKind,
        project: Option<&str>,
        service: &str,
        existing: Option<(&InstanceView, Option<i64>)>,
        detail: &str,
        launcher: &str,
    ) -> (RunOutcome, Vec<Effect>) {
        let alternatives = if matches!(kind, RejectKind::Conflict | RejectKind::Unhealthy) {
            messages::conflict_alternatives(service)
        } else {
            Vec::new()
        };
        let message = messages::rejection(
            kind,
            project,
            service,
            existing,
            &alternatives,
            detail,
            self.now(),
        );
        self.event(
            "run_rejected",
            project,
            Some(service),
            existing.map(|(view, _)| view.instance_ref.as_str()),
            &json!({
                "code": kind.exit_code(),
                "error": kind,
                "launcher": launcher,
                "existing_url": existing.and_then(|(view, _)| view.url.clone()),
            }),
        );
        let outcome = RunOutcome::Rejected(Rejected {
            error: kind,
            message,
            project: project.map(str::to_owned),
            service: Some(service.to_owned()),
            existing: existing.map(|(view, _)| view.clone()),
            alternatives,
        });
        (outcome, Vec::new())
    }

    fn conflict(
        &mut self,
        kind: RejectKind,
        resolved: &Resolved,
        existing: InstanceView,
        launcher: &str,
        _now: i64,
    ) -> (RunOutcome, Vec<Effect>) {
        let started = self
            .tracked
            .iter()
            .find(|record| record.reference == existing.instance_ref)
            .and_then(|record| record.started_at)
            .or_else(|| self.started_of(&existing));
        self.refuse(
            kind,
            resolved.project.as_deref(),
            &resolved.service,
            Some((&existing, started)),
            "",
            launcher,
        )
    }

    /// When a view's instance started, in seconds, from its own timestamp.
    fn started_of(&self, view: &InstanceView) -> Option<i64> {
        let text = view.started_at.as_deref()?;
        text.parse::<jiff::Timestamp>()
            .ok()
            .map(jiff::Timestamp::as_second)
    }

    fn accept_principal(
        &mut self,
        request: &RunRegister,
        resolved: &Resolved,
        command: Command,
        launcher: &str,
        now: i64,
    ) -> (RunOutcome, Vec<Effect>) {
        let reserved = self.reserved_ports();
        let port = match resolved.port {
            Some(preferred) => {
                if reserved.contains(&preferred) || !self.ports.is_free(preferred) {
                    let who = self.who_holds(preferred);
                    return self.refuse(
                        RejectKind::PortBusy,
                        resolved.project.as_deref(),
                        &resolved.service,
                        None,
                        &format!("El puerto {preferred} está en uso por {who}; no es una instancia de este servicio."),
                        launcher,
                    );
                }
                preferred
            }
            None => match self.free_port(&reserved) {
                Some(port) => port,
                None => return self.no_port(resolved, launcher),
            },
        };
        self.accept(
            request,
            resolved,
            command,
            Role::Principal,
            port,
            None,
            launcher,
            now,
        )
    }

    fn accept_temporal(
        &mut self,
        request: &RunRegister,
        resolved: &Resolved,
        command: Command,
        launcher: &str,
        now: i64,
    ) -> (RunOutcome, Vec<Effect>) {
        let running = self
            .tracked
            .iter()
            .filter(|record| {
                record.role == Role::Temporal
                    && record.project == resolved.project
                    && record.service.as_deref() == Some(resolved.service.as_str())
            })
            .count();
        let max = self.config.lease.max_per_service;
        if running >= max {
            let detail = format!(
                "Ya hay {running} pruebas de este servicio (máximo {max}). Detén una con `devports stop <token>` o espera a que venza."
            );
            return self.refuse(
                RejectKind::Limit,
                resolved.project.as_deref(),
                &resolved.service,
                None,
                &detail,
                launcher,
            );
        }
        let ttl = request
            .ttl_seconds
            .map_or(self.config.lease.default, Duration::from_secs);
        let reserved = self.reserved_ports();
        let Some(port) = self.free_port(&reserved) else {
            return self.no_port(resolved, launcher);
        };
        self.accept(
            request,
            resolved,
            command,
            Role::Temporal,
            port,
            Some(ttl),
            launcher,
            now,
        )
    }

    fn no_port(&mut self, resolved: &Resolved, launcher: &str) -> (RunOutcome, Vec<Effect>) {
        let (low, high) = self.config.observation.temporal_port_range;
        self.refuse(
            RejectKind::NoPort,
            resolved.project.as_deref(),
            &resolved.service,
            None,
            &format!("No queda ningún puerto libre entre {low} y {high}."),
            launcher,
        )
    }

    fn reserved_ports(&self) -> BTreeSet<u16> {
        self.tracked
            .iter()
            .filter(|record| record.state.is_live())
            .filter_map(|record| record.port)
            .collect()
    }

    fn free_port(&mut self, reserved: &BTreeSet<u16>) -> Option<u16> {
        let entropy = (self.entropy)();
        let seed = u64::from_le_bytes([
            entropy[0], entropy[1], entropy[2], entropy[3], entropy[4], entropy[5], entropy[6],
            entropy[7],
        ]);
        pick_in_range(
            &self.ports,
            self.config.observation.temporal_port_range,
            reserved,
            seed,
        )
    }

    /// Names whatever listens on `port`, for the refusal message.
    fn who_holds(&self, port: u16) -> String {
        self.host
            .iter()
            .find(|process| process.ports.contains(&port))
            .map_or_else(
                || "otro programa".to_owned(),
                |process| {
                    let name = process
                        .info
                        .argv
                        .first()
                        .and_then(|program| program.rsplit('/').next())
                        .unwrap_or("un proceso");
                    format!("{name} (pid {})", process.info.id.pid)
                },
            )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one accepted request has this many facts"
    )]
    fn accept(
        &mut self,
        request: &RunRegister,
        resolved: &Resolved,
        command: Command,
        role: Role,
        port: u16,
        ttl: Option<Duration>,
        launcher: &str,
        now: i64,
    ) -> (RunOutcome, Vec<Effect>) {
        let taken: BTreeSet<String> = self
            .tracked
            .iter()
            .map(|record| record.reference.clone())
            .chain(self.views.iter().map(|view| view.instance_ref.clone()))
            .collect();
        let reference = new_reference(&(self.entropy)(), &taken);
        let token = (role == Role::Temporal).then(|| token_text(&(self.entropy)()));

        let ttl_seconds = ttl.map_or(0, |ttl| ttl.as_secs());
        let mut record = InstanceRecord::new(reference.clone(), role, State::Reserved);
        record.project.clone_from(&resolved.project);
        record.service = Some(resolved.service.clone());
        record.registered = true;
        record.port = Some(port);
        record.cwd = Some(match (&command, &resolved.cwd) {
            (Command::Start(_), Some(dir)) => dir.display().to_string(),
            _ => request.cwd.clone(),
        });
        record.launcher_pid = Some(request.launcher.pid);
        record.launcher_start = self
            .source
            .process_info(request.launcher.pid)
            .map(|info| info.id.start.0);
        record.launcher_chain = serde_json::to_string(&self.labelled(&request.launcher.chain))
            .unwrap_or_else(|_| "[]".to_owned());
        record.reason = request.reason.clone();
        record.token_hash = token.as_deref().map(token_hash);
        record.reserved_at = Some(now);
        record.expires_at =
            ttl.map(|_| now.saturating_add(i64::try_from(ttl_seconds).unwrap_or(i64::MAX)));
        record.leased_total_s = ttl_seconds;

        let stored = match self.store.insert_instance(&record) {
            Ok(stored) => stored,
            Err(error) => {
                return self.refuse(
                    RejectKind::NoPort,
                    resolved.project.as_deref(),
                    &resolved.service,
                    None,
                    &format!("No se pudo guardar el registro: {error}"),
                    launcher,
                );
            }
        };
        self.tracked.push(stored.clone());
        self.rebuild_views();

        let url = format!("http://localhost:{port}");
        let shell = build_shell(&command, port);
        let run_dir = stored.cwd.clone().unwrap_or_else(|| request.cwd.clone());
        let role_view = if role == Role::Temporal {
            InstanceRole::Temporal
        } else {
            InstanceRole::Principal
        };
        let project_name = resolved.project.as_deref().unwrap_or("compartidos");
        let banner = messages::banner(
            &resolved.service,
            project_name,
            role_view,
            &url,
            stored.expires_at,
            ttl,
            token.as_deref(),
        );
        let mut env: BTreeMap<String, String> = BTreeMap::new();
        env.insert("DEVPORTS_PROJECT".to_owned(), project_name.to_owned());
        env.insert("DEVPORTS_SERVICE".to_owned(), resolved.service.clone());
        env.insert(
            "DEVPORTS_ROLE".to_owned(),
            if role == Role::Temporal {
                "temporal"
            } else {
                "principal"
            }
            .to_owned(),
        );
        env.insert("DEVPORTS_PORT".to_owned(), port.to_string());
        env.insert("PORT".to_owned(), port.to_string());
        env.insert("DEVPORTS_INSTANCE_REF".to_owned(), reference.clone());
        if let Some(token) = &token {
            env.insert("DEVPORTS_STOP_TOKEN".to_owned(), token.clone());
        }
        if let Some(end) = stored.expires_at {
            env.insert("DEVPORTS_EXPIRES_AT".to_owned(), messages::iso(end));
        }
        self.event(
            "run_accepted",
            resolved.project.as_deref(),
            Some(&resolved.service),
            Some(&reference),
            &json!({
                "role": if role == Role::Temporal { "temporal" } else { "principal" },
                "port": port,
                "reason": request.reason,
                "launcher": launcher,
                "docker": resolved.docker,
                "kind": resolved.kind.as_str(),
            }),
        );
        let outcome = RunOutcome::Accepted(Accepted {
            instance_ref: reference,
            project: project_name.to_owned(),
            service: resolved.service.clone(),
            role: role_view,
            port,
            url,
            expires_at: stored.expires_at.map(messages::iso),
            token,
            run_dir,
            shell,
            env,
            banner,
        });
        (outcome, Vec::new())
    }

    /// The launcher chain with friendly labels for the agents it recognises.
    fn labelled(&self, chain: &[ChainLink]) -> Vec<ChainLink> {
        chain
            .iter()
            .map(|link| {
                let known = self
                    .config
                    .known_agents
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(&link.name))
                    .map(|(_, label)| label.clone());
                ChainLink {
                    pid: link.pid,
                    name: link.name.clone(),
                    label: link.label.clone().or(known),
                }
            })
            .collect()
    }
}

/// The complete shell command, always starting with `exec` so the final process
/// replaces the shell and keeps the launcher's pid.
fn build_shell(command: &Command, port: u16) -> String {
    let port_text = port.to_string();
    match command {
        Command::Start(start) => format!("exec {}", start.trim().replace("${PORT}", &port_text)),
        Command::Args(args) => {
            let quoted: Vec<String> = args
                .iter()
                .map(|arg| shell_quote(&arg.replace("{PORT}", &port_text)))
                .collect();
            format!("exec {}", quoted.join(" "))
        }
    }
}

pub(crate) use crate::shell::shell_quote;
