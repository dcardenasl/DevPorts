use super::*;

fn process(cwd: &str, argv: &[&str]) -> InstanceView {
    let mut view: InstanceView = serde_json::from_value(serde_json::json!({
        "ref": "a1b2", "state": "healthy", "role": "unregistered", "kind": "app",
        "registered": false, "launcher_chain": [], "shared": false, "ignored": false,
        "target_identity": null, "pid": 7, "port": 8080, "argv": []
    }))
    .unwrap();
    view.cwd = Some(cwd.to_owned());
    view.argv = argv.iter().map(|arg| (*arg).to_owned()).collect();
    view
}

fn container(name: &str) -> InstanceView {
    let mut view = process("/", &[]);
    view.cwd = None;
    view.container = Some(name.to_owned());
    view
}

// covers: UXR-5.1
#[test]
fn arguments_are_a_list_so_spaces_cannot_make_two_commands_collide() {
    let spaced = rule_key(&process("/work/a", &["node", "run dev"]), "default").unwrap();
    let split = rule_key(&process("/work/a", &["node", "run", "dev"]), "default").unwrap();
    assert_ne!(spaced, split);
    let rules = BTreeSet::from([spaced]);
    assert!(is_ignored(
        &rules,
        &process("/work/a", &["node", "run dev"]),
        "default"
    ));
    assert!(!is_ignored(
        &rules,
        &process("/work/a", &["node", "run", "dev"]),
        "default"
    ));
}

// covers: UXR-5.1
#[test]
fn another_folder_command_port_argument_or_context_is_never_hidden_by_accident() {
    let base = process("/work/a", &["php", "spark", "serve", "--port", "8080"]);
    let rules = BTreeSet::from([rule_key(&base, "default").unwrap()]);
    assert!(is_ignored(&rules, &base, "default"));
    for other in [
        process("/work/b", &["php", "spark", "serve", "--port", "8080"]),
        process("/work/a", &["php", "spark", "serve", "--port", "8081"]),
        process("/work/a", &["php", "spark", "serve"]),
        process("/work/a/sub", &["php", "spark", "serve", "--port", "8080"]),
    ] {
        assert!(!is_ignored(&rules, &other, "default"), "{other:?}");
    }
    let database = BTreeSet::from([rule_key(&container("mysql"), "colima").unwrap()]);
    assert!(is_ignored(&database, &container("mysql"), "colima"));
    assert!(!is_ignored(&database, &container("mysql"), "default"));
    assert!(!is_ignored(&database, &container("mysql-2"), "colima"));
    // A view with no folder and no container cannot be told apart: no rule.
    assert_eq!(rule_key(&process("/", &[]), "default"), None);
}

// covers: UXR-5.1
#[test]
fn rules_from_earlier_builds_keep_matching_exactly_and_are_described() {
    let view = process("/work/a", &["node", "server.js", "--port", "8080"]);
    let old_process = "process:/work/a\u{1f}node\u{1f}node server.js --port 8080".to_owned();
    let old_container = "container:mysql".to_owned();
    let rules = BTreeSet::from([old_process.clone(), old_container.clone()]);
    assert!(is_ignored(&rules, &view, "default"));
    assert!(is_ignored(&rules, &container("mysql"), "anything"));
    assert!(!is_ignored(
        &rules,
        &process("/work/a", &["node", "server.js", "--port", "9"]),
        "default"
    ));
    let described = describe(&old_process, 5);
    assert!(described.legacy && described.kind == IgnoreKind::Process);
    assert_eq!(described.folder.as_deref(), Some("/work/a"));
    assert_eq!(described.command, ["node", "server.js", "--port", "8080"]);
    let described = describe(&old_container, 6);
    assert!(described.legacy && described.context.is_none());
    assert_eq!(described.container.as_deref(), Some("mysql"));
}

// covers: UXR-5.1
#[test]
fn what_the_person_is_shown_is_what_gets_stored_and_unreadable_keys_stay_listed() {
    let view = process("/work/a", &["php", "-S", "localhost:8000"]);
    let key = rule_key(&view, "default").unwrap();
    let rule = describe(&key, 0);
    assert_eq!(rule.kind, IgnoreKind::Process);
    assert_eq!(rule.folder.as_deref(), Some("/work/a"));
    assert_eq!(rule.command, ["php", "-S", "localhost:8000"]);
    assert!(!rule.legacy);
    assert_eq!(rule.id, rule_id(&key));
    assert_eq!(rule.id.len(), 32);
    let box_rule = describe(&rule_key(&container("db"), "colima").unwrap(), 0);
    assert_eq!(box_rule.context.as_deref(), Some("colima"));
    let unknown = describe("v9:something-new", 3);
    assert_eq!(unknown.kind, IgnoreKind::Unknown);
    assert_eq!(unknown.id, rule_id("v9:something-new"));
}
