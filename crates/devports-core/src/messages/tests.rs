//! Golden tests of the wording (`CLI.md`, sections 3 and 6).

use devports_proto::views::{InstanceRole, InstanceState, InstanceView, LogFacts};
use serde_json::json;

use super::*;

fn existing() -> InstanceView {
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
        target_identity: Some("process-fingerprint".to_owned()),
        ports: vec![8080],
        container: None,
        branch: Some("feature/x".to_owned()),
        worktree: Some("/Users/x/tienda-api".to_owned()),
        cwd: None,
        started_at: None,
        expires_at: None,
        reason: None,
        registered: true,
        launcher_chain: Vec::new(),
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
fn spans_read_naturally() {
    assert_eq!(span(Duration::from_secs(45)), "45 s");
    assert_eq!(span(Duration::from_mins(12)), "12 min");
    assert_eq!(span(Duration::from_mins(130)), "2 h 10 min");
    assert_eq!(span(Duration::from_hours(3)), "3 h");
    assert_eq!(span(Duration::ZERO), "0 s");
}

#[test]
fn the_conflict_text_matches_the_documented_one() {
    let text = rejection(
        RejectKind::Conflict,
        Some("tienda-demo"),
        "api",
        Some((&existing(), Some(1_000))),
        &conflict_alternatives("api"),
        "",
        1_000 + 130 * 60,
    );
    assert_eq!(
        text,
        "devports: conflicto en api (proyecto tienda-demo)\n\
         Ya existe la instancia Principal de \"api\":\n\
         \x20 http://localhost:8080   pid 4121   rama feature/x   worktree /Users/x/tienda-api\n\
         \x20 desde hace 2 h 10 min   sana\n\
         No se inició nada. Alternativas:\n\
         \x20 Reutilizarla:         devports run api --reuse\n\
         \x20 Levantar una prueba:  devports run api --as test --reason \"<motivo>\" -- <comando con {PORT}>\n\
         \x20 Reemplazarla:         detén la instancia desde la app de barra y vuelve a ejecutar"
    );
}

#[test]
fn an_unhealthy_principal_says_so() {
    let mut hung = existing();
    hung.state = InstanceState::Unhealthy;
    let text = rejection(
        RejectKind::Unhealthy,
        Some("tienda-demo"),
        "api",
        Some((&hung, Some(0))),
        &conflict_alternatives("api"),
        "",
        90,
    );
    assert!(text.starts_with(
        "devports: la Principal de api (proyecto tienda-demo) existe pero no responde"
    ));
    assert!(text.contains("no responde"));
}

#[test]
fn the_other_refusals_have_their_own_text() {
    let limit = rejection(
        RejectKind::Limit,
        Some("p"),
        "api",
        None,
        &[],
        "Ya hay 2 pruebas: detén una con `devports stop <token>`.",
        0,
    );
    assert!(limit.contains("máximo de instancias de prueba"));
    let no_command = rejection(RejectKind::NoCommand, None, "api", None, &[], "", 0);
    assert!(no_command.contains("no hay comando para iniciar api"));
    assert!(no_command.contains("`start:`"));
    let not_found = rejection(
        RejectKind::NotFound,
        None,
        "api",
        None,
        &[],
        "no se encontró el servicio «api»",
        0,
    );
    assert_eq!(not_found, "devports: no se encontró el servicio «api»");
    let bad = rejection(
        RejectKind::BadArgs,
        None,
        "api",
        None,
        &[],
        "--as test exige --reason",
        0,
    );
    assert_eq!(
        bad,
        "devports: argumentos inválidos: --as test exige --reason"
    );
    let busy = rejection(
        RejectKind::PortBusy,
        Some("p"),
        "api",
        None,
        &[],
        "Lo ocupa node (pid 77).",
        0,
    );
    assert!(busy.contains("puerto preferido de api (proyecto p) está ocupado"));
    let none = rejection(
        RejectKind::NoPort,
        None,
        "api",
        None,
        &[],
        "El rango está lleno.",
        0,
    );
    assert!(none.contains("no hay un puerto libre"));
}

#[test]
fn the_banner_of_a_principal_and_of_a_temporal() {
    assert_eq!(
        banner(
            "api",
            "tienda-demo",
            InstanceRole::Principal,
            "http://localhost:8080",
            None,
            None,
            None
        ),
        ["devports: api (proyecto tienda-demo) como PRINCIPAL en http://localhost:8080"]
    );
    let temporal = banner(
        "api",
        "tienda-demo",
        InstanceRole::Temporal,
        "http://localhost:51234",
        Some(1_700_000_000),
        Some(Duration::from_mins(30)),
        Some("9f1c"),
    );
    assert!(temporal[0].starts_with(
        "devports: api (proyecto tienda-demo) como TEMPORAL en http://localhost:51234, vence "
    ));
    assert!(temporal[0].ends_with("(30 min)"));
    assert_eq!(
        temporal[1],
        "DEVPORTS_STOP_TOKEN=9f1c   (guárdalo: permite detener o extender esta prueba)"
    );
}

#[test]
fn reuse_says_what_runs_and_where() {
    assert_eq!(
        reuse(&existing()),
        "devports: api ya corre (Principal) en http://localhost:8080 (pid 4121)"
    );
}

#[test]
fn the_log_lines_read_like_the_documented_ones() {
    let rejected = event_summary(
        "run_rejected",
        Some("api"),
        &json!({"launcher": "Cursor > zsh", "existing_url": "http://localhost:8080"}),
    );
    assert_eq!(
        rejected,
        "un agente (Cursor > zsh) intentó levantar otra instancia de api; la Principal sigue en http://localhost:8080"
    );
    assert_eq!(
        event_summary(
            "run_accepted",
            Some("api"),
            &json!({"role": "temporal", "port": 51234, "reason": "QA login"})
        ),
        "Prueba de api :51234 para «QA login»"
    );
    assert_eq!(
        event_summary("lease_warned", Some("api"), &json!({"port": 51234})),
        "La prueba de api :51234 vence en 1 minuto"
    );
    assert_eq!(
        event_summary("lease_expired", Some("api"), &json!({"port": 51234})),
        "La prueba de api :51234 venció y se detuvo"
    );
    assert_eq!(
        event_summary("duplicate_detected", Some("api"), &json!({"port": 8083})),
        "Apareció un duplicado de api :8083 sin registrar"
    );
    assert_eq!(
        event_summary("something_new", None, &json!({})),
        "something new"
    );
}

// covers: UXR-9.4
#[test]
fn an_events_facts_are_what_its_sentence_is_written_from() {
    let facts = event_facts(&json!({
        "role": "temporal",
        "port": 51234,
        "reason": "QA login",
        "launcher": "Cursor > zsh",
        "existing_url": "http://localhost:8080",
        "expires_at": 1_700_001_800,
    }));
    assert_eq!(facts.port, Some(51234));
    assert_eq!(facts.reason.as_deref(), Some("QA login"));
    assert_eq!(facts.launcher.as_deref(), Some("Cursor > zsh"));
    assert_eq!(facts.existing_url.as_deref(), Some("http://localhost:8080"));
    assert_eq!(
        facts.expires_at.as_deref(),
        Some(iso(1_700_001_800).as_str())
    );
    // Nothing the event does not carry is invented, and a port out of range is no port.
    assert_eq!(event_facts(&json!({})), LogFacts::default());
    assert_eq!(event_facts(&json!({"port": 70_000})).port, None);
    assert_eq!(event_facts(&json!({"port": "80"})).port, None);
}

#[test]
fn times_are_formatted_in_the_local_zone() {
    // Whatever the machine's zone is, the shapes are fixed.
    assert_eq!(clock(1_700_000_000).len(), 5);
    let stamp = iso(1_700_000_000);
    assert_eq!(stamp.len(), 25, "{stamp}");
    assert!(stamp.contains('T'));
}
