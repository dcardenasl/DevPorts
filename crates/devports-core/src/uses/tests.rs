use super::*;
use tempfile::TempDir;

fn shared() -> BTreeSet<String> {
    ["mysql", "redis"].map(str::to_owned).into()
}

fn project(home: &TempDir, name: &str, uses_block: &str) -> (String, PathBuf) {
    let root = std::fs::canonicalize(home.path()).unwrap().join(name);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(MANIFEST_FILE),
        format!("# {name}\nversion: 1\nproject: {name}\n{uses_block}services:\n  api:\n    cwd: .\n    port: 8080 # keep\n"),
    )
    .unwrap();
    (name.to_owned(), root)
}

fn text(project: &(String, PathBuf)) -> String {
    std::fs::read_to_string(project.1.join(MANIFEST_FILE)).unwrap()
}

fn base(project: &(String, PathBuf)) -> String {
    token(&text(project))
}

fn ask(project: &(String, PathBuf), include: bool) -> UseRequest {
    UseRequest {
        project: project.0.clone(),
        include,
        base: base(project),
    }
}

fn run(projects: &[(String, PathBuf)], requests: &[UseRequest]) -> Vec<UseReport> {
    apply("mysql", &shared(), projects, requests)
}

fn declared(project: &(String, PathBuf)) -> Vec<String> {
    manifest::parse(&text(project), &project.1.join(MANIFEST_FILE))
        .unwrap()
        .uses
}

// covers: UXR-4.1
#[test]
fn every_supported_form_is_edited_in_place_and_keeps_comments_and_endings() {
    let cases = [
        ("", true, "project: p\nuses: [mysql]\nservices"),
        (
            "uses: [redis] # cache\n",
            true,
            "uses: [redis, mysql] # cache\n",
        ),
        (
            "uses: [mysql, redis]  # both\n",
            false,
            "uses: [redis]  # both\n",
        ),
        ("uses: [mysql]\n", false, "uses: []\n"),
        ("uses: []\n", true, "uses: [mysql]\n"),
        ("uses:\n  - redis\n", true, "uses:\n  - redis\n  - mysql\n"),
        (
            "uses:\n    - mysql\n    - redis # c\n",
            false,
            "uses:\n    - redis # c\n",
        ),
        ("uses: # none\n  - mysql\n", false, "uses: [] # none\n"),
    ];
    for (index, (before, include, expected)) in cases.into_iter().enumerate() {
        let source = format!("version: 1\nproject: p\n{before}services:\n  api: {{}}\n");
        let edited = edit_uses(&source, "mysql", include).unwrap();
        assert!(
            edited.contains(expected),
            "case {index}: {edited:?} lacks {expected:?}"
        );
        assert!(edited.contains("services:\n  api: {}\n"), "case {index}");
    }
    let crlf = "version: 1\r\nproject: p\r\nuses: [redis]\r\nservices: {}\r\n";
    assert_eq!(
        edit_uses(crlf, "mysql", true).unwrap(),
        "version: 1\r\nproject: p\r\nuses: [redis, mysql]\r\nservices: {}\r\n"
    );
    let bare = "version: 1\nproject: p\nuses:\n  - redis";
    assert_eq!(
        edit_uses(bare, "mysql", true).unwrap(),
        "version: 1\nproject: p\nuses:\n  - redis\n  - mysql\n"
    );
}

// covers: UXR-4.1
#[test]
fn forms_that_cannot_be_edited_safely_are_refused_without_a_rewrite() {
    for uses in [
        "uses: ~\n",
        "uses: &shared [mysql]\n",
        "uses: [\"mysql\"]\n",
        "uses: [mysql,\n  redis]\n",
        "uses: mysql\n",
        "uses:\n  - mysql\n  extra: 1\n",
        "uses:\n  - {name: mysql}\n",
    ] {
        let source = format!("version: 1\nproject: p\n{uses}services: {{}}\n");
        assert!(
            matches!(
                edit_uses(&source, "redis", true),
                Err(UsesError::Unsupported { .. })
            ),
            "{uses:?}"
        );
    }
}

// covers: UXR-4.1
#[test]
fn two_manifests_are_updated_together_and_a_repeat_changes_nothing() {
    let home = TempDir::new().unwrap();
    let shop = project(&home, "shop", "");
    let blog = project(&home, "blog", "uses: [redis]\n");
    let projects = vec![shop.clone(), blog.clone()];
    let requests = [ask(&shop, true), ask(&blog, true)];
    let reports = run(&projects, &requests);
    assert!(reports.iter().all(|r| r.result == Ok(UseOutcome::Saved)));
    assert_eq!(declared(&shop), ["mysql"]);
    assert_eq!(declared(&blog), ["redis", "mysql"]);
    assert!(text(&shop).contains("# shop\n") && text(&shop).contains("port: 8080 # keep"));
    // The same confirmed request again: nothing is declared twice or rewritten.
    let before = (text(&shop), text(&blog));
    let again = run(&projects, &requests);
    assert!(again.iter().all(|r| r.result == Ok(UseOutcome::Unchanged)));
    assert_eq!((text(&shop), text(&blog)), before);
    // Removing keeps the other declarations; an empty selection is valid.
    let removal = run(&projects, &[ask(&blog, false)]);
    assert_eq!(removal[0].result, Ok(UseOutcome::Saved));
    assert_eq!(declared(&blog), ["redis"]);
    assert!(run(&projects, &[]).is_empty());
}

// covers: UXR-4.1
#[test]
fn a_known_validation_error_writes_nothing_anywhere() {
    let home = TempDir::new().unwrap();
    let good = project(&home, "good", "");
    let broken = project(&home, "broken", "");
    std::fs::write(broken.1.join(MANIFEST_FILE), "version: 1\nproject: [oops\n").unwrap();
    let readonly = project(&home, "locked", "");
    let missing = ("ghost".to_owned(), good.1.join("../ghost"));
    let projects = vec![good.clone(), broken.clone(), readonly.clone()];
    let before = text(&good);
    let reports = run(
        &projects,
        &[
            ask(&good, true),
            UseRequest {
                project: broken.0.clone(),
                include: true,
                base: String::new(),
            },
            ask(&readonly, true),
        ],
    );
    assert_eq!(reports[0].result, Err(UsesError::Blocked));
    assert!(matches!(
        reports[1].result,
        Err(UsesError::Ineligible { .. })
    ));
    assert_eq!(reports[2].result, Err(UsesError::Blocked));
    assert_eq!(text(&good), before);
    // Unknown projects and shared services are typed, not silent.
    let unknown = run(
        &projects,
        &[UseRequest {
            project: missing.0,
            include: true,
            base: String::new(),
        }],
    );
    assert!(matches!(
        unknown[0].result,
        Err(UsesError::UnknownProject { .. })
    ));
    let undefined = apply("nope", &shared(), &projects, &[ask(&good, true)]);
    assert!(matches!(
        undefined[0].result,
        Err(UsesError::UnknownShared { .. })
    ));
    assert_eq!(text(&good), before);
}

// covers: UXR-4.1
#[test]
fn an_external_edit_after_review_is_kept_and_needs_another_review() {
    let home = TempDir::new().unwrap();
    let shop = project(&home, "shop", "");
    let stale = ask(&shop, true);
    let ours = text(&shop).replace("# shop", "# edited by a person");
    std::fs::write(shop.1.join(MANIFEST_FILE), &ours).unwrap();
    let reports = run(std::slice::from_ref(&shop), &[stale]);
    assert!(matches!(
        reports[0].result,
        Err(UsesError::DocumentChanged { .. })
    ));
    assert_eq!(text(&shop), ours);
    // Reviewing again picks the new version and saves on top of it.
    let saved = run(std::slice::from_ref(&shop), &[ask(&shop, true)]);
    assert_eq!(saved[0].result, Ok(UseOutcome::Saved));
    assert!(text(&shop).contains("# edited by a person"));
}

// covers: UXR-4.1
#[test]
fn a_failure_after_the_first_write_reports_partial_results_and_retries_only_the_rest() {
    let home = TempDir::new().unwrap();
    let a = project(&home, "alpha", "");
    let b = project(&home, "bravo", "");
    let c = project(&home, "charlie", "");
    let projects = vec![a.clone(), b.clone(), c.clone()];
    let requests = [ask(&a, true), ask(&b, true), ask(&c, true)];
    let mut calls = 0;
    let reports = apply_with(
        "mysql",
        &shared(),
        &projects,
        &requests,
        |snapshot, text| {
            calls += 1;
            if calls == 2 {
                return Err(EditError::Io("disk full".to_owned()));
            }
            snapshot.commit(text)
        },
    );
    assert_eq!(reports[0].result, Ok(UseOutcome::Saved));
    assert!(matches!(reports[1].result, Err(UsesError::Io { .. })));
    assert_eq!(reports[2].result, Ok(UseOutcome::Saved));
    assert_eq!(declared(&a), ["mysql"]);
    assert!(declared(&b).is_empty(), "the failed file is untouched");
    assert_eq!(declared(&c), ["mysql"]);
    // Retry only the pending project: re-read, revalidated, written once.
    let retry = run(std::slice::from_ref(&b), &[requests[1].clone()]);
    assert_eq!(retry[0].result, Ok(UseOutcome::Saved));
    assert_eq!(declared(&b), ["mysql"]);
    assert_eq!(
        declared(&a),
        ["mysql"],
        "completed writes were not reverted"
    );
}

// covers: UXR-4.1
#[test]
fn a_read_only_manifest_is_listed_with_its_reason_and_never_written() {
    use std::os::unix::fs::PermissionsExt;
    let home = TempDir::new().unwrap();
    let locked = project(&home, "locked", "");
    let path = locked.1.join(MANIFEST_FILE);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let choice = inspect("locked", &locked.1, "mysql", &shared());
    assert!(!choice.eligible && !choice.declared);
    assert!(matches!(
        choice.reason,
        Some(UsesError::Ineligible {
            reason: Ineligibility::ReadOnly { .. }
        })
    ));
    let before = text(&locked);
    let reports = run(
        std::slice::from_ref(&locked),
        &[UseRequest {
            project: "locked".to_owned(),
            include: true,
            base: token(&before),
        }],
    );
    assert!(matches!(
        reports[0].result,
        Err(UsesError::Ineligible { .. })
    ));
    assert_eq!(text(&locked), before);
}

// covers: UXR-4.1
#[test]
fn inspection_explains_each_ineligible_project_and_marks_declared_ones() {
    let home = TempDir::new().unwrap();
    let declared_here = project(&home, "declared", "uses: [mysql]\n");
    let plain = project(&home, "plain", "");
    let bare = (
        "bare".to_owned(),
        std::fs::canonicalize(home.path()).unwrap().join("bare"),
    );
    std::fs::create_dir_all(&bare.1).unwrap();
    let odd = project(&home, "odd", "uses: ~\n");
    let yes = inspect("declared", &declared_here.1, "mysql", &shared());
    assert!(yes.declared && yes.eligible && yes.base.is_some());
    let no = inspect("plain", &plain.1, "mysql", &shared());
    assert!(!no.declared && no.eligible);
    let missing = inspect("bare", &bare.1, "mysql", &shared());
    assert!(!missing.eligible && missing.base.is_none());
    assert_eq!(
        missing.reason,
        Some(UsesError::Ineligible {
            reason: Ineligibility::NoManifest
        })
    );
    let unsupported = inspect("odd", &odd.1, "mysql", &shared());
    assert!(!unsupported.eligible && unsupported.reason.is_some());
}
