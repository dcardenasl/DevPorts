//! Ignore rules and classification corrections.

use super::*;

pub(super) fn loose_views(world: &World) -> Vec<InstanceView> {
    world
        .engine
        .views()
        .iter()
        .filter(|view| view.cwd.as_deref() == Some("/tmp/scratch"))
        .cloned()
        .collect()
}

// covers: UXR-5.1
#[test]
pub(super) fn an_ignored_server_stays_ignored_when_recreated_and_comes_back_when_restored() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    let table = |pid: u32, start: i64, port: u16| {
        vec![proc(
            pid,
            1,
            start,
            "/tmp/scratch",
            Some(port),
            &["node", "server.js"],
        )]
    };
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    world.scan(table(40, base, 5000));
    let loose = loose_views(&world);
    assert_eq!(loose.len(), 1);
    assert!(!loose[0].ignored);
    let preview = world.engine.rule_preview(&loose[0].instance_ref).unwrap();
    assert_eq!(preview.folder.as_deref(), Some("/tmp/scratch"));
    assert_eq!(preview.command, ["node", "server.js"]);
    assert!(
        world.engine.ignored_rules().unwrap().is_empty(),
        "a preview saves nothing"
    );
    world
        .engine
        .ignore_instance(&loose[0].instance_ref)
        .unwrap();

    // The same command in the same folder, as a new process on another port.
    world.scan(table(41, base + 10, 5001));
    assert!(loose_views(&world)[0].ignored);

    // After a restart the rule is still there, listed with its criterion even with
    // nothing running that matches it.
    drop(world);
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    let rules = world.engine.ignored_rules().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].folder.as_deref(), Some("/tmp/scratch"));
    assert!(!rules[0].legacy && rules[0].created_at.is_some());
    assert!(loose_views(&world).is_empty());
    world.scan(table(42, base + 20, 5002));
    assert!(loose_views(&world)[0].ignored);

    // A different folder or command is not hidden by that rule.
    world.scan(vec![
        proc(
            42,
            1,
            base + 20,
            "/tmp/scratch",
            Some(5002),
            &["node", "server.js"],
        ),
        proc(
            43,
            1,
            base + 30,
            "/tmp/scratch",
            Some(5003),
            &["node", "other.js"],
        ),
        proc(
            44,
            1,
            base + 40,
            "/tmp/elsewhere",
            Some(5004),
            &["node", "server.js"],
        ),
    ]);
    let ignored: Vec<_> = world
        .engine
        .views()
        .iter()
        .filter(|view| view.ignored)
        .map(|view| view.pid)
        .collect();
    assert_eq!(ignored, [Some(42)]);

    // Restoring needs no SQLite edit, is idempotent and is recorded.
    let id = rules[0].id.clone();
    world.engine.restore_ignored(&id).unwrap();
    assert!(world.engine.views().iter().all(|view| !view.ignored));
    assert!(world.engine.ignored_rules().unwrap().is_empty());
    assert!(world.events().contains(&"restored".to_owned()));
    world.engine.restore_ignored(&id).unwrap();
    assert!(world.engine.restore_ignored("nothing-like-it").is_ok());
}

// covers: UXR-5.1
#[test]
pub(super) fn rules_from_earlier_builds_still_hide_only_what_they_did_and_can_be_restored() {
    let store = Store::open_in_memory().unwrap();
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    store
        .add_ignored("process:/tmp/scratch\u{1f}node\u{1f}node server.js", 7)
        .unwrap();
    let mut world = world_with(
        store,
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    world.scan(vec![
        proc(
            50,
            1,
            base,
            "/tmp/scratch",
            Some(5100),
            &["node", "server.js"],
        ),
        proc(
            51,
            1,
            base,
            "/tmp/scratch",
            Some(5101),
            &["node", "server.js", "--x"],
        ),
    ]);
    let ignored: Vec<_> = world
        .engine
        .views()
        .iter()
        .filter(|view| view.ignored)
        .map(|view| view.pid)
        .collect();
    assert_eq!(ignored, [Some(50)]);
    let rules = world.engine.ignored_rules().unwrap();
    assert!(rules[0].legacy);
    assert_eq!(rules[0].created_at.as_deref(), Some("1970-01-01T00:00:07Z"));
    world.engine.restore_ignored(&rules[0].id).unwrap();
    assert!(world.engine.views().iter().all(|view| !view.ignored));
}

// covers: UXR-5.1
#[test]
pub(super) fn a_container_rule_belongs_to_its_docker_context() {
    let mut world = world();
    world.engine.set_docker_context("colima");
    world.engine.observe_containers(Some(vec![container(
        "c9",
        "scratch-db",
        "redis:7",
        6390,
        None,
    )]));
    let view = world
        .engine
        .views()
        .iter()
        .find(|view| view.container.as_deref() == Some("scratch-db"))
        .cloned()
        .unwrap();
    let preview = world.engine.rule_preview(&view.instance_ref).unwrap();
    assert_eq!(preview.context.as_deref(), Some("colima"));
    world.engine.ignore_instance(&view.instance_ref).unwrap();
    let hidden = |world: &World| {
        world
            .engine
            .views()
            .iter()
            .find(|view| view.container.as_deref() == Some("scratch-db"))
            .map(|view| view.ignored)
    };
    assert_eq!(hidden(&world), Some(true));
    world.engine.set_docker_context("desktop-linux");
    assert_eq!(hidden(&world), Some(false), "same name on another context");
    world.engine.set_docker_context("colima");
    assert_eq!(hidden(&world), Some(true));
}

pub(super) fn find_loose(world: &World, cwd: &str) -> InstanceView {
    world
        .engine
        .views()
        .iter()
        .find(|view| view.cwd.as_deref() == Some(cwd))
        .cloned()
        .unwrap()
}

// covers: UXR-5.2
#[test]
pub(super) fn a_correction_moves_only_the_panel_group_and_survives_a_restart() {
    use crate::classify::ProcessClass::{Auxiliary, Unclassified};
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    let table = |pid: u32, start: i64| {
        vec![
            proc(
                pid,
                1,
                start,
                "/tmp/scratch",
                Some(5300),
                &["node", "server.js"],
            ),
            proc(
                pid + 1,
                1,
                start,
                "/tmp/other",
                Some(5301),
                &["node", "server.js"],
            ),
        ]
    };
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    world.scan(table(60, base));
    let classes = |world: &World| -> Vec<(String, crate::classify::ProcessClass, bool)> {
        let mut found: Vec<_> = world
            .engine
            .classifications()
            .into_iter()
            .map(|item| {
                let cwd = world
                    .engine
                    .views()
                    .iter()
                    .find(|view| view.instance_ref == item.instance_ref)
                    .and_then(|view| view.cwd.clone())
                    .unwrap();
                (cwd, item.class, item.corrected)
            })
            .collect();
        found.sort();
        found
    };
    assert_eq!(
        classes(&world),
        [
            ("/tmp/other".to_owned(), Unclassified, false),
            ("/tmp/scratch".to_owned(), Unclassified, false),
        ]
    );
    let before = world.engine.views().to_vec();
    let target = find_loose(&world, "/tmp/scratch");
    world
        .engine
        .set_classification(&target.instance_ref, Some(Auxiliary))
        .unwrap();
    // Only that exact folder and command moved; nothing was ignored, adopted or stopped.
    assert_eq!(
        classes(&world),
        [
            ("/tmp/other".to_owned(), Unclassified, false),
            ("/tmp/scratch".to_owned(), Auxiliary, true),
        ]
    );
    assert_eq!(world.engine.views(), before.as_slice());
    assert!(world.engine.ignored_rules().unwrap().is_empty());
    assert!(world.events().contains(&"reclassified".to_owned()));

    // A restart, a new pid and another start time: the correction still applies.
    drop(world);
    let mut world = world_with(
        Store::open(&path).unwrap(),
        Arc::new(ManualClock::new(START.parse::<Timestamp>().unwrap())),
    );
    world.scan(table(70, base + 10));
    assert_eq!(
        classes(&world)[1],
        ("/tmp/scratch".to_owned(), Auxiliary, true)
    );
    let target = find_loose(&world, "/tmp/scratch");
    world
        .engine
        .set_classification(&target.instance_ref, None)
        .unwrap();
    assert_eq!(
        classes(&world)[1],
        ("/tmp/scratch".to_owned(), Unclassified, false)
    );
    assert!(world.engine.set_classification("nope", None).is_err());
}

// covers: UXR-5.2
#[test]
pub(super) fn ignored_and_declared_things_are_not_classified_and_corrections_are_bounded() {
    let mut world = world();
    let base = START.parse::<Timestamp>().unwrap().as_second() - 100;
    world.scan(vec![proc(
        80,
        1,
        base,
        "/tmp/scratch",
        Some(5400),
        &["node", "a.js"],
    )]);
    let target = find_loose(&world, "/tmp/scratch");
    world.engine.ignore_instance(&target.instance_ref).unwrap();
    assert!(
        world.engine.classifications().is_empty(),
        "an ignored process is not listed, so it is not classified"
    );
    world
        .engine
        .restore_ignored(&crate::ignore::rule_id(
            &crate::ignore::rule_key(&target, "default").unwrap(),
        ))
        .unwrap();
    for index in 0..256 {
        world.engine.corrections.insert(
            format!("synthetic-{index}"),
            crate::classify::ProcessClass::Project,
        );
    }
    let result = world.engine.set_classification(
        &target.instance_ref,
        Some(crate::classify::ProcessClass::Auxiliary),
    );
    assert!(result.is_err(), "too many corrections");
    assert!(
        world
            .engine
            .set_classification(&target.instance_ref, None)
            .is_ok()
    );
}
