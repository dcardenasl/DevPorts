//! U-03: every error and warning of `MANIFIESTO.md`, section 1, with its line.

use super::*;
use crate::diagnostic::Severity;

const FULL: &str = "\
version: 1
project: tienda-demo
uses: [mysql]
services:
  api:
    cwd: ./tienda-api
    start: php spark serve --port ${PORT}
    port: 8080
    health: http:/health
    match: php spark serve
  cache:
    docker: tienda-redis
    kind: data
    port: 6379
    start: docker start tienda-redis
";

fn path() -> &'static Path {
    Path::new("m.yml")
}

fn load(text: &str) -> Result<Loaded, ManifestError> {
    parse_checked(text, path(), None)
}

/// The error messages of a manifest that must not load, with their lines.
fn errors(text: &str) -> Vec<(Option<u64>, String)> {
    match load(text) {
        Err(ManifestError::Invalid { diagnostics, .. }) => diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| (diagnostic.line, diagnostic.message))
            .collect(),
        other => panic!("expected an invalid manifest, got {other:?}"),
    }
}

/// The warnings of a manifest that must load.
fn warnings(text: &str) -> Vec<(Option<u64>, String)> {
    load(text)
        .unwrap_or_else(|error| panic!("expected a valid manifest: {error}"))
        .warnings
        .into_iter()
        .map(|diagnostic| (diagnostic.line, diagnostic.message))
        .collect()
}

fn service(extra: &str) -> String {
    format!("version: 1\nproject: p\nservices:\n  api:\n{extra}")
}

#[track_caller]
fn assert_error(text: &str, line: u64, fragment: &str) {
    let found = errors(text);
    assert!(
        found
            .iter()
            .any(|(at, message)| *at == Some(line) && message.contains(fragment)),
        "no error with «{fragment}» on line {line}; got {found:?}"
    );
}

#[track_caller]
fn assert_warning(text: &str, line: u64, fragment: &str) {
    let found = warnings(text);
    assert!(
        found
            .iter()
            .any(|(at, message)| *at == Some(line) && message.contains(fragment)),
        "no warning with «{fragment}» on line {line}; got {found:?}"
    );
}

#[test]
fn the_documented_example_loads_without_warnings() {
    let loaded = load(FULL).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let manifest = loaded.manifest;
    assert_eq!(manifest.project, "tienda-demo");
    assert_eq!(manifest.uses, ["mysql"]);
    let api = &manifest.services["api"];
    assert_eq!(api.cwd.as_deref(), Some("./tienda-api"));
    assert_eq!(api.port, Some(8080));
    assert_eq!(api.health, Health::Http("/health".to_owned()));
    assert_eq!(api.match_text.as_deref(), Some("php spark serve"));
    assert_eq!(api.kind, ServiceKind::App);
    let cache = &manifest.services["cache"];
    assert_eq!(cache.docker.as_deref(), Some("tienda-redis"));
    assert_eq!(cache.kind, ServiceKind::Data);
    assert_eq!(cache.health, Health::Tcp);
}

#[test]
fn an_empty_services_mapping_is_valid() {
    let loaded = load("version: 1\nproject: empty\nservices: {}\n").unwrap();
    assert!(loaded.manifest.services.is_empty());
}

// --- errors ---

#[test]
fn a_missing_version_is_an_error() {
    assert!(
        errors("project: p\nservices: {}\n")
            .iter()
            .any(|(_, message)| message.contains("falta «version»"))
    );
}

#[test]
fn another_version_is_an_error_on_its_line() {
    assert_error(
        "version: 2\nproject: p\nservices: {}\n",
        1,
        "versión 2 no soportada",
    );
}

#[test]
fn project_and_services_are_required() {
    let missing = errors("version: 1\n");
    assert!(
        missing
            .iter()
            .any(|(_, message)| message.contains("falta «project»"))
    );
    assert!(
        missing
            .iter()
            .any(|(_, message)| message.contains("falta «services»"))
    );
}

#[test]
fn project_names_follow_the_pattern() {
    for name in ["Tienda", "-x", "a_b", "tienda demo", &"a".repeat(64)] {
        let text = format!("version: 1\nproject: \"{name}\"\nservices: {{}}\n");
        assert_error(&text, 2, "no es válido");
    }
    for name in ["a", "0abc", "tienda-demo", &"a".repeat(63)] {
        let text = format!("version: 1\nproject: {name}\nservices: {{}}\n");
        assert!(load(&text).is_ok(), "{name} should be valid");
    }
}

#[test]
fn service_names_follow_the_pattern() {
    assert_error(
        "version: 1\nproject: p\nservices:\n  Api:\n    cwd: .\n",
        4,
        "nombre de servicio",
    );
}

#[test]
fn a_service_needs_exactly_one_of_cwd_or_docker() {
    assert_error(&service("    port: 8080\n"), 4, "necesita «cwd»");
    assert_error(
        &service("    cwd: .\n    docker: x\n"),
        4,
        "exactamente uno",
    );
}

#[test]
fn cwd_must_be_relative_and_stay_inside_the_project() {
    assert_error(&service("    cwd: /etc\n"), 5, "relativa al manifiesto");
    assert_error(&service("    cwd: ~/x\n"), 5, "relativa al manifiesto");
    assert_error(&service("    cwd: ../other\n"), 5, "sale de la carpeta");
    assert_error(&service("    cwd: a/../../b\n"), 5, "sale de la carpeta");
    assert!(load(&service("    cwd: a/../b\n")).is_ok());
    assert!(load(&service("    cwd: ./a/b\n")).is_ok());
}

#[test]
fn ports_must_be_valid_and_unique_within_the_project() {
    assert_error(&service("    cwd: .\n    port: 0\n"), 6, "entre 1 y 65535");
    assert_error(
        &service("    cwd: .\n    port: 70000\n"),
        6,
        "entre 1 y 65535",
    );
    assert_error(&service("    cwd: .\n    port: -3\n"), 6, "entre 1 y 65535");
    let twice = "version: 1\nproject: p\nservices:\n  a:\n    cwd: .\n    port: 8080\n  b:\n    cwd: .\n    port: 8080\n";
    assert_error(twice, 9, "ya lo usa «a»");
}

#[test]
fn health_must_be_tcp_or_an_http_path() {
    assert_error(
        &service("    cwd: .\n    health: ping\n"),
        6,
        "`tcp` o `http:/ruta`",
    );
    assert_error(
        &service("    cwd: .\n    health: http:health\n"),
        6,
        "`tcp` o `http:/ruta`",
    );
    assert!(load(&service("    cwd: .\n    health: tcp\n")).is_ok());
    assert!(load(&service("    cwd: .\n    health: http:/up\n")).is_ok());
}

#[test]
fn kind_must_be_app_or_data() {
    assert_error(
        &service("    cwd: .\n    kind: cache\n"),
        6,
        "`app` o `data`",
    );
}

#[test]
fn start_must_be_a_single_line() {
    let text = service("    cwd: .\n    start: \"npm install\\nnpm start\"\n");
    assert_error(&text, 6, "un solo comando");
}

#[test]
fn invalid_yaml_reports_its_line() {
    let result = load("version: 1\nproject: [oops\n");
    let Err(ManifestError::Invalid { diagnostics, .. }) = result else {
        panic!("expected an error");
    };
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].line.is_some(), "{:?}", diagnostics[0]);
}

#[test]
fn a_value_of_the_wrong_type_is_an_error_not_a_crash() {
    assert!(!errors(&service("    cwd: .\n    port: abc\n")).is_empty());
}

#[test]
fn every_error_is_reported_at_once() {
    let text = "version: 1\nproject: BAD\nservices:\n  Api:\n    port: 0\n";
    assert!(errors(text).len() >= 3);
}

#[test]
fn a_missing_file_is_a_read_error() {
    let error = load_file("/definitely/not/here/.devports.yml");
    assert!(matches!(error, ManifestError::Read { .. }));
}

fn load_file(path: &str) -> ManifestError {
    super::load(Path::new(path), None).unwrap_err()
}

// --- warnings ---

#[test]
fn unknown_keys_are_warned_about_with_their_line() {
    assert_warning(
        "version: 1\nproject: p\nservices: {}\nextra: 1\n",
        4,
        "clave desconocida «extra»",
    );
    assert_warning(
        &service("    cwd: .\n    colour: red\n"),
        6,
        "clave desconocida «colour»",
    );
}

#[test]
fn the_test_key_has_its_own_message() {
    assert_warning(
        "version: 1\nproject: p\nservices: {}\ntest:\n  db: x\n",
        4,
        "no gestiona configuración de pruebas",
    );
}

#[test]
fn uses_must_exist_in_shared_when_it_is_known() {
    let text = "version: 1\nproject: p\nuses: [mysql, ghost]\nservices: {}\n";
    let shared: BTreeSet<String> = ["mysql".to_owned()].into_iter().collect();
    let loaded = parse_checked(text, path(), Some(&shared)).unwrap();
    assert_eq!(loaded.warnings.len(), 1);
    assert!(loaded.warnings[0].message.contains("«ghost»"));
    assert_eq!(loaded.warnings[0].line, Some(3));
    // Without the list there is nothing to compare against.
    assert!(load(text).unwrap().warnings.is_empty());
}

#[test]
fn a_privileged_port_is_a_warning_not_an_error() {
    assert_warning(&service("    cwd: .\n    port: 80\n"), 6, "menor que 1024");
}

#[test]
fn an_empty_match_is_a_warning() {
    assert_warning(
        &service("    cwd: .\n    match: \"\"\n"),
        6,
        "no reconocerá ningún proceso",
    );
}
