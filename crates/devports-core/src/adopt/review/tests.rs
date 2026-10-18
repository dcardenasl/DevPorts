use super::*;
use tempfile::TempDir;

fn fixture() -> (TempDir, AdoptionContext) {
    let home = TempDir::new().unwrap();
    let root = home.path().join("work/shop");
    std::fs::create_dir_all(root.join("public")).unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/dev\n").unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let view: InstanceView = serde_json::from_value(serde_json::json!({
        "ref":"e5f6", "state":"healthy", "role":"unregistered", "kind":"app",
        "target_identity":"exact-process-generation", "pid":77, "port":8080,
        "cwd":root.join("public"), "argv":["node", "server.js", "--port", "8080"],
        "registered":false,"launcher_chain":[],"shared":false,"ignored":false
    }))
    .unwrap();
    let context = AdoptionContext {
        view,
        projects: vec![("shop".to_owned(), root)],
        shared: BTreeSet::new(),
        home: home.path().to_path_buf(),
        roots: vec![home.path().to_path_buf()],
        max_depth: 3,
    };
    (home, context)
}

fn manifest(context: &AdoptionContext, text: &str) -> PathBuf {
    let path = context.projects[0].1.join(MANIFEST_FILE);
    std::fs::write(&path, text).unwrap();
    path
}

const EXISTING: &str = "# team notes\r\nversion: 1\r\nproject: shop\r\nservices:\r\n  api:\r\n    cwd: ./api # keep this\r\n# trailing note\r\n";

// covers: UXR-3.1
#[test]
fn destinations_explain_outside_roots_and_provisional_git_projects_without_writing() {
    let (_home, mut context) = fixture();
    let outside = TempDir::new().unwrap();
    context.projects.push((
        "outside".to_owned(),
        std::fs::canonicalize(outside.path()).unwrap(),
    ));
    let choices = options(&context);
    let provisional = choices
        .destinations
        .iter()
        .find(|choice| choice.target == AdoptionTarget::Project("shop".to_owned()))
        .unwrap();
    assert!(provisional.eligible);
    assert!(provisional.creates_manifest);
    assert_eq!(provisional.root.as_deref(), context.projects[0].1.to_str());
    let rejected = choices
        .destinations
        .iter()
        .find(|choice| choice.target == AdoptionTarget::Project("outside".to_owned()))
        .unwrap();
    assert!(!rejected.eligible);
    assert!(rejected.reason.as_ref().unwrap().contains("fuera"));
    let prepared = prepare(&context, AdoptionTarget::New(None)).unwrap();
    let preview = prepared.validate(&prepared.defaults()).unwrap();
    assert_eq!(preview.root, context.projects[0].1.display().to_string());
    assert_eq!(
        preview.manifest,
        context.projects[0]
            .1
            .join(MANIFEST_FILE)
            .display()
            .to_string()
    );
    assert_eq!(preview.service.cwd.as_deref(), Some("./public"));
    assert!(
        !Path::new(&preview.manifest).exists(),
        "options/preview are read-only"
    );
    let shared = choices
        .destinations
        .iter()
        .find(|choice| choice.target == AdoptionTarget::Shared)
        .unwrap();
    assert!(!shared.eligible);
    assert!(shared.reason.as_ref().unwrap().contains("contenedores"));
}

// covers: UXR-3.1
#[test]
fn edited_values_are_validated_and_comments_line_endings_and_permissions_survive() {
    let (_home, context) = fixture();
    let path = manifest(&context, EXISTING);
    let prepared = prepare(&context, AdoptionTarget::Project("shop".to_owned())).unwrap();
    let fields = ServiceEdits {
        name: "backend".to_owned(),
        start: Some("node 'a path/server.js' --message '$SECRET'".to_owned()),
    };
    let preview = prepared.validate(&fields).unwrap();
    assert_eq!(preview.service.name, "backend");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        EXISTING,
        "validation writes nothing"
    );
    assert!(
        prepared
            .validate(&ServiceEdits {
                name: "api".to_owned(),
                ..fields.clone()
            })
            .is_err()
    );
    assert!(
        matches!(prepared.validate(&ServiceEdits { name: "Invalid Name".to_owned(), ..fields.clone() }), Err(AdoptionError::InvalidField { field, .. }) if field == "name")
    );
    assert!(
        matches!(prepared.validate(&ServiceEdits { start: Some("node script.js\nsecond-command".to_owned()), ..fields.clone() }), Err(AdoptionError::InvalidField { field, .. }) if field == "start")
    );
    let applied = prepared.commit(&fields).unwrap();
    assert_eq!(applied.service, "backend");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(EXISTING.split("# trailing note").next().unwrap()));
    assert!(text.ends_with("# trailing note\r\n"));
    assert!(!text.replace("\r\n", "").contains('\n'));
    let parsed = manifest::load(&path, None).unwrap();
    assert_eq!(parsed.manifest.services["backend"].start, fields.start);
    assert_eq!(parsed.manifest.services.len(), 2);
}

// covers: UXR-3.1
#[test]
fn unsupported_yaml_and_permissions_are_ineligible_before_any_save() {
    let (_home, context) = fixture();
    let path = manifest(
        &context,
        "version: 1\nproject: shop\nservices: {api: {cwd: ./api}}\n",
    );
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        prepare(&context, AdoptionTarget::Project("shop".to_owned())),
        Err(AdoptionError::Unsupported { .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            !options(&context)
                .destinations
                .iter()
                .find(|choice| choice.target == AdoptionTarget::Project("shop".to_owned()))
                .unwrap()
                .eligible
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

// covers: UXR-3.1
#[test]
fn a_changed_external_document_is_preserved_and_the_frozen_preview_is_not_rebased() {
    let (_home, context) = fixture();
    let path = manifest(&context, EXISTING);
    let prepared = prepare(&context, AdoptionTarget::Project("shop".to_owned())).unwrap();
    let fields = prepared.defaults();
    let foreign = format!("{EXISTING}# added by another editor\r\n");
    std::fs::write(&path, &foreign).unwrap();
    assert!(matches!(
        prepared.commit(&fields),
        Err(AdoptionError::DocumentChanged { .. })
    ));
    assert!(matches!(
        prepared.commit(&fields),
        Err(AdoptionError::DocumentChanged { .. })
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), foreign);
    let (_other_home, another) = fixture();
    let absent = prepare(&another, AdoptionTarget::New(None)).unwrap();
    std::fs::write(absent.file.path(), "external creation").unwrap();
    assert!(matches!(
        absent.commit(&absent.defaults()),
        Err(AdoptionError::DocumentChanged { .. })
    ));
    assert_eq!(
        std::fs::read_to_string(absent.file.path()).unwrap(),
        "external creation"
    );
}

// covers: UXR-3.1
#[test]
fn duplicate_inflight_and_lost_response_replays_have_one_service_without_a_suffix() {
    let (_home, context) = fixture();
    let prepared = prepare(&context, AdoptionTarget::Project("shop".to_owned())).unwrap();
    let mut book = AdoptionBook::default();
    let preview = book.remember(prepared).unwrap();
    let fields = ServiceEdits {
        name: "backend".to_owned(),
        start: preview.service.start.clone(),
    };
    let BeginApply::Work(work) = book
        .begin(&preview.id, &fields, Some(&context.view), &context.projects)
        .unwrap()
    else {
        panic!("first save schedules exactly one worker")
    };
    assert!(matches!(
        book.begin(&preview.id, &fields, Some(&context.view), &context.projects)
            .unwrap(),
        BeginApply::Running
    ));
    assert!(matches!(
        book.begin(
            &preview.id,
            &ServiceEdits {
                name: "other".to_owned(),
                ..fields.clone()
            },
            Some(&context.view),
            &context.projects
        ),
        Err(AdoptionError::RequestChanged)
    ));
    let result = work.commit(&fields);
    book.finish(&preview.id, &result);
    let saved = result.unwrap();
    // Deliberately discard the first caller's response; no current target is needed for its receipt.
    let BeginApply::Applied(replayed) = book.begin(&preview.id, &fields, None, &[]).unwrap() else {
        panic!("completed replay returns its receipt")
    };
    assert_eq!(replayed, saved);
    let parsed = manifest::load(&saved.manifest, None).unwrap();
    assert_eq!(parsed.manifest.services.len(), 1);
    assert!(parsed.manifest.services.contains_key("backend"));
    assert!(matches!(
        book.read(&preview.id),
        Err(AdoptionError::RequestChanged)
    ));
}

// covers: UXR-3.1
#[test]
fn changed_targets_and_expired_previews_cannot_be_recalculated_into_another_write() {
    let (_home, context) = fixture();
    let mut book = AdoptionBook::default();
    let first = book
        .remember(prepare(&context, AdoptionTarget::New(None)).unwrap())
        .unwrap();
    let fields = ServiceEdits {
        name: first.service.name,
        start: first.service.start,
    };
    let mut changed = context.view.clone();
    changed.target_identity = Some("replacement-process".to_owned());
    assert!(matches!(
        book.begin(&first.id, &fields, Some(&changed), &context.projects),
        Err(AdoptionError::TargetChanged)
    ));
    for _ in 0..8 {
        book.remember(prepare(&context, AdoptionTarget::New(None)).unwrap())
            .unwrap();
    }
    assert!(matches!(
        book.read(&first.id),
        Err(AdoptionError::PreviewExpired)
    ));
    assert!(!context.projects[0].1.join(MANIFEST_FILE).exists());
}

// covers: UXR-3.1
#[test]
fn containers_can_be_shared_without_an_existing_compose_folder() {
    let (_home, mut context) = fixture();
    context.view.container = Some("mysql".to_owned());
    context.view.cwd = Some("/missing/compose/folder".to_owned());
    context.view.image = Some("mysql:8".to_owned());
    context.view.argv.clear();
    let prepared = prepare(&context, AdoptionTarget::Shared).unwrap();
    let preview = prepared.validate(&prepared.defaults()).unwrap();
    assert!(preview.creates_manifest);
    assert!(preview.service.is_data);
    assert!(!Path::new(&preview.manifest).exists());
    assert!(
        !Path::new(&preview.manifest).parent().unwrap().exists(),
        "preview does not create the config folder"
    );
}

// covers: UXR-3.1
#[test]
fn active_reviews_are_bounded_and_never_evicted_while_their_save_runs() {
    let (_home, context) = fixture();
    let mut book = AdoptionBook::default();
    for _ in 0..8 {
        let preview = book
            .remember(prepare(&context, AdoptionTarget::New(None)).unwrap())
            .unwrap();
        let fields = ServiceEdits {
            name: preview.service.name,
            start: preview.service.start,
        };
        assert!(matches!(
            book.begin(&preview.id, &fields, Some(&context.view), &context.projects),
            Ok(BeginApply::Work(_))
        ));
    }
    assert!(matches!(
        book.remember(prepare(&context, AdoptionTarget::New(None)).unwrap()),
        Err(AdoptionError::AtCapacity)
    ));
}

// covers: UXR-3.1
#[cfg(unix)]
#[test]
fn symlinked_folders_do_not_make_an_external_host_eligible_for_a_project() {
    let (_home, mut context) = fixture();
    let outside = TempDir::new().unwrap();
    let link = context.projects[0].1.join("linked-server");
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    context.view.cwd = Some(link.display().to_string());
    let choices = options(&context);
    assert!(
        !choices
            .destinations
            .iter()
            .find(|choice| choice.target == AdoptionTarget::Project("shop".to_owned()))
            .unwrap()
            .eligible
    );
    assert!(!context.projects[0].1.join(MANIFEST_FILE).exists());
}

// covers: UXR-3.1
#[test]
fn field_errors_and_conflicts_have_stable_wire_codes() {
    let error = AdoptionError::DocumentChanged {
        path: "/work/.devports.yml".to_owned(),
    };
    let value = serde_json::to_value(error).unwrap();
    assert_eq!(value["code"], "documentChanged");
    assert_eq!(value["path"], "/work/.devports.yml");
    assert_eq!(
        serde_json::to_value(invalid("name", "invalid name")).unwrap()["field"],
        "name"
    );
}

// covers: UXR-3.1
#[test]
fn an_unavailable_command_is_editable_and_warned_instead_of_rejecting_the_root() {
    let (_home, mut context) = fixture();
    context.view.argv.clear();
    let prepared = prepare(&context, AdoptionTarget::New(None)).unwrap();
    let preview = prepared.validate(&prepared.defaults()).unwrap();
    assert!(preview.service.start.is_none());
    assert!(!preview.warnings.is_empty());
    let edited = prepared
        .validate(&ServiceEdits {
            name: "backend".to_owned(),
            start: Some("node server.js --port ${PORT}".to_owned()),
        })
        .unwrap();
    assert_eq!(edited.service.name, "backend");
    assert!(!Path::new(&edited.manifest).exists());
}

// covers: UXR-3.1
#[test]
fn creating_a_home_project_is_refused_but_an_explicit_existing_manifest_stays_editable() {
    let (_home, mut context) = fixture();
    let home = std::fs::canonicalize(&context.home).unwrap();
    context.projects = vec![("explicit-home".to_owned(), home.clone())];
    let target = AdoptionTarget::Project("explicit-home".to_owned());
    assert!(matches!(
        prepare(&context, target.clone()),
        Err(AdoptionError::Unsuitable { .. })
    ));
    std::fs::write(
        home.join(MANIFEST_FILE),
        "version: 1\nproject: explicit-home\nservices: {}\n",
    )
    .unwrap();
    let prepared = prepare(&context, target).unwrap();
    assert!(
        !prepared
            .validate(&prepared.defaults())
            .unwrap()
            .creates_manifest
    );
}

// covers: UXR-3.2
#[test]
fn a_root_the_discovery_cannot_reach_is_refused_so_the_saved_card_can_appear() {
    let (_home, mut context) = fixture();
    let other = TempDir::new().unwrap();
    context.roots = vec![other.path().to_path_buf()];
    assert!(matches!(
        prepare(&context, AdoptionTarget::Project("shop".to_owned())),
        Err(AdoptionError::Unsuitable { .. })
    ));
    context.roots = vec![context.home.clone()];
    context.max_depth = 1;
    assert!(
        matches!(
            prepare(&context, AdoptionTarget::Project("shop".to_owned())),
            Err(AdoptionError::Unsuitable { .. })
        ),
        "work/shop is two levels below the configured root"
    );
    context.max_depth = 2;
    assert!(prepare(&context, AdoptionTarget::Project("shop".to_owned())).is_ok());
}
