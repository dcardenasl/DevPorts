use super::*;
use crate::diagnostic::Severity;

fn home() -> &'static Path {
    Path::new("/Users/me")
}

fn parse(text: &str) -> Result<(Config, Vec<Diagnostic>), ConfigError> {
    Config::parse(text, Path::new("config.yml"), home())
}

#[track_caller]
fn assert_invalid(text: &str, line: u32, fragment: &str) {
    let Err(ConfigError::Invalid { diagnostics, .. }) = parse(text) else {
        panic!("expected an invalid config for: {text}");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error
                && diagnostic.line == Some(line)
                && diagnostic.message().contains(fragment)),
        "no error with «{fragment}» on line {line}; got {diagnostics:?}"
    );
}

#[test]
fn an_empty_file_gives_the_documented_defaults() {
    let (config, warnings) = parse("version: 1\n").unwrap();
    assert!(warnings.is_empty());
    assert_eq!(config, Config::defaults(home()));
    assert_eq!(config.roots, [PathBuf::from("/Users/me/developer")]);
    assert_eq!(config.max_depth, 3);
    assert_eq!(config.lease.default, Duration::from_mins(30));
    assert_eq!(config.lease.max_total, Duration::from_hours(4));
    assert_eq!(config.lease.warn_before, Duration::from_mins(1));
    assert_eq!(config.lease.max_per_service, 2);
    assert_eq!(config.notification_dedupe_window, Duration::from_mins(10));
    assert_eq!(
        config.observation.host_interval,
        Duration::from_millis(1500)
    );
    assert_eq!(config.observation.temporal_port_range, (49152, 65535));
    assert_eq!(config.observation.reservation_grace, Duration::from_mins(1));
    assert_eq!(config.events_retention_days, 30);
    assert_eq!(config.known_agents["claude"], "Claude Code");
}

#[test]
fn a_missing_file_gives_the_defaults() {
    let (config, warnings) =
        Config::load(Path::new("/definitely/not/here/config.yml"), home()).unwrap();
    assert_eq!(config, Config::defaults(home()));
    assert!(warnings.is_empty());
}

#[test]
fn every_key_can_be_overridden() {
    let text = "\
version: 1
roots: [\"~/work\", /srv/code]
max_depth: 2
import_ops_registry: true
lease:
  default: 45m
  max_total: 8h
  extend_by: 15m
  warn_before: 2m
  max_per_service: 3
notifications:
  dedupe_window: 5m
observation:
  host_interval: 2s
  temporal_port_range: [50000, 60000]
  reservation_grace: 30s
events_retention_days: 7
known_agents:
  aider: Aider
";
    let (config, warnings) = parse(text).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        config.roots,
        [PathBuf::from("/Users/me/work"), PathBuf::from("/srv/code")]
    );
    assert_eq!(config.max_depth, 2);
    assert!(config.import_ops_registry);
    assert_eq!(config.lease.default, Duration::from_mins(45));
    assert_eq!(config.lease.max_total, Duration::from_hours(8));
    assert_eq!(config.lease.extend_by, Duration::from_mins(15));
    assert_eq!(config.lease.warn_before, Duration::from_mins(2));
    assert_eq!(config.lease.max_per_service, 3);
    assert_eq!(config.notification_dedupe_window, Duration::from_mins(5));
    assert_eq!(config.observation.host_interval, Duration::from_secs(2));
    assert_eq!(config.observation.temporal_port_range, (50000, 60000));
    assert_eq!(
        config.observation.reservation_grace,
        Duration::from_secs(30)
    );
    assert_eq!(config.events_retention_days, 7);
    // User entries add to the built-in table.
    assert_eq!(config.known_agents["aider"], "Aider");
    assert_eq!(config.known_agents["cursor"], "Cursor");
}

#[test]
fn bad_values_are_errors_on_their_line() {
    assert_invalid("version: 1\nmax_depth: 20\n", 2, "entre 0 y 8");
    assert_invalid("version: 1\nlease:\n  default: soon\n", 3, "lease.default");
    assert_invalid(
        "version: 1\nlease:\n  max_per_service: 0\n",
        3,
        "entre 1 y 16",
    );
    assert_invalid(
        "version: 1\nobservation:\n  host_interval: 10ms\n",
        3,
        "entre 250ms y 10s",
    );
    assert_invalid(
        "version: 1\nobservation:\n  temporal_port_range: [80, 90]\n",
        3,
        "1024 <= inicio",
    );
    assert_invalid(
        "version: 1\nobservation:\n  temporal_port_range: [60000, 50000]\n",
        3,
        "1024 <= inicio",
    );
    assert_invalid("version: 2\n", 1, "versión 2");
    assert_invalid("version: 1\nevents_retention_days: 0\n", 2, "al menos 1");
}

#[test]
fn lease_values_must_make_sense_together() {
    assert_invalid("version: 1\nlease:\n  default: 5h\n", 3, "no puede superar");
    assert_invalid(
        "version: 1\nlease:\n  extend_by: 9h\n",
        3,
        "no puede superar",
    );
    assert_invalid("version: 1\nlease:\n  warn_before: 40m\n", 3, "menor que");
}

#[test]
fn unknown_keys_are_warnings() {
    let (_, warnings) = parse("version: 1\ncolour: red\nlease:\n  nap: 1m\n").unwrap();
    assert_eq!(warnings.len(), 2);
    assert!(
        warnings
            .iter()
            .any(|w| w.line == Some(2) && w.message().contains("colour"))
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.line == Some(4) && w.message().contains("nap"))
    );
}

#[test]
fn broken_yaml_is_an_error_with_a_line() {
    let Err(ConfigError::Invalid { diagnostics, .. }) = parse("roots: [unclosed\n") else {
        panic!("expected an error");
    };
    assert!(diagnostics[0].line.is_some());
}

// --- shared.yml ---

const SHARED: &str = "\
version: 1
services:
  mysql:
    docker: mysql
    kind: data
    port: 3306
    start: docker start mysql
";

#[test]
fn shared_services_are_read_and_named() {
    let (shared, warnings) = SharedServices::parse(SHARED, Path::new("shared.yml")).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(shared.names().into_iter().collect::<Vec<_>>(), ["mysql"]);
    let mysql = &shared.services["mysql"];
    assert_eq!(mysql.docker.as_deref(), Some("mysql"));
    assert_eq!(mysql.port, Some(3306));
    assert_eq!(mysql.kind, crate::manifest::ServiceKind::Data);
}

#[test]
fn a_missing_shared_file_means_there_are_none() {
    let (shared, _) = SharedServices::load(Path::new("/definitely/not/here/shared.yml")).unwrap();
    assert!(shared.services.is_empty());
}

#[test]
fn shared_services_follow_the_same_rules_but_with_absolute_paths() {
    let bad = "version: 1\nservices:\n  api:\n    cwd: ./relative\n";
    let Err(ConfigError::Invalid { diagnostics, .. }) =
        SharedServices::parse(bad, Path::new("s.yml"))
    else {
        panic!("a relative cwd must be refused in shared.yml");
    };
    assert!(diagnostics.iter().any(|d| d.message().contains("absoluta")));
    let good = "version: 1\nservices:\n  api:\n    cwd: ~/work/api\n";
    assert!(SharedServices::parse(good, Path::new("s.yml")).is_ok());
    let missing_target = "version: 1\nservices:\n  api:\n    port: 3000\n";
    assert!(SharedServices::parse(missing_target, Path::new("s.yml")).is_err());
}
