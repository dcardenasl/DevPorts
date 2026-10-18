use super::*;
use tempfile::TempDir;

struct Home {
    dir: TempDir,
    work: PathBuf,
}

fn home(config: Option<&str>) -> Home {
    let dir = TempDir::new().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = base.join("work");
    std::fs::create_dir_all(work.join("shop")).unwrap();
    std::fs::write(
        work.join("shop/.devports.yml"),
        "version: 1\nproject: shop\nservices:\n  api:\n    cwd: .\n",
    )
    .unwrap();
    if let Some(config) = config {
        let path = base.join(".config/devports/config.yml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, config.replace("{work}", &work.display().to_string())).unwrap();
    }
    Home { dir, work }
}

impl Home {
    fn path(&self) -> PathBuf {
        std::fs::canonicalize(self.dir.path()).unwrap()
    }
    fn config(&self) -> String {
        std::fs::read_to_string(self.path().join(".config/devports/config.yml")).unwrap()
    }
    fn base(&self) -> String {
        read(&self.path()).unwrap().base
    }
    fn folder(&self, name: &str, project: Option<&str>) -> String {
        let folder = self.path().join(name);
        std::fs::create_dir_all(&folder).unwrap();
        if let Some(project) = project {
            std::fs::write(
                folder.join(".devports.yml"),
                format!("version: 1\nproject: {project}\nservices: {{}}\n"),
            )
            .unwrap();
        }
        folder.display().to_string()
    }
}

const FULL: &str = "# my settings\nversion: 1\nroots: [\"{work}\"]   # where I work\nmax_depth: 2\nlease:\n  default: 45m # long\n";

// covers: UXR-6.1
#[test]
fn every_supported_form_is_edited_in_place_keeping_everything_else() {
    let two = vec!["/a/one".to_owned(), "/a/two".to_owned()];
    let flow = edit_roots("version: 1\nroots: ['/x/y']  # why\nmax_depth: 2\n", &two).unwrap();
    assert_eq!(
        flow,
        "version: 1\nroots: [\"/a/one\", \"/a/two\"]  # why\nmax_depth: 2\n"
    );
    let block = edit_roots(
        "version: 1\nroots:\n  # mine\n  - ~/developer\n  - \"/b\" # old\nmax_depth: 2\n",
        &two,
    )
    .unwrap();
    assert_eq!(
        block,
        "version: 1\nroots:\n  # mine\n  - \"/a/one\"\n  - \"/a/two\"\nmax_depth: 2\n"
    );
    let absent = edit_roots("# top\nversion: 1\nmax_depth: 2\n", &two).unwrap();
    assert_eq!(
        absent,
        "# top\nversion: 1\nroots: [\"/a/one\", \"/a/two\"]\nmax_depth: 2\n"
    );
    assert_eq!(
        edit_roots("", &two).unwrap(),
        "version: 1\nroots: [\"/a/one\", \"/a/two\"]\n"
    );
    let crlf = edit_roots("version: 1\r\nroots: [\"/x\"]\r\nmax_depth: 2", &two).unwrap();
    assert_eq!(
        crlf,
        "version: 1\r\nroots: [\"/a/one\", \"/a/two\"]\r\nmax_depth: 2"
    );
}

// covers: UXR-6.1
#[test]
fn forms_that_cannot_be_edited_safely_are_refused() {
    for roots in [
        "roots: ~\n",
        "roots: &r [\"/a\"]\n",
        "roots: [\"/a\",\n  \"/b\"]\n",
        "roots: /a\n",
        "roots:\n  - {path: /a}\n",
        "roots:\n  - /a\n  extra: 1\n",
        "roots: [\"/a\\\"b\"]\n",
    ] {
        let source = format!("version: 1\n{roots}");
        assert!(
            matches!(
                edit_roots(&source, &["/z".to_owned()]),
                Err(SettingsError::Unsupported { .. })
            ),
            "{roots:?}"
        );
    }
}

// covers: UXR-6.1
#[test]
fn a_new_folder_is_saved_keeping_comments_other_keys_and_the_old_folder() {
    let h = home(Some(FULL));
    let extra = h.folder("more", Some("blog"));
    let seen = read(&h.path()).unwrap();
    assert!(seen.editable && !seen.from_defaults);
    assert_eq!(seen.roots.len(), 1);
    assert_eq!(seen.roots[0].projects, 1);
    let list = vec![h.work.display().to_string(), extra.clone()];
    let shown = preview(&h.path(), &list).unwrap();
    assert_eq!(shown.added, ["blog"]);
    assert!(shown.removed.is_empty() && !shown.unchanged);
    assert_eq!(
        h.config(),
        FULL.replace("{work}", &h.work.display().to_string()),
        "preview writes nothing"
    );
    assert_eq!(
        apply(&h.path(), &list, &seen.base),
        Ok(SettingsOutcome::Saved)
    );
    let text = h.config();
    assert!(text.starts_with("# my settings\nversion: 1\nroots: [\""));
    assert!(text.contains("# where I work") && text.contains("max_depth: 2"));
    assert!(text.contains("  default: 45m # long"));
    let (config, _) = Config::parse(&text, &h.path().join("c.yml"), &h.path()).unwrap();
    assert_eq!(config.roots, [h.work.clone(), PathBuf::from(extra)]);
    assert_eq!(config.max_depth, 2);
    // The same list again: nothing to do, whatever version the person holds.
    assert_eq!(
        apply(&h.path(), &list, "stale"),
        Ok(SettingsOutcome::Unchanged)
    );
}

// covers: UXR-6.1
#[test]
fn removing_a_folder_only_stops_looking_and_says_which_projects_leave_the_list() {
    let h = home(Some(FULL));
    let other = h.folder("elsewhere", Some("blog"));
    let both = vec![h.work.display().to_string(), other.clone()];
    let base = h.base();
    apply(&h.path(), &both, &base).unwrap();
    let shown = preview(&h.path(), std::slice::from_ref(&other)).unwrap();
    assert_eq!(shown.removed, ["shop"]);
    assert!(shown.added.is_empty());
    assert_eq!(
        apply(&h.path(), &[other], &h.base()),
        Ok(SettingsOutcome::Saved)
    );
    assert!(
        h.work.join("shop/.devports.yml").exists(),
        "no file is deleted"
    );
}

// covers: UXR-6.1
#[test]
fn unacceptable_folders_are_reported_by_position_and_nothing_is_written() {
    let h = home(Some(FULL));
    let before = h.config();
    let good = h.work.display().to_string();
    let list = vec![
        good.clone(),
        "relative/path".to_owned(),
        format!("{good}/"),
        h.path().join("ghost").display().to_string(),
        "~".to_owned(),
        "/".to_owned(),
        "  ".to_owned(),
        "/tmp/\"quoted\"".to_owned(),
    ];
    let Err(SettingsError::InvalidRoots { problems }) = apply(&h.path(), &list, &h.base()) else {
        panic!("expected invalid roots");
    };
    let indexes: Vec<u32> = problems.iter().map(|p| p.index).collect();
    assert_eq!(indexes, [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(problems[1].kind, RootProblemKind::Duplicate);
    assert_eq!(problems[2].kind, RootProblemKind::Missing);
    assert_eq!(problems[3].kind, RootProblemKind::TooBroad);
    assert_eq!(h.config(), before);
    assert!(matches!(
        apply(&h.path(), &[], &h.base()),
        Err(SettingsError::InvalidList { .. })
    ));
    let many: Vec<String> = (0..=MAX_ROOTS).map(|n| format!("/x/y{n}")).collect();
    assert!(matches!(
        apply(&h.path(), &many, &h.base()),
        Err(SettingsError::InvalidList { .. })
    ));
    assert_eq!(h.config(), before);
}

// covers: UXR-6.1
#[test]
fn a_vanished_folder_already_in_the_list_does_not_block_other_changes() {
    let gone = "/Volumes/not-mounted/work";
    let h = home(Some(&format!(
        "version: 1\nroots: [\"{gone}\", \"{{work}}\"]\n"
    )));
    let seen = read(&h.path()).unwrap();
    assert!(!seen.roots[0].exists && seen.roots[1].exists);
    let extra = h.folder("more", None);
    let list = vec![gone.to_owned(), h.work.display().to_string(), extra];
    assert_eq!(
        apply(&h.path(), &list, &seen.base),
        Ok(SettingsOutcome::Saved)
    );
    assert!(
        h.config().contains(gone),
        "the unmounted folder stays configured"
    );
}

// covers: UXR-6.1
#[test]
fn an_external_edit_is_kept_and_needs_another_review() {
    let h = home(Some(FULL));
    let reviewed = h.base();
    let ours = format!("{}# edited by a person\n", h.config());
    std::fs::write(h.path().join(".config/devports/config.yml"), &ours).unwrap();
    let extra = h.folder("more", None);
    let list = vec![h.work.display().to_string(), extra];
    assert!(matches!(
        apply(&h.path(), &list, &reviewed),
        Err(SettingsError::DocumentChanged { .. })
    ));
    assert_eq!(h.config(), ours);
    assert_eq!(
        apply(&h.path(), &list, &h.base()),
        Ok(SettingsOutcome::Saved)
    );
    assert!(h.config().contains("# edited by a person"));
}

// covers: UXR-6.1
#[test]
fn a_broken_unsupported_or_read_only_configuration_is_explained_and_never_written() {
    use std::os::unix::fs::PermissionsExt;
    let h = home(Some("version: 1\nroots: [oops\n"));
    let seen = read(&h.path()).unwrap();
    assert!(!seen.editable && matches!(seen.reason, Some(Ineligibility::ConfigInvalid { .. })));
    let extra = h.folder("more", None);
    assert!(matches!(
        apply(&h.path(), std::slice::from_ref(&extra), &seen.base),
        Err(SettingsError::Ineligible { .. })
    ));
    let anchored = home(Some("version: 1\nroots: &r [\"{work}\"]\n"));
    let seen = read(&anchored.path()).unwrap();
    assert!(!seen.editable && seen.reason.is_some());
    assert!(!seen.roots.is_empty(), "what is in force is still shown");
    let locked = home(Some(FULL));
    let path = locked.path().join(".config/devports/config.yml");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let before = locked.config();
    let extra = locked.folder("more", None);
    let list = vec![locked.work.display().to_string(), extra];
    assert!(matches!(
        apply(&locked.path(), &list, &locked.base()),
        Err(SettingsError::Ineligible { .. })
    ));
    assert_eq!(locked.config(), before);
}

// covers: UXR-6.1
#[test]
fn without_a_config_file_the_defaults_are_shown_and_a_save_creates_it() {
    let h = home(None);
    let seen = read(&h.path()).unwrap();
    assert!(seen.from_defaults && seen.editable);
    assert_eq!(
        seen.roots[0].configured,
        h.path().join("developer").display().to_string()
    );
    assert!(!seen.roots[0].exists);
    let extra = h.folder("more", Some("blog"));
    assert_eq!(
        apply(&h.path(), std::slice::from_ref(&extra), &seen.base),
        Ok(SettingsOutcome::Saved)
    );
    assert_eq!(h.config(), format!("version: 1\nroots: [\"{extra}\"]\n"));
    let after = read(&h.path()).unwrap();
    assert!(!after.from_defaults);
    assert_eq!(after.roots[0].projects, 1);
}
