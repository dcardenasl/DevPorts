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

    let applied = create_project(&root, Some("Mi Tienda"), &view, None).unwrap();
    assert_eq!(applied.project, "mi-tienda");
    let loaded = manifest::load(&root.join(MANIFEST_FILE), None).unwrap();
    assert_eq!(loaded.manifest.project, "mi-tienda");
    assert_eq!(loaded.manifest.services.len(), 1);

    let again = create_project(&root, None, &view, None).unwrap_err();
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
