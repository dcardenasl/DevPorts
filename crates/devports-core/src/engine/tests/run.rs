//! `devports run`: who may start what.

use super::*;

#[test]
pub(super) fn the_first_run_becomes_the_principal_on_its_preferred_port() {
    let mut world = world();
    let outcome = world.run(&request("api", RunRole::Principal));
    let accepted = accepted(&outcome);

    assert_eq!(accepted.port, 8080);
    assert_eq!(accepted.role, InstanceRole::Principal);
    assert_eq!(accepted.project, "shop");
    assert_eq!(accepted.url, "http://localhost:8080");
    assert_eq!(accepted.shell, "exec php spark serve --port 8080");
    assert!(accepted.token.is_none(), "a Principal has no stop token");
    assert!(accepted.run_dir.ends_with("/work/shop/api"));
    assert_eq!(accepted.env["PORT"], "8080");
    assert_eq!(accepted.env["DEVPORTS_PORT"], "8080");
    assert_eq!(accepted.env["DEVPORTS_ROLE"], "principal");
    assert_eq!(accepted.env["DEVPORTS_SERVICE"], "api");
    assert_eq!(accepted.env["DEVPORTS_INSTANCE_REF"], accepted.instance_ref);
    assert!(!accepted.env.contains_key("DEVPORTS_STOP_TOKEN"));
    assert!(accepted.banner[0].contains("como PRINCIPAL en http://localhost:8080"));

    let principal = &world.instance(Some("shop"), "api", InstanceRole::Principal)[0];
    assert_eq!(principal.state, InstanceState::Starting);
    assert!(principal.registered);
}

#[test]
pub(super) fn the_project_is_resolved_from_the_folder_or_named_explicitly() {
    let mut world = world();
    let mut elsewhere = request("api", RunRole::Principal);
    elsewhere.cwd = "/tmp".to_owned();
    let refused = world.run(&elsewhere);
    assert_eq!(rejected(&refused).error, RejectKind::NotFound);
    assert!(rejected(&refused).message.contains("`--project`"));

    elsewhere.project = Some("shop".to_owned());
    assert_eq!(accepted(&world.run(&elsewhere)).project, "shop");

    let mut ghost = request("api", RunRole::Principal);
    ghost.project = Some("ghost".to_owned());
    assert_eq!(rejected(&world.run(&ghost)).error, RejectKind::NotFound);
}

// covers: U-04
#[test]
pub(super) fn an_unknown_service_is_refused_with_code_14() {
    let mut world = world();
    let outcome = world.run(&request("nope", RunRole::Principal));
    assert_eq!(rejected(&outcome).error, RejectKind::NotFound);
    assert_eq!(rejected(&outcome).error.exit_code(), 14);
}

// covers: U-04
#[test]
pub(super) fn without_a_command_the_run_is_refused_with_code_13() {
    let mut world = world();
    // `admin` has no `start`.
    let outcome = world.run(&request("admin", RunRole::Principal));
    assert_eq!(rejected(&outcome).error, RejectKind::NoCommand);
    // Giving a command after `--` fixes it.
    let mut with_command = request("admin", RunRole::Principal);
    with_command.command = Some(vec![
        "node".to_owned(),
        "server.js".to_owned(),
        "--port".to_owned(),
        "{PORT}".to_owned(),
    ]);
    assert_eq!(
        accepted(&world.run(&with_command)).shell,
        "exec node server.js --port 8081"
    );
}

#[test]
pub(super) fn arguments_are_quoted_so_they_reach_the_command_unchanged() {
    let mut world = world();
    let mut tricky = request("admin", RunRole::Principal);
    tricky.command = Some(vec![
        "echo".to_owned(),
        "it's $HOME; rm -rf *".to_owned(),
        "{PORT}".to_owned(),
        String::new(),
    ]);
    assert_eq!(
        accepted(&world.run(&tricky)).shell,
        "exec echo 'it'\\''s $HOME; rm -rf *' 8081 ''"
    );
}

#[test]
pub(super) fn a_shared_service_can_be_started_from_a_project_that_uses_it() {
    let mut world = world();
    let mut mysql = request("mysql", RunRole::Principal);
    mysql.command = None;
    let outcome = world.run(&mysql);
    let accepted = accepted(&outcome);
    assert_eq!(accepted.shell, "exec docker start mysql");
    assert_eq!(accepted.port, 3306);
    assert_eq!(accepted.project, "compartidos");
    let view = &world.instance(None, "mysql", InstanceRole::Principal)[0];
    assert!(view.shared);
}

// -------------------------------------------------------------- run: conflicts

// covers: U-04, I-01
#[test]
pub(super) fn a_second_run_is_refused_while_the_first_is_starting_i_01() {
    let mut world = world();
    assert!(matches!(
        world.run(&request("api", RunRole::Principal)),
        RunOutcome::Accepted(_)
    ));
    // Seven agents racing: one Principal, six refusals, nothing else created.
    for _ in 0..6 {
        let outcome = world.run(&request("api", RunRole::Principal));
        assert_eq!(rejected(&outcome).error, RejectKind::Conflict);
        assert_eq!(rejected(&outcome).error.exit_code(), 10);
    }
    assert_eq!(
        world
            .instance(Some("shop"), "api", InstanceRole::Principal)
            .len(),
        1
    );
    assert_eq!(world.engine.tracked.len(), 1);
    let kinds = world.events();
    assert_eq!(
        kinds.iter().filter(|kind| *kind == "run_rejected").count(),
        6
    );
}

#[test]
pub(super) fn the_refusal_names_what_exists_and_the_ways_out() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    let outcome = world.run(&request("api", RunRole::Principal));
    let refusal = rejected(&outcome);
    assert!(refusal.message.contains("conflicto en api (proyecto shop)"));
    assert!(refusal.message.contains("http://localhost:8080"));
    assert!(refusal.message.contains("devports run api --reuse"));
    assert!(refusal.message.contains("detén la instancia desde la app"));
    assert_eq!(
        refusal.existing.as_ref().unwrap().service.as_deref(),
        Some("api")
    );
    assert_eq!(refusal.alternatives.len(), 3);
    assert!(refusal.alternatives.iter().any(|alt| alt.needs_human));
}

// covers: U-04
#[test]
pub(super) fn reuse_starts_nothing_and_reports_the_existing_instance() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    let mut again = request("api", RunRole::Principal);
    again.reuse = true;
    let outcome = world.run(&again);
    let RunOutcome::Reused(reused) = outcome else {
        panic!("expected a reuse");
    };
    assert!(
        reused
            .message
            .contains("api ya corre (Principal) en http://localhost:8080")
    );
    assert_eq!(world.engine.tracked.len(), 1);
    assert!(world.events().contains(&"reuse".to_owned()));
}

// covers: U-04
#[test]
pub(super) fn a_hung_principal_is_refused_with_code_11_even_with_reuse() {
    let mut world = world();
    let accepted = accepted(&world.run(&request("api", RunRole::Principal))).clone();
    // The process is bound and then stops answering.
    world.clock.advance(Duration::from_secs(1));
    let now = epoch(&world);
    world.scan(vec![proc(
        500,
        1,
        now,
        "/work/shop/api",
        Some(8080),
        &["php", "spark", "serve"],
    )]);
    for _ in 0..3 {
        world.engine.record_health(&accepted.instance_ref, false);
    }
    let mut again = request("api", RunRole::Principal);
    again.reuse = true;
    let outcome = world.run(&again);
    assert_eq!(rejected(&outcome).error, RejectKind::Unhealthy);
    assert_eq!(rejected(&outcome).error.exit_code(), 11);
    assert!(rejected(&outcome).message.contains("no responde"));

    // One good check clears it.
    world.engine.record_health(&accepted.instance_ref, true);
    assert!(matches!(world.run(&again), RunOutcome::Reused(_)));
}

// covers: U-04
#[test]
pub(super) fn a_preferred_port_held_by_another_program_is_refused_with_code_18() {
    let mut world = world();
    world.ports.occupy(8080);
    world.scan(vec![proc(
        77,
        1,
        100,
        "/somewhere/else",
        Some(8080),
        &["/usr/bin/node", "server.js"],
    )]);
    let outcome = world.run(&request("api", RunRole::Principal));
    let refusal = rejected(&outcome);
    assert_eq!(refusal.error, RejectKind::PortBusy);
    assert_eq!(refusal.error.exit_code(), 18);
    assert!(
        refusal.message.contains("node (pid 77)"),
        "{}",
        refusal.message
    );
}

// ------------------------------------------------------------------- temporals

// covers: U-04
#[test]
pub(super) fn a_test_run_needs_a_reason() {
    let mut world = world();
    let mut no_reason = request("api", RunRole::Test);
    no_reason.reason = None;
    assert_eq!(rejected(&world.run(&no_reason)).error, RejectKind::BadArgs);
    no_reason.reason = Some("   ".to_owned());
    assert_eq!(rejected(&world.run(&no_reason)).error.exit_code(), 16);
}

// covers: U-04
#[test]
pub(super) fn a_temporal_gets_a_free_port_a_token_and_a_lease() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    let outcome = world.run(&request("api", RunRole::Test));
    let temporal = accepted(&outcome);

    assert_eq!(temporal.role, InstanceRole::Temporal);
    assert!((49152..=65535).contains(&temporal.port));
    assert_ne!(temporal.port, 8080);
    let token = temporal.token.as_deref().unwrap();
    assert_eq!(token.len(), 32);
    assert_eq!(temporal.env["DEVPORTS_STOP_TOKEN"], token);
    assert_eq!(temporal.env["DEVPORTS_ROLE"], "temporal");
    assert!(temporal.env.contains_key("DEVPORTS_EXPIRES_AT"));
    assert!(temporal.banner[1].starts_with("DEVPORTS_STOP_TOKEN="));
    assert!(temporal.banner[0].contains("como TEMPORAL"));
    assert!(temporal.banner[0].contains("(30 min)"));

    // It lives next to the Principal, which is not a duplicate of it.
    assert_eq!(
        world
            .instance(Some("shop"), "api", InstanceRole::Temporal)
            .len(),
        1
    );
    assert_eq!(
        world
            .instance(Some("shop"), "api", InstanceRole::Principal)
            .len(),
        1
    );
}

// covers: U-08, I-08
#[test]
pub(super) fn the_token_is_never_stored_or_shown() {
    let mut world = world();
    let outcome = world.run(&request("api", RunRole::Test));
    let token = accepted(&outcome).token.clone().unwrap();
    let stored = world.engine.store.live_instances().unwrap();
    let json = serde_json::to_string(&stored.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>())
        .unwrap();
    assert!(!json.contains(&token), "the plain token is in the store");
    assert_eq!(
        stored[0].token_hash.as_deref(),
        Some(crate::ids::token_hash(&token).as_str())
    );
    let listed = serde_json::to_string(&world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    }))
    .unwrap();
    assert!(!listed.contains(&token));
    let log = serde_json::to_string(&world.engine.log(&LogArgs::default())).unwrap();
    assert!(!log.contains(&token));
}

// covers: U-04
#[test]
pub(super) fn at_most_two_temporals_per_service() {
    let mut world = world();
    assert!(matches!(
        world.run(&request("api", RunRole::Test)),
        RunOutcome::Accepted(_)
    ));
    assert!(matches!(
        world.run(&request("api", RunRole::Test)),
        RunOutcome::Accepted(_)
    ));
    let third = world.run(&request("api", RunRole::Test));
    assert_eq!(rejected(&third).error, RejectKind::Limit);
    assert_eq!(rejected(&third).error.exit_code(), 12);
    // Another service has its own allowance.
    let mut other = request("admin", RunRole::Test);
    other.command = Some(vec!["true".to_owned()]);
    assert!(matches!(world.run(&other), RunOutcome::Accepted(_)));
}

// covers: U-04
#[test]
pub(super) fn a_ttl_above_the_maximum_is_refused() {
    let mut world = world();
    let mut long = request("api", RunRole::Test);
    long.ttl_seconds = Some(5 * 3600);
    assert_eq!(rejected(&world.run(&long)).error, RejectKind::BadArgs);
    long.ttl_seconds = Some(0);
    assert_eq!(rejected(&world.run(&long)).error, RejectKind::BadArgs);
    long.ttl_seconds = Some(600);
    assert!(matches!(world.run(&long), RunOutcome::Accepted(_)));
}

// covers: U-07
#[test]
pub(super) fn temporals_never_get_the_same_port() {
    let mut world = world();
    let first = accepted(&world.run(&request("api", RunRole::Test))).port;
    let second = accepted(&world.run(&request("api", RunRole::Test))).port;
    assert_ne!(first, second);
}

// ------------------------------------------------------------------- binding
