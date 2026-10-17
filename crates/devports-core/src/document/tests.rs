use super::*;
use tempfile::TempDir;

// covers: UXR-3.1
#[test]
fn captured_versions_preserve_external_edits_and_absence() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.yml");
    let absent = FileSnapshot::capture(&path).unwrap();
    assert!(!path.exists());
    std::fs::write(&path, "external").unwrap();
    assert!(matches!(absent.commit("ours"), Err(EditError::Conflict(_))));
    let captured = FileSnapshot::capture(&path).unwrap();
    std::fs::write(&path, "changed").unwrap();
    assert!(matches!(
        captured.commit("ours"),
        Err(EditError::Conflict(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "changed");
}

// covers: UXR-3.1
#[test]
fn successful_edits_are_complete_and_leave_no_temporary_files() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nested/manifest.yml");
    let snapshot = FileSnapshot::capture(&path).unwrap();
    assert!(!path.parent().unwrap().exists(), "preview creates nothing");
    snapshot.commit("first").unwrap();
    let next = FileSnapshot::capture(&path).unwrap();
    next.commit("second").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1
    );
}

// covers: UXR-3.1
#[cfg(unix)]
#[test]
fn permissions_and_links_are_not_silently_overwritten() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.yml");
    std::fs::write(&path, "original").unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(0o640)).unwrap();
    FileSnapshot::capture(&path)
        .unwrap()
        .commit("edited")
        .unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    std::fs::set_permissions(&path, Permissions::from_mode(0o440)).unwrap();
    assert!(matches!(
        FileSnapshot::capture(&path),
        Err(EditError::ReadOnly(_))
    ));
    std::fs::set_permissions(&path, Permissions::from_mode(0o640)).unwrap();
    let link = dir.path().join("link.yml");
    symlink(&path, &link).unwrap();
    assert!(matches!(
        FileSnapshot::capture(&link),
        Err(EditError::Unsupported(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "edited");
}

// covers: UXR-3.1
#[test]
fn cooperating_writers_cannot_overwrite_each_other_from_the_same_version() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.yml");
    std::fs::write(&path, "base").unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|text| {
            let snapshot = FileSnapshot::capture(&path).unwrap();
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                snapshot.commit(text)
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(EditError::Conflict(_))))
            .count(),
        1
    );
    assert!(["first", "second"].contains(&std::fs::read_to_string(path).unwrap().as_str()));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

// covers: UXR-3.1
#[test]
fn replacing_a_project_directory_does_not_redirect_a_missing_file_edit() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let path = root.join("manifest.yml");
    let snapshot = FileSnapshot::capture(&path).unwrap();
    std::fs::rename(&root, dir.path().join("original-project")).unwrap();
    std::fs::create_dir(&root).unwrap();
    assert!(matches!(
        snapshot.commit("must not be installed in another folder"),
        Err(EditError::Conflict(_))
    ));
    assert!(!path.exists());
}

// covers: UXR-3.1
#[cfg(target_os = "macos")]
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "native metadata checks use only a disposable fixture"
)]
fn native_metadata_and_acl_write_restrictions_survive_configuration_edits() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.yml");
    std::fs::write(&path, "long original contents").unwrap();
    let attribute = "com.devports.test-edit";
    assert!(
        std::process::Command::new("/usr/bin/xattr")
            .args(["-w", attribute, "retained"])
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone deny execute"])
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    FileSnapshot::capture(&path)
        .unwrap()
        .commit("short")
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "short",
        "copied original bytes cannot remain as a suffix"
    );
    let attribute_value = std::process::Command::new("/usr/bin/xattr")
        .args(["-p", attribute])
        .arg(&path)
        .output()
        .unwrap();
    assert!(attribute_value.status.success());
    assert_eq!(
        String::from_utf8(attribute_value.stdout).unwrap().trim(),
        "retained"
    );
    let acl = std::process::Command::new("/bin/ls")
        .arg("-le")
        .arg(&path)
        .output()
        .unwrap();
    assert!(acl.status.success());
    assert!(
        String::from_utf8(acl.stdout)
            .unwrap()
            .contains("deny execute")
    );
    assert!(
        std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone deny write"])
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert!(matches!(
        FileSnapshot::capture(&path),
        Err(EditError::ReadOnly(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "short");
    assert!(
        std::process::Command::new("/bin/chmod")
            .arg("-N")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
}
