//! Every sentence the core writes for people, in one place.
//!
//! Text that a person reads (the CLI's refusals, the banner of an accepted run,
//! the lines of the activity log) is built here and nowhere else, so wording,
//! language and format stay consistent and are covered by golden tests. The
//! machine-readable fields travel next to it in the protocol types and never
//! depend on this text.

use std::time::Duration;

use devports_proto::run::{Alternative, AlternativeKind, RejectKind};
use devports_proto::views::{InstanceRole, InstanceState, InstanceView, LogFacts, LogTopic};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use serde_json::Value;

/// A calendar time as a local `HH:MM`.
#[must_use]
pub fn clock(timestamp: i64) -> String {
    zoned(timestamp).map_or_else(
        || "--:--".to_owned(),
        |time| time.strftime("%H:%M").to_string(),
    )
}

/// A calendar time as ISO 8601 with the local offset.
#[must_use]
pub fn iso(timestamp: i64) -> String {
    zoned(timestamp).map_or_else(
        || String::from("1970-01-01T00:00:00Z"),
        |time| time.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string(),
    )
}

fn zoned(timestamp: i64) -> Option<Zoned> {
    Timestamp::from_second(timestamp)
        .ok()
        .map(|moment| moment.to_zoned(TimeZone::system()))
}

/// A length of time as `45 s`, `12 min`, `2 h 10 min`.
#[must_use]
pub fn span(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        return format!("{seconds} s");
    }
    let minutes = seconds.checked_div(60).unwrap_or(0);
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let hours = minutes.checked_div(60).unwrap_or(0);
    let rest = minutes.saturating_sub(hours.saturating_mul(60));
    if rest == 0 {
        format!("{hours} h")
    } else {
        format!("{hours} h {rest} min")
    }
}

fn since(started_at: Option<i64>, now: i64) -> String {
    match started_at {
        Some(start) if now > start => {
            let elapsed = u64::try_from(now.saturating_sub(start)).unwrap_or(0);
            if elapsed < 60 {
                "desde hace menos de 1 min".to_owned()
            } else {
                format!("desde hace {}", span(Duration::from_secs(elapsed)))
            }
        }
        _ => "desde hace un momento".to_owned(),
    }
}

fn health_word(state: InstanceState) -> &'static str {
    match state {
        InstanceState::Healthy => "sana",
        InstanceState::Unhealthy => "no responde",
        InstanceState::Down => "detenida",
        InstanceState::Starting => "arrancando",
        InstanceState::Stopping => "deteniéndose",
        InstanceState::Forgotten => "olvidada",
        InstanceState::Unknown => "desconocida",
    }
}

fn describe_existing(existing: &InstanceView, now: i64, started_epoch: Option<i64>) -> Vec<String> {
    let mut place = Vec::new();
    if let Some(url) = &existing.url {
        place.push(url.clone());
    }
    if let Some(pid) = existing.pid {
        place.push(format!("pid {pid}"));
    }
    if let Some(container) = &existing.container {
        place.push(format!("contenedor {container}"));
    }
    if let Some(branch) = &existing.branch {
        place.push(format!("rama {branch}"));
    }
    if let Some(worktree) = &existing.worktree {
        place.push(format!("worktree {worktree}"));
    }
    vec![
        format!("  {}", place.join("   ")),
        format!(
            "  {}   {}",
            since(started_epoch, now),
            health_word(existing.state)
        ),
    ]
}

/// The complete refusal text of `devports run`.
#[must_use]
pub fn rejection(
    kind: RejectKind,
    project: Option<&str>,
    service: &str,
    existing: Option<(&InstanceView, Option<i64>)>,
    alternatives: &[Alternative],
    detail: &str,
    now: i64,
) -> String {
    let scope = project.map_or_else(
        || service.to_owned(),
        |project| format!("{service} (proyecto {project})"),
    );
    let mut lines = Vec::new();
    match kind {
        RejectKind::Conflict | RejectKind::Unhealthy => {
            let headline = if kind == RejectKind::Conflict {
                format!("devports: conflicto en {scope}")
            } else {
                format!("devports: la Principal de {scope} existe pero no responde")
            };
            lines.push(headline);
            lines.push(format!(
                "Ya existe la instancia Principal de \"{service}\":"
            ));
            if let Some((view, started)) = existing {
                lines.extend(describe_existing(view, now, started));
            }
            lines.push("No se inició nada. Alternativas:".to_owned());
            for alternative in alternatives {
                lines.push(alternative_line(alternative, service));
            }
        }
        RejectKind::Limit => {
            lines.push(format!(
                "devports: {scope} ya tiene el máximo de instancias de prueba"
            ));
            lines.push(detail.to_owned());
        }
        RejectKind::NoCommand => {
            lines.push(format!("devports: no hay comando para iniciar {scope}"));
            lines.push(
                "Añade `start:` al servicio en .devports.yml o pasa el comando después de `--`."
                    .to_owned(),
            );
        }
        RejectKind::NotFound => lines.push(format!("devports: {detail}")),
        RejectKind::BadArgs => lines.push(format!("devports: argumentos inválidos: {detail}")),
        RejectKind::PortBusy => {
            lines.push(format!(
                "devports: el puerto preferido de {scope} está ocupado"
            ));
            lines.push(detail.to_owned());
        }
        RejectKind::NoPort => {
            lines.push(format!("devports: no hay un puerto libre para {scope}"));
            lines.push(detail.to_owned());
        }
    }
    lines.join("\n")
}

fn alternative_line(alternative: &Alternative, service: &str) -> String {
    match alternative.kind {
        AlternativeKind::Reuse => format!(
            "  Reutilizarla:         {}",
            alternative
                .command
                .clone()
                .unwrap_or_else(|| format!("devports run {service} --reuse"))
        ),
        AlternativeKind::Test => format!(
            "  Levantar una prueba:  {}",
            alternative.command.clone().unwrap_or_else(|| format!(
                "devports run {service} --as test --reason \"<motivo>\" -- <comando con {{PORT}}>"
            ))
        ),
        AlternativeKind::StopInApp => {
            "  Reemplazarla:         detén la instancia desde la app de barra y vuelve a ejecutar"
                .to_owned()
        }
    }
}

/// The two ways out of a conflict that a CLI can take, plus the human one.
#[must_use]
pub fn conflict_alternatives(service: &str) -> Vec<Alternative> {
    vec![
        Alternative {
            kind: AlternativeKind::Reuse,
            command: Some(format!("devports run {service} --reuse")),
            needs_human: false,
        },
        Alternative {
            kind: AlternativeKind::Test,
            command: Some(format!(
                "devports run {service} --as test --reason \"<motivo>\" -- <comando con {{PORT}}>"
            )),
            needs_human: false,
        },
        Alternative {
            kind: AlternativeKind::StopInApp,
            command: None,
            needs_human: true,
        },
    ]
}

/// The stderr lines printed when a run is accepted.
#[must_use]
pub fn banner(
    service: &str,
    project: &str,
    role: InstanceRole,
    url: &str,
    expires_at: Option<i64>,
    lease: Option<Duration>,
    token: Option<&str>,
) -> Vec<String> {
    let mut lines = Vec::new();
    match (role, expires_at, lease) {
        (InstanceRole::Temporal, Some(end), Some(lease)) => lines.push(format!(
            "devports: {service} (proyecto {project}) como TEMPORAL en {url}, vence {} ({})",
            clock(end),
            span(lease)
        )),
        _ => lines.push(format!(
            "devports: {service} (proyecto {project}) como PRINCIPAL en {url}"
        )),
    }
    if let Some(token) = token {
        lines.push(format!(
            "DEVPORTS_STOP_TOKEN={token}   (guárdalo: permite detener o extender esta prueba)"
        ));
    }
    lines
}

/// The stdout line of `--reuse`.
#[must_use]
pub fn reuse(existing: &InstanceView) -> String {
    let who = match existing.role {
        InstanceRole::Temporal => "Temporal",
        _ => "Principal",
    };
    let service = existing.service.as_deref().unwrap_or("el servicio");
    let place = existing
        .url
        .clone()
        .or_else(|| {
            existing
                .container
                .clone()
                .map(|name| format!("contenedor {name}"))
        })
        .unwrap_or_default();
    if existing.state == InstanceState::Unknown {
        return format!(
            "devports: se conserva la referencia de {service} ({who}) en {place}; estado desconocido, sin observación actual. No se inició nada."
        );
    }
    match existing.pid {
        Some(pid) => format!("devports: {service} ya corre ({who}) en {place} (pid {pid})"),
        None => format!("devports: {service} ya corre ({who}) en {place}"),
    }
}

/// The themes an event belongs to, decided from its kind and details rather than from
/// the sentence that describes it.
#[must_use]
pub fn event_topics(kind: &str, payload: &Value) -> Vec<LogTopic> {
    let temporal_run =
        kind == "run_accepted" && payload.get("role").and_then(Value::as_str) == Some("temporal");
    let mut topics = Vec::new();
    if temporal_run
        || matches!(
            kind,
            "lease_warned" | "lease_extended" | "lease_expired" | "stop_failed"
        )
    {
        topics.push(LogTopic::Temporal);
    }
    if matches!(
        kind,
        "duplicate_detected" | "unregistered_seen" | "port_stolen" | "stop_failed" | "process_gone"
    ) {
        topics.push(LogTopic::Alert);
    }
    if kind == "run_rejected" {
        topics.push(LogTopic::Rejection);
    }
    topics
}

/// What an event's stored details say, read once: the sentence and the facts both come from
/// here, so they can never disagree about a key.
struct Details<'a> {
    port: Option<u16>,
    reason: Option<&'a str>,
    launcher: Option<&'a str>,
    existing_url: Option<&'a str>,
    expires_at: Option<i64>,
}

impl<'a> Details<'a> {
    fn read(payload: &'a Value) -> Self {
        let text = |key: &str| payload.get(key).and_then(Value::as_str);
        Self {
            port: payload
                .get("port")
                .and_then(Value::as_i64)
                .and_then(|port| u16::try_from(port).ok()),
            reason: text("reason"),
            launcher: text("launcher"),
            existing_url: text("existing_url"),
            expires_at: payload.get("expires_at").and_then(Value::as_i64),
        }
    }
}

/// What the interface needs to write an event's sentence itself, from its stored details.
#[must_use]
pub fn event_facts(payload: &Value) -> LogFacts {
    let details = Details::read(payload);
    LogFacts {
        port: details.port,
        reason: details.reason.map(str::to_owned),
        launcher: details.launcher.map(str::to_owned),
        existing_url: details.existing_url.map(str::to_owned),
        expires_at: details.expires_at.map(iso),
    }
}

/// One line of the activity log, in Spanish: what the CLI prints. The panel writes its own
/// from [`event_facts`] in the language in force.
#[must_use]
pub fn event_summary(kind: &str, service: Option<&str>, payload: &Value) -> String {
    let service = service.unwrap_or("un servicio");
    let details = Details::read(payload);
    let text = |key: &str| payload.get(key).and_then(Value::as_str);
    let port = details.port.map(|port| format!(":{port}"));
    let by = details.launcher.map_or_else(
        || "un proceso".to_owned(),
        |label| format!("un agente ({label})"),
    );
    match kind {
        "core_started" => "DevPorts se inició".to_owned(),
        "run_accepted" => match text("role") {
            Some("temporal") => format!(
                "Prueba de {service} {} para «{}»",
                port.unwrap_or_default(),
                details.reason.unwrap_or("sin motivo")
            ),
            _ => format!(
                "{service} registrada como Principal {}",
                port.unwrap_or_default()
            ),
        },
        "run_rejected" => format!(
            "{by} intentó levantar otra instancia de {service}; la Principal sigue {}",
            details
                .existing_url
                .map_or_else(String::new, |url| format!("en {url}"))
        ),
        "reuse" => format!("{by} reutilizó la Principal de {service}"),
        "reservation_released" => {
            format!("Se liberó el puerto reservado para {service}: nadie lo abrió a tiempo")
        }
        "bound" => format!(
            "{service} {} quedó vinculada a su proceso",
            port.unwrap_or_default()
        ),
        "port_stolen" => format!(
            "Otro proceso tomó el puerto {} reservado para {service}",
            details
                .port
                .map_or_else(|| "?".to_owned(), |port| port.to_string())
        ),
        "lease_warned" => format!(
            "La prueba de {service} {} vence en 1 minuto",
            port.unwrap_or_default()
        ),
        "lease_extended" => format!(
            "La prueba de {service} {} se extendió; vence {}",
            port.unwrap_or_default(),
            details
                .expires_at
                .map_or_else(|| "más tarde".to_owned(), clock)
        ),
        "lease_expired" => format!(
            "La prueba de {service} {} venció y se detuvo",
            port.unwrap_or_default()
        ),
        "stopped" => format!("{service} {} se detuvo", port.unwrap_or_default()),
        "stop_failed" => format!(
            "{service} {} no se pudo detener: queda como olvidada",
            port.unwrap_or_default()
        ),
        "duplicate_detected" => format!(
            "Apareció un duplicado de {service} {} sin registrar",
            port.unwrap_or_default()
        ),
        "kept" => format!(
            "Conservaste {service} {} aunque duplica a la Principal",
            port.unwrap_or_default()
        ),
        "restored" => "Restauraste un servicio ignorado".to_owned(),
        "reclassified" => format!(
            "Cambiaste la clasificación de {service} {}",
            port.unwrap_or_default()
        ),
        "adopted" => format!("Adoptaste {service} en un manifiesto"),
        "unregistered_seen" => format!(
            "Apareció un proceso sin registrar {}",
            port.unwrap_or_default()
        ),
        "process_gone" => format!("{service} {} dejó de ejecutarse", port.unwrap_or_default()),
        other => other.replace('_', " "),
    }
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

#[cfg(test)]
mod tests;
