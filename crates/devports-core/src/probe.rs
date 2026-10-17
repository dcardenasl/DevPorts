//! Phase 0 probe: a read-only `ls` without the core process.
//!
//! Phase 0 measures whether visibility alone cuts the daily duplicates
//! (ADR-022), before the IPC server exists. `devports-probe ls` scans the host
//! directly and records every invocation in `opens.log`, which is the "opens per
//! day" metric of that gate. The `devports` CLI cannot do this itself because it
//! must not depend on the core.
//!
//! Temporary by design: task T1.10 replaces it with `devports ls` talking to the
//! core over the socket, and this module and its binary are deleted then.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use jiff::Zoned;

use crate::catalog::{Catalog, Classification, ContainerClassification};
use crate::discovery::{self, Discovery, DiscoveryIssue};
use crate::docker::Container;
use crate::fingerprint::Fingerprint;
use crate::process::ListeningProcess;

/// Longest command line shown in the table, in characters.
const COMMAND_WIDTH: usize = 70;

/// Directory of the baseline data: `DEVPORTS_BASELINE_DIR`, or
/// `~/.devports-baseline` (the folder `tools/baseline.sh` already uses).
#[must_use]
pub fn baseline_dir() -> Option<PathBuf> {
    std::env::var_os("DEVPORTS_BASELINE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".devports-baseline"))
        })
}

/// Appends one line to `opens.log` inside `dir`: local ISO-8601 time, a tab and
/// the command name.
///
/// # Errors
///
/// Fails if the directory or the file cannot be written.
pub fn record_open(dir: &Path, command: &str, now: &Zoned) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("opens.log"))?;
    writeln!(file, "{}\t{command}", now.strftime("%Y-%m-%dT%H:%M:%S%:z"))
}

/// Number of recorded opens per local date, from the contents of `opens.log`.
/// Lines that do not start with a `YYYY-MM-DD` date are ignored.
#[must_use]
pub fn opens_per_day(log: &str) -> BTreeMap<String, usize> {
    let mut days = BTreeMap::new();
    for line in log.lines() {
        let date: String = line.chars().take(10).collect();
        if is_iso_date(&date) {
            let count = days.entry(date).or_insert(0_usize);
            *count = count.saturating_add(1);
        }
    }
    days
}

fn is_iso_date(text: &str) -> bool {
    text.len() == 10
        && text.char_indices().all(|(index, character)| match index {
            4 | 7 => character == '-',
            _ => character.is_ascii_digit(),
        })
}

/// Renders the opens-per-day summary.
#[must_use]
pub fn render_opens(days: &BTreeMap<String, usize>) -> String {
    if days.is_empty() {
        return "Sin aperturas registradas todavía.\n".to_owned();
    }
    let mut out = String::from("FECHA       APERTURAS\n");
    for (date, count) in days {
        let _ = writeln!(out, "{date}  {count}");
    }
    out
}

/// Folders searched for projects: `DEVPORTS_ROOTS` (separated like `PATH`), or
/// `~/developer` (ADR-024).
#[must_use]
pub fn search_roots() -> Vec<PathBuf> {
    if let Some(value) = std::env::var_os("DEVPORTS_ROOTS") {
        return std::env::split_paths(&value).collect();
    }
    std::env::var_os("HOME")
        .map(|home| discovery::default_roots(Path::new(&home)))
        .unwrap_or_default()
}

/// What the catalog says about a process, in one short phrase.
fn describe(classification: &Classification<'_>) -> String {
    match classification {
        Classification::Service { project, service } => {
            format!("{}/{}", project.name, service.name)
        }
        Classification::UnregisteredInProject { project } => {
            format!("{} (sin registrar)", project.name)
        }
        Classification::Ambiguous { project, .. } => format!("{} (ambiguo)", project.name),
        Classification::NoProject => "sin proyecto".to_owned(),
    }
}

/// Renders the listening processes as a table, three lines per process: ports,
/// pid and owning project on the first, working directory and command below.
#[must_use]
pub fn render_table(processes: &[ListeningProcess], catalog: &Catalog) -> String {
    if processes.is_empty() {
        return "No hay procesos escuchando en TCP.\n".to_owned();
    }
    let mut sorted: Vec<&ListeningProcess> = processes.iter().collect();
    sorted.sort_by_key(|process| (process.ports.first().copied(), process.info.id.pid));

    let mut out = String::from("PUERTOS        PID    PPID  PROYECTO / SERVICIO\n");
    for process in sorted {
        let ports = process
            .ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let cwd = process
            .info
            .cwd
            .as_deref()
            .map_or_else(|| "-".to_owned(), |path| path.display().to_string());
        // The normalised command line is what a manifest's `match` is compared with.
        let command: String = Fingerprint::from_argv(&process.info.argv)
            .render()
            .chars()
            .take(COMMAND_WIDTH)
            .collect();
        let _ = writeln!(
            out,
            "{ports:<14} {:<6} {:<5} {}\n{:<28}{cwd}\n{:<28}{command}",
            process.info.id.pid,
            process.info.ppid,
            describe(&catalog.classify(process)),
            "",
            ""
        );
    }
    out
}

/// What the catalog says about a container, in one short phrase.
fn describe_container(classification: &ContainerClassification<'_>) -> String {
    match classification {
        ContainerClassification::Service { project, service } => {
            format!("{}/{}", project.name, service.name)
        }
        ContainerClassification::Ambiguous { .. } => "ambiguo".to_owned(),
        ContainerClassification::InProject { project } => {
            format!("{} (compose, sin registrar)", project.name)
        }
        ContainerClassification::Compose { name, .. } => format!("compose: {name}"),
        ContainerClassification::NoProject => "sin proyecto".to_owned(),
    }
}

/// Renders the containers as a table: state, name, published ports and owner.
#[must_use]
pub fn render_containers(containers: &[Container], catalog: &Catalog) -> String {
    if containers.is_empty() {
        return "No hay contenedores.\n".to_owned();
    }
    let mut sorted: Vec<&Container> = containers.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out =
        String::from("ESTADO       CONTENEDOR                     PUERTOS      PROYECTO\n");
    for container in sorted {
        let ports = container
            .ports
            .iter()
            .map(|port| format!("{}->{}", port.host_port, port.container_port))
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            out,
            "{:<12} {:<30} {:<12} {}",
            format!("{:?}", container.state).to_lowercase(),
            container.name,
            if ports.is_empty() { "-" } else { &ports },
            describe_container(&catalog.classify_container(container)),
        );
    }
    out
}

/// Renders the discovered projects, their services and any issues.
#[must_use]
pub fn render_projects(discovery: &Discovery, elapsed: std::time::Duration) -> String {
    let mut out = format!(
        "{} proyecto(s) encontrados en {} ms\n",
        discovery.catalog.projects().len(),
        elapsed.as_millis()
    );
    for project in discovery.catalog.projects() {
        let _ = writeln!(out, "\n{}  {}", project.name, project.root.display());
        for service in &project.services {
            let place = service.cwd.as_deref().map_or_else(
                || format!("docker {}", service.docker.as_deref().unwrap_or("?")),
                |cwd| cwd.display().to_string(),
            );
            let _ = writeln!(out, "  {:<16} {place}", service.name);
        }
    }
    for issue in &discovery.issues {
        let _ = writeln!(out, "\naviso: {}", describe_issue(issue));
    }
    out
}

fn describe_issue(issue: &DiscoveryIssue) -> String {
    match issue {
        DiscoveryIssue::RootMissing(path) => {
            format!("la carpeta raíz {} no existe", path.display())
        }
        DiscoveryIssue::Unreadable { path, reason } => {
            format!("no se pudo leer {}: {reason}", path.display())
        }
        DiscoveryIssue::Manifest(error) => format!("manifiesto no cargado: {error}"),
        DiscoveryIssue::DuplicateProjectName { name, roots } => {
            let places: Vec<String> = roots
                .iter()
                .map(|root| root.display().to_string())
                .collect();
            format!(
                "el nombre de proyecto «{name}» se repite en: {}",
                places.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ProcessId, ProcessInfo, ProcessStart};

    fn listening(pid: u32, ports: &[u16], argv: &[&str]) -> ListeningProcess {
        ListeningProcess {
            info: ProcessInfo {
                id: ProcessId {
                    pid,
                    start: ProcessStart(1),
                },
                ppid: 1,
                pgid: pid,
                uid: 501,
                argv: argv.iter().map(|part| (*part).to_owned()).collect(),
                cwd: Some(PathBuf::from("/work/shop")),
            },
            ports: ports.to_vec(),
        }
    }

    #[test]
    fn opens_are_counted_per_date_and_junk_is_ignored() {
        let log = "2026-10-17T09:00:00-03:00\tls\n\
                   2026-10-17T11:30:00-03:00\tls\n\
                   2026-10-18T08:00:00-03:00\tls\n\
                   garbage line\n\
                   \n";
        let days = opens_per_day(log);
        assert_eq!(days.get("2026-10-17"), Some(&2));
        assert_eq!(days.get("2026-10-18"), Some(&1));
        assert_eq!(days.len(), 2);
    }

    #[test]
    fn an_empty_log_renders_a_message() {
        assert!(render_opens(&BTreeMap::new()).contains("Sin aperturas"));
    }

    #[test]
    fn the_table_is_ordered_by_first_port_then_pid() {
        let table = render_table(
            &[
                listening(30, &[8081], &["php", "spark", "serve"]),
                listening(20, &[3000, 3001], &["node", "server.js"]),
            ],
            &Catalog::default(),
        );
        let node = table.find("node server.js").unwrap();
        let php = table.find("php spark serve").unwrap();
        assert!(node < php, "port 3000 sorts before 8081:\n{table}");
        assert!(table.contains("3000,3001"));
        assert!(table.contains("/work/shop"));
    }

    #[test]
    fn a_long_command_is_cut_without_splitting_a_character() {
        let long = "é".repeat(200);
        let table = render_table(
            &[listening(1, &[80], &[long.as_str()])],
            &Catalog::default(),
        );
        assert!(table.contains(&"é".repeat(COMMAND_WIDTH)));
        assert!(!table.contains(&"é".repeat(COMMAND_WIDTH + 1)));
    }

    #[test]
    fn containers_are_listed_with_state_ports_and_owner() {
        use crate::docker::{ComposeLabels, ContainerState, PublishedPort};
        let containers = [
            Container {
                id: "1".to_owned(),
                name: "mysql".to_owned(),
                image: "mysql:8".to_owned(),
                state: ContainerState::Running,
                ports: vec![PublishedPort {
                    host_port: 3306,
                    container_port: 3306,
                }],
                compose: Some(ComposeLabels {
                    project: "docker-services".to_owned(),
                    working_dir: None,
                }),
                created: 1,
            },
            Container {
                id: "2".to_owned(),
                name: "adminer".to_owned(),
                image: "adminer".to_owned(),
                state: ContainerState::Exited,
                ports: Vec::new(),
                compose: None,
                created: 1,
            },
        ];
        let table = render_containers(&containers, &Catalog::default());
        let adminer = table.find("adminer").unwrap();
        let mysql = table.find("mysql").unwrap();
        assert!(adminer < mysql, "sorted by name:\n{table}");
        assert!(table.contains("running"));
        assert!(table.contains("3306->3306"));
        assert!(table.contains("compose: docker-services"));
        assert!(table.contains("exited"));
        assert!(table.contains("sin proyecto"));
    }

    #[test]
    fn recording_an_open_appends_a_dated_line() {
        let dir = std::env::temp_dir().join(format!("devports-probe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let now: Zoned = "2026-10-17T09:15:00-03:00[America/Santiago]"
            .parse()
            .unwrap();
        record_open(&dir, "ls", &now).unwrap();
        record_open(&dir, "ls", &now).unwrap();
        let log = std::fs::read_to_string(dir.join("opens.log")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            log,
            "2026-10-17T09:15:00-03:00\tls\n2026-10-17T09:15:00-03:00\tls\n"
        );
        assert_eq!(opens_per_day(&log).get("2026-10-17"), Some(&2));
    }
}
