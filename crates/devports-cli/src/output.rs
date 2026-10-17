//! What the CLI prints for people. `--json` prints the protocol types as they are.

use devports_proto::views::{InstanceRole, InstanceState, InstanceView, LogEntry};

/// `HH:MM` of an ISO 8601 timestamp that already carries the local offset.
#[must_use]
pub fn clock_time(iso: &str) -> String {
    iso.get(11..16).unwrap_or(iso).to_owned()
}

fn dash(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn state_word(state: InstanceState) -> &'static str {
    match state {
        InstanceState::Healthy => "sana",
        InstanceState::Unhealthy => "colgada",
        InstanceState::Down => "caída",
        InstanceState::Starting => "iniciando",
        InstanceState::Stopping => "deteniendo",
        InstanceState::Forgotten => "olvidada",
    }
}

fn role_word(role: InstanceRole) -> &'static str {
    match role {
        InstanceRole::Principal => "principal",
        InstanceRole::Temporal => "temporal",
        InstanceRole::Unregistered => "sin registrar",
    }
}

/// The table of `devports ls`.
#[must_use]
pub fn instances_table(instances: &[InstanceView]) -> String {
    if instances.is_empty() {
        return "No hay instancias.\n".to_owned();
    }
    let registered: Vec<&InstanceView> = instances
        .iter()
        .filter(|view| view.role != InstanceRole::Unregistered)
        .collect();
    let mut rows: Vec<[String; 9]> = vec![
        [
            "PROYECTO", "SERVICIO", "ROL", "ESTADO", "PUERTO", "PID", "RAMA", "VENCE", "REF",
        ]
        .map(str::to_owned),
    ];
    for view in &registered {
        rows.push([
            dash(view.project.as_deref()).to_owned(),
            dash(view.service.as_deref()).to_owned(),
            role_word(view.role).to_owned(),
            state_word(view.state).to_owned(),
            view.port
                .map_or_else(|| "-".to_owned(), |port| port.to_string()),
            view.pid
                .map_or_else(|| "-".to_owned(), |pid| pid.to_string()),
            dash(view.branch.as_deref()).to_owned(),
            view.expires_at
                .as_deref()
                .map_or_else(|| "-".to_owned(), clock_time),
            view.instance_ref.clone(),
        ]);
    }
    let mut widths = [0_usize; 9];
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut out = String::new();
    if registered.is_empty() {
        out.push_str("No hay instancias registradas.\n");
    } else {
        for row in &rows {
            let line: Vec<String> = row
                .iter()
                .zip(widths)
                .map(|(cell, width)| format!("{cell:<width$}"))
                .collect();
            out.push_str(line.join("  ").trim_end());
            out.push('\n');
        }
    }
    for view in instances
        .iter()
        .filter(|view| view.role == InstanceRole::Unregistered)
    {
        out.push_str(&unregistered_line(view));
        out.push('\n');
    }
    out
}

fn unregistered_line(view: &InstanceView) -> String {
    let place = match (&view.project, &view.service) {
        (Some(project), Some(service)) => format!("{project}/{service}"),
        (Some(project), None) => project.clone(),
        _ => view
            .container
            .clone()
            .or_else(|| view.argv.first().cloned())
            .unwrap_or_else(|| "(sin proyecto)".to_owned()),
    };
    let mut parts = vec![format!("(sin registrar) {place}")];
    if let Some(port) = view.port {
        parts.push(format!(":{port}"));
    }
    if let Some(pid) = view.pid {
        parts.push(format!("pid {pid}"));
    }
    if view.duplicate_of.is_some() {
        parts.push("duplica a la Principal".to_owned());
    }
    format!("{}   {}", parts.join("  "), view.instance_ref)
}

/// The lines of `devports log`.
#[must_use]
pub fn log_lines(entries: &[LogEntry]) -> String {
    if entries.is_empty() {
        return "No hay actividad en ese período.\n".to_owned();
    }
    let lines: Vec<String> = entries
        .iter()
        .map(|entry| {
            format!(
                "{}  {:<12} {}\n",
                clock_time(&entry.at),
                entry.service.as_deref().unwrap_or("-"),
                entry.summary
            )
        })
        .collect();
    lines.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(role: InstanceRole, state: InstanceState) -> InstanceView {
        InstanceView {
            instance_ref: "a1b2".to_owned(),
            project: Some("tienda-demo".to_owned()),
            service: Some("api".to_owned()),
            role,
            state,
            kind: "app".to_owned(),
            port: Some(8080),
            url: Some("http://localhost:8080".to_owned()),
            pid: Some(4121),
            container: None,
            branch: Some("feature/x".to_owned()),
            worktree: None,
            cwd: None,
            started_at: None,
            expires_at: Some("2026-10-17T14:50:00-03:00".to_owned()),
            reason: None,
            registered: role != InstanceRole::Unregistered,
            launcher_chain: Vec::new(),
            duplicate_of: (role == InstanceRole::Unregistered).then(|| "a1b2".to_owned()),
            stale: None,
            shared: false,
            compose: None,
            argv: Vec::new(),
            image: None,
        }
    }

    #[test]
    fn the_table_aligns_columns_and_lists_duplicates_apart() {
        let text = instances_table(&[
            view(InstanceRole::Principal, InstanceState::Healthy),
            view(InstanceRole::Unregistered, InstanceState::Healthy),
        ]);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("PROYECTO"));
        assert!(lines[1].starts_with("tienda-demo  api"));
        assert!(lines[1].contains("principal"));
        assert!(lines[1].contains("sana"));
        assert!(lines[1].contains("14:50"));
        assert!(lines[2].starts_with(
            "(sin registrar) tienda-demo/api  :8080  pid 4121  duplica a la Principal"
        ));
    }

    #[test]
    fn an_empty_list_says_so() {
        assert_eq!(instances_table(&[]), "No hay instancias.\n");
    }

    #[test]
    fn times_are_cut_from_the_iso_text() {
        assert_eq!(clock_time("2026-10-17T14:50:03-03:00"), "14:50");
        assert_eq!(clock_time("short"), "short");
    }

    #[test]
    fn the_log_prints_time_service_and_summary() {
        let text = log_lines(&[LogEntry {
            at: "2026-10-17T14:02:00-03:00".to_owned(),
            kind: "run_rejected".to_owned(),
            project: Some("tienda-demo".to_owned()),
            service: Some("api".to_owned()),
            summary: "un agente intentó levantar otra instancia".to_owned(),
        }]);
        assert_eq!(
            text,
            "14:02  api          un agente intentó levantar otra instancia\n"
        );
    }
}
