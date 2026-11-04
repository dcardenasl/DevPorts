//! Stops, extensions and the log as a person or the socket asks for them.

use super::*;

#[test]
pub(super) fn a_person_can_stop_a_principal_and_a_process_nobody_registered() {
    let mut world = world();
    let now = epoch(&world) - 100;
    world.scan(vec![
        proc(
            1,
            1,
            now,
            "/work/shop/api",
            Some(8080),
            &["php", "spark", "serve"],
        ),
        proc(
            2,
            1,
            now + 5,
            "/work/shop/api",
            Some(8090),
            &["php", "spark", "serve"],
        ),
    ]);
    let duplicate = world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0].clone();
    let effects = world.engine.stop_instance(&duplicate.instance_ref).unwrap();
    assert!(
        matches!(&effects[0], Effect::Terminate(request) if request.reason == StopReason::PersonRequest)
    );
    assert_eq!(
        world.engine.stop_instance("ffff").unwrap_err(),
        ActionError::UnknownInstance {
            reference: "ffff".to_owned()
        }
    );
    world.engine.termination_finished(
        &duplicate.instance_ref,
        StopReason::PersonRequest,
        &Terminated::Stopped,
    );
    assert!(world.events().contains(&"stopped".to_owned()));
}

// covers: UXR-2.1
#[test]
pub(super) fn a_manual_stop_rejects_a_changed_or_disappeared_confirmation_target() {
    let mut changed_world = world();
    let now = epoch(&changed_world) - 100;
    changed_world.scan(vec![proc(
        77,
        1,
        now,
        "/work/loose/server",
        Some(8080),
        &["node", "server.js"],
    )]);
    let original = changed_world
        .engine
        .views()
        .iter()
        .find(|view| view.pid == Some(77))
        .cloned()
        .expect("observed target");
    let expectation = StopExpectation::from_view(&original);
    let mut changed = expectation.clone();
    changed.port = Some(9090);
    assert_eq!(
        changed_world
            .engine
            .stop_confirmed(&original.instance_ref, &changed)
            .unwrap_err(),
        ActionError::TargetChanged
    );
    let mut reused_identity = expectation.clone();
    reused_identity.target_identity = Some("new-process-generation".to_owned());
    assert_eq!(
        changed_world
            .engine
            .stop_confirmed(&original.instance_ref, &reused_identity)
            .unwrap_err(),
        ActionError::TargetChanged
    );
    assert!(
        changed_world
            .engine
            .stop_confirmed(&original.instance_ref, &expectation)
            .is_ok()
    );

    let mut gone_world = world();
    gone_world.scan(vec![proc(
        78,
        1,
        now,
        "/work/loose/vanished",
        Some(8081),
        &["node", "server.js"],
    )]);
    let vanished = gone_world
        .engine
        .views()
        .iter()
        .find(|view| view.pid == Some(78))
        .cloned()
        .expect("observed target");
    let expectation = StopExpectation::from_view(&vanished);
    let mut replacement = proc(
        78,
        1,
        now,
        "/work/loose/vanished",
        Some(8081),
        &["node", "server.js"],
    );
    replacement.info.id.start =
        crate::process::ProcessStart(u64::try_from(now).unwrap() * 1_000_000 + 1);
    gone_world.scan(vec![replacement]);
    assert_eq!(
        gone_world
            .engine
            .stop_confirmed(&vanished.instance_ref, &expectation)
            .unwrap_err(),
        ActionError::UnknownInstance {
            reference: vanished.instance_ref
        }
    );
}

#[test]
pub(super) fn a_person_can_extend_a_running_temporal_from_the_application() {
    let (mut world, reference, _, _) = bound_temporal();
    world.engine.extend_instance(&reference).unwrap();
    let view = &world.instance(Some("shop"), "api", InstanceRole::Temporal)[0];
    assert!(view.expires_at.is_some());
    assert!(world.engine.extend_instance("nope").is_err());
}

#[test]
pub(super) fn the_log_tells_the_story_in_spanish() {
    let mut world = world();
    world.run(&request("api", RunRole::Principal));
    world.run(&request("api", RunRole::Principal));
    let log = world.engine.log(&LogArgs::default()).unwrap();
    let summaries: Vec<&str> = log
        .entries
        .iter()
        .map(|entry| entry.summary.as_str())
        .collect();
    assert!(
        summaries
            .iter()
            .any(|summary| summary.contains("DevPorts se inició"))
    );
    assert!(
        summaries
            .iter()
            .any(|summary| summary.contains("registrada como Principal :8080"))
    );
    assert!(
        summaries.iter().any(|summary| summary
            .contains("un agente (cursor > zsh) intentó levantar otra instancia de api")),
        "{summaries:?}"
    );
    let filtered = world.engine.log(&LogArgs {
        project: Some("other".to_owned()),
        since_seconds: None,
    });
    assert!(filtered.unwrap().entries.is_empty());
}

#[test]
pub(super) fn the_socket_dispatcher_answers_each_operation() {
    use devports_proto::{ExtendArgs, LsResult, Op, PingResult, Request, TokenArgs};
    let (mut world, _, token, _) = bound_temporal();
    let ping = world.engine.handle_request(Request::new("1", Op::Ping)).0;
    assert_eq!(ping.into_result::<PingResult>().unwrap().pid, 99);
    let ls = world
        .engine
        .handle_request(Request::new(
            "2",
            Op::Ls(LsArgs {
                all: true,
                ..LsArgs::default()
            }),
        ))
        .0;
    assert!(!ls.into_result::<LsResult>().unwrap().instances.is_empty());
    let extended = world
        .engine
        .handle_request(Request::new(
            "3",
            Op::Extend(ExtendArgs {
                token: token.clone(),
                by_seconds: Some(60),
            }),
        ))
        .0;
    assert!(extended.ok);
    let bad = world
        .engine
        .handle_request(Request::new(
            "4",
            Op::Stop(TokenArgs {
                token: "x".to_owned(),
            }),
        ))
        .0;
    assert_eq!(bad.error.unwrap().code, ErrorCode::UnknownToken);
    let adopt = world
        .engine
        .handle_request(Request::new(
            "5",
            Op::AdoptPropose(devports_proto::AdoptArgs {
                dir: "/definitely/not/here".to_owned(),
            }),
        ))
        .0;
    assert_eq!(adopt.error.unwrap().code, ErrorCode::NotFound);
}

// ------------------------------------------------------------------- keeping

#[test]
pub(super) fn a_kept_duplicate_stops_asking_for_attention_and_the_choice_survives_a_restart() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    let table = |base: i64| {
        vec![
            proc(
                1,
                1,
                base,
                "/work/shop/api",
                Some(8080),
                &["php", "spark", "serve"],
            ),
            proc(
                2,
                1,
                base + 5,
                "/work/shop/api",
                Some(8090),
                &["php", "spark", "serve", "--port", "8090"],
            ),
        ]
    };
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    let duplicate_ref = {
        let mut world = world_with(
            Store::open(&path).unwrap(),
            Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
        );
        world.scan(table(base));
        let duplicate = world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0].clone();
        assert!(!duplicate.ignored);
        world
            .engine
            .ignore_instance(&duplicate.instance_ref)
            .unwrap();
        let kept = &world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0];
        assert!(kept.ignored);
        assert!(world.events().contains(&"kept".to_owned()));
        duplicate.instance_ref
    };

    // After a restart the same command in the same folder is still kept, and it
    // is not announced again.
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    let effects = world.scan(table(base));
    let kept = &world.instance(Some("shop"), "api", InstanceRole::Unregistered)[0];
    assert!(kept.ignored, "{duplicate_ref}");
    assert!(
        effects.is_empty(),
        "a kept duplicate is not news: {effects:?}"
    );
    assert!(world.engine.ignore_instance("nope").is_err());
}

#[test]
pub(super) fn programs_that_ship_with_the_system_or_an_app_are_not_listed() {
    let mut world = world();
    let now = epoch(&world) - 10;
    world.scan(vec![
        proc(
            1,
            1,
            now,
            "/",
            Some(5000),
            &["/System/Library/CoreServices/ControlCenter.app/Contents/MacOS/ControlCenter"],
        ),
        proc(
            2,
            1,
            now,
            "/",
            Some(5001),
            &["/Applications/GitKraken.app/Contents/MacOS/GitKraken"],
        ),
        proc(3, 1, now, "/", Some(5002), &["/usr/libexec/rapportd"]),
        proc(
            4,
            1,
            now,
            "/tmp/scratch",
            Some(7000),
            &["/opt/homebrew/bin/node", "server.js"],
        ),
    ]);
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    let pids: Vec<Option<u32>> = all
        .instances
        .iter()
        .filter_map(|view| view.pid.map(Some))
        .collect();
    assert_eq!(pids, vec![Some(4)]);
}

#[test]
pub(super) fn workers_that_share_their_parents_socket_are_one_server() {
    let mut world = world();
    let now = epoch(&world) - 10;
    world.scan(vec![
        proc(
            100,
            1,
            now,
            "/opt/homebrew/var",
            Some(9000),
            &["php-fpm: master process"],
        ),
        proc(
            101,
            100,
            now,
            "/opt/homebrew/var",
            Some(9000),
            &["php-fpm: pool www"],
        ),
        proc(
            102,
            100,
            now,
            "/opt/homebrew/var",
            Some(9000),
            &["php-fpm: pool www"],
        ),
        proc(200, 1, now, "/tmp/a", Some(7000), &["node", "a.js"]),
        proc(201, 1, now, "/tmp/b", Some(7000), &["node", "b.js"]),
    ]);
    let all = world.engine.ls(&LsArgs {
        all: true,
        ..LsArgs::default()
    });
    let mut pids: Vec<u32> = all.instances.iter().filter_map(|view| view.pid).collect();
    pids.sort_unstable();
    // Unrelated processes on the same port (two dev servers) stay separate.
    assert_eq!(pids, vec![100, 200, 201]);
}
