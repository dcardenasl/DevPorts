use std::path::Path;

use devports_proto::views::{InstanceRole, InstanceState, InstanceView};
use tempfile::TempDir;

use super::*;

fn process(cwd: &Path, port: u16, argv: &[&str]) -> InstanceView {
    InstanceView {
        instance_ref: "e5f6".to_owned(),
        project: None,
        service: None,
        role: InstanceRole::Unregistered,
        state: InstanceState::Healthy,
        kind: "app".to_owned(),
        port: Some(port),
        url: Some(format!("http://localhost:{port}")),
        pid: Some(900),
        target_identity: Some("process-fingerprint".to_owned()),
        ports: vec![port],
        container: None,
        branch: None,
        worktree: None,
        cwd: Some(cwd.display().to_string()),
        started_at: None,
        expires_at: None,
        reason: None,
        registered: false,
        launcher_chain: Vec::new(),
        duplicate_of: None,
        stale: None,
        shared: false,
        compose: None,
        argv: argv.iter().map(|part| (*part).to_owned()).collect(),
        image: None,
        ignored: false,
    }
}

const MANIFEST: &str = "\
# The shop, as the team runs it.
version: 1
project: shop
services:
  # the main API
  api:
    cwd: ./api
    port: 8080
    start: php spark serve --port ${PORT}   # keep the flag

# trailing note
";

fn project() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join("api")).unwrap();
    std::fs::create_dir_all(root.join("admin")).unwrap();
    std::fs::write(root.join(MANIFEST_FILE), MANIFEST).unwrap();
    (dir, root)
}

// covers: I-15
#[test]
fn adding_a_service_keeps_every_comment_and_the_layout() {
    let (_dir, root) = project();
    let view = process(
        &root.join("admin"),
        8081,
        &["php", "spark", "serve", "--port", "8081"],
    );

    let applied = add_service(&root, &view, None).unwrap();

    assert_eq!(applied.project, "shop");
    assert_eq!(applied.service, "admin");
    let text = std::fs::read_to_string(root.join(MANIFEST_FILE)).unwrap();
    for kept in [
        "# The shop, as the team runs it.",
        "  # the main API",
        "start: php spark serve --port ${PORT}   # keep the flag",
        "# trailing note",
    ] {
        assert!(text.contains(kept), "lost {kept:?} in:\n{text}");
    }
    // The new service sits with the others, before the trailing note.
    let admin = text.find("  admin:").unwrap();
    assert!(admin > text.find("  api:").unwrap());
    assert!(admin < text.find("# trailing note").unwrap());
    // And the file still validates and has both.
    let loaded = manifest::load(&root.join(MANIFEST_FILE), None).unwrap();
    assert_eq!(loaded.manifest.services.len(), 2);
    assert_eq!(loaded.manifest.services["admin"].port, Some(8081));
}

#[test]
fn a_four_space_manifest_gets_a_four_space_block() {
    let (_dir, root) = project();
    std::fs::write(
        root.join(MANIFEST_FILE),
        "version: 1\nproject: shop\nservices:\n    api:\n        cwd: ./api\n        port: 8080\n",
    )
    .unwrap();
    let view = process(&root.join("admin"), 8081, &["node", "server.js", "8081"]);
    add_service(&root, &view, None).unwrap();
    let text = std::fs::read_to_string(root.join(MANIFEST_FILE)).unwrap();
    assert!(
        text.contains("\n    admin:\n        cwd: ./admin\n"),
        "{text}"
    );
    assert!(manifest::load(&root.join(MANIFEST_FILE), None).is_ok());
}

#[test]
fn an_empty_services_mapping_is_filled_and_a_flow_style_one_is_left_alone() {
    let (_dir, root) = project();
    std::fs::write(
        root.join(MANIFEST_FILE),
        "version: 1\nproject: shop\nservices: {}\n",
    )
    .unwrap();
    let view = process(&root.join("admin"), 8081, &["node", "server.js"]);
    add_service(&root, &view, None).unwrap();
    let loaded = manifest::load(&root.join(MANIFEST_FILE), None).unwrap();
    assert!(loaded.manifest.services.contains_key("admin"));

    let flow = "version: 1\nproject: shop\nservices: {api: {cwd: ./api}}\n";
    std::fs::write(root.join(MANIFEST_FILE), flow).unwrap();
    let other = process(&root.join("admin"), 8082, &["node", "x.js"]);
    let refused = add_service(&root, &other, None).unwrap_err();
    assert!(matches!(refused, ApplyError::Unsupported(_)), "{refused}");
    assert_eq!(
        std::fs::read_to_string(root.join(MANIFEST_FILE)).unwrap(),
        flow
    );
}

#[test]
fn a_name_that_is_taken_gets_a_number() {
    let (_dir, root) = project();
    let view = process(&root.join("api"), 8090, &["php", "spark", "serve"]);
    let applied = add_service(&root, &view, None).unwrap();
    assert_eq!(applied.service, "api-2");
}

#[test]
fn a_process_outside_the_project_is_refused_and_nothing_changes() {
    let (_dir, root) = project();
    let outside = TempDir::new().unwrap();
    let view = process(outside.path(), 9000, &["node", "x.js"]);
    let error = add_service(&root, &view, None).unwrap_err();
    assert!(matches!(error, ApplyError::Unsuitable(_)), "{error}");
    assert_eq!(
        std::fs::read_to_string(root.join(MANIFEST_FILE)).unwrap(),
        MANIFEST
    );
}

#[test]
fn a_broken_manifest_is_never_edited() {
    let (_dir, root) = project();
    std::fs::write(root.join(MANIFEST_FILE), "version: 1\nproject: [oops\n").unwrap();
    let view = process(&root.join("admin"), 8081, &["node", "x.js"]);
    assert!(matches!(
        add_service(&root, &view, None).unwrap_err(),
        ApplyError::Invalid(_)
    ));
}

#[test]
fn a_new_project_is_created_once_and_never_overwritten() {
    let dir = TempDir::new().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let view = process(&root, 3000, &["npm", "run", "dev"]);

    let applied = create_project(
        &root,
        Path::new("/home/nobody"),
        Some("Mi Tienda"),
        &view,
        None,
    )
    .unwrap();
    assert_eq!(applied.project, "mi-tienda");
    let loaded = manifest::load(&root.join(MANIFEST_FILE), None).unwrap();
    assert_eq!(loaded.manifest.project, "mi-tienda");
    assert_eq!(loaded.manifest.services.len(), 1);

    let again = create_project(&root, Path::new("/home/nobody"), None, &view, None).unwrap_err();
    assert!(matches!(again, ApplyError::Exists(_)));
}

#[test]
fn no_temporary_file_is_left_behind() {
    let (_dir, root) = project();
    let view = process(&root.join("admin"), 8081, &["node", "x.js"]);
    add_service(&root, &view, None).unwrap();
    let names: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!names.iter().any(|name| name.contains("tmp")), "{names:?}");
}

fn container(name: &str, port: u16, image: &str) -> InstanceView {
    InstanceView {
        container: Some(name.to_owned()),
        image: Some(image.to_owned()),
        cwd: None,
        argv: Vec::new(),
        ..process(Path::new("/"), port, &[])
    }
}

#[test]
fn a_container_becomes_a_shared_service_and_the_file_is_created_when_missing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config").join("shared.yml");

    let applied = add_shared(&path, &container("mysql", 3306, "mysql:8.0")).unwrap();
    assert_eq!(applied.service, "mysql");
    let (shared, _) = crate::config::SharedServices::load(&path).unwrap();
    let spec = &shared.services["mysql"];
    assert_eq!(spec.docker.as_deref(), Some("mysql"));
    assert_eq!(spec.port, Some(3306));
    assert_eq!(spec.kind, crate::manifest::ServiceKind::Data);

    // A second one is appended, and the same name gets a number.
    add_shared(&path, &container("redis", 6379, "redis:7")).unwrap();
    let again = add_shared(&path, &container("mysql", 3307, "mysql:8.4")).unwrap();
    assert_eq!(again.service, "mysql-2");
    let (shared, _) = crate::config::SharedServices::load(&path).unwrap();
    assert_eq!(shared.services.len(), 3);
}

#[test]
fn only_containers_can_be_shared() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("shared.yml");
    let view = process(dir.path(), 8080, &["node", "x.js"]);
    assert!(matches!(
        add_shared(&path, &view),
        Err(ApplyError::Unsuitable(_))
    ));
    assert!(!path.exists());
}

#[test]
fn a_document_root_is_named_after_its_project() {
    let dir = TempDir::new().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join("public")).unwrap();
    let view = process(
        &root.join("public"),
        8192,
        &["php", "-S", "127.0.0.1:8192", "-t", "public"],
    );
    let applied = create_project(&root, Path::new("/home/nobody"), None, &view, None).unwrap();
    let folder = root.file_name().unwrap().to_string_lossy().to_lowercase();
    assert_eq!(applied.service, super::slug(&folder));
    assert_ne!(applied.service, "public");
}

#[test]
fn a_folder_that_is_the_whole_home_or_the_volume_root_is_not_a_project() {
    let dir = TempDir::new().unwrap();
    let home = std::fs::canonicalize(dir.path()).unwrap();
    let view = process(&home, 3000, &["npm", "run", "dev"]);
    assert!(matches!(
        create_project(&home, &home, None, &view, None),
        Err(ApplyError::Unsuitable(_))
    ));
    assert!(matches!(
        create_project(Path::new("/"), &home, None, &view, None),
        Err(ApplyError::Unsuitable(_))
    ));
    assert!(matches!(
        create_project(Path::new("/Users"), &home, None, &view, None),
        Err(ApplyError::Unsuitable(_))
    ));
    assert!(!home.join(MANIFEST_FILE).exists());
}

// covers: UXR-3.1
#[test]
fn empty_mappings_keep_inline_comments_and_files_without_a_final_newline() {
    let source = "# before\r\nversion: 1\r\nproject: shop\r\nservices: {} # keep this comment";
    let edited = insert_service(source, &["  api:".to_owned(), "    cwd: .".to_owned()]).unwrap();
    assert!(edited.starts_with(
        "# before\r\nversion: 1\r\nproject: shop\r\nservices: # keep this comment\r\n"
    ));
    assert!(!edited.ends_with('\n'));
    assert!(
        manifest::parse(&edited, Path::new("m.yml"))
            .unwrap()
            .services
            .contains_key("api")
    );
}

// covers: UXR-3.1
#[test]
fn proposed_collision_names_stay_within_the_manifest_name_limit() {
    let name = "a".repeat(63);
    let taken = std::collections::BTreeMap::from([(name.clone(), true)]);
    let proposed = unused_name(&name, &taken);
    assert!(manifest::is_valid_name(&proposed));
    assert_ne!(proposed, name);
    assert!(proposed.ends_with("-2"));
}

// covers: UXR-3.1
#[test]
fn comments_inside_an_existing_service_are_not_reassigned_to_the_new_one() {
    let source = "version: 1\nproject: shop\nservices:\n  api:\n    cwd: ./api\n    # comment about the API\n# note after services\n";
    let edited = insert_service(
        source,
        &["  admin:".to_owned(), "    cwd: ./admin".to_owned()],
    )
    .unwrap();
    assert!(edited.find("# comment about the API").unwrap() < edited.find("  admin:").unwrap());
    assert!(edited.find("  admin:").unwrap() < edited.find("# note after services").unwrap());
    assert!(
        manifest::parse(&edited, Path::new("m.yml"))
            .unwrap()
            .services
            .contains_key("admin")
    );
}
