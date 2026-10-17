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

/// Renders the listening processes as a table, one row per process.
#[must_use]
pub fn render_table(processes: &[ListeningProcess]) -> String {
    if processes.is_empty() {
        return "No hay procesos escuchando en TCP.\n".to_owned();
    }
    let mut sorted: Vec<&ListeningProcess> = processes.iter().collect();
    sorted.sort_by_key(|process| (process.ports.first().copied(), process.info.id.pid));

    let mut out = String::from("PUERTOS        PID    PPID  CARPETA / COMANDO\n");
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
        let command: String = process
            .info
            .argv
            .join(" ")
            .chars()
            .take(COMMAND_WIDTH)
            .collect();
        let _ = writeln!(
            out,
            "{ports:<14} {:<6} {:<5} {cwd}\n{:<28}{command}",
            process.info.id.pid, process.info.ppid, ""
        );
    }
    out
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
        let table = render_table(&[
            listening(30, &[8081], &["php", "spark", "serve"]),
            listening(20, &[3000, 3001], &["node", "server.js"]),
        ]);
        let node = table.find("node server.js").unwrap();
        let php = table.find("php spark serve").unwrap();
        assert!(node < php, "port 3000 sorts before 8081:\n{table}");
        assert!(table.contains("3000,3001"));
        assert!(table.contains("/work/shop"));
    }

    #[test]
    fn a_long_command_is_cut_without_splitting_a_character() {
        let long = "é".repeat(200);
        let table = render_table(&[listening(1, &[80], &[long.as_str()])]);
        assert!(table.contains(&"é".repeat(COMMAND_WIDTH)));
        assert!(!table.contains(&"é".repeat(COMMAND_WIDTH + 1)));
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
