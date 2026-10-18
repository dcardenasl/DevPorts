use super::*;

// Anonymised from the audit's real machine: a Homebrew php-fpm, a Java server that a
// VS Code extension launches, a PHP server in a git worktree and a throwaway node.
fn listener(
    program: &[&str],
    cwd: &str,
    worktree: Option<&str>,
    project: Option<&str>,
) -> InstanceView {
    let mut view: InstanceView = serde_json::from_value(serde_json::json!({
        "ref": "p1", "state": "healthy", "role": "unregistered", "kind": "app",
        "registered": false, "launcher_chain": [], "shared": false, "ignored": false,
        "target_identity": null, "pid": 90, "port": 9000, "argv": []
    }))
    .unwrap();
    view.argv = program.iter().map(|part| (*part).to_owned()).collect();
    view.cwd = Some(cwd.to_owned());
    view.worktree = worktree.map(str::to_owned);
    view.project = project.map(str::to_owned);
    view
}

// covers: UXR-5.2
#[test]
fn known_tools_are_recognised_by_origin_without_reading_their_arguments() {
    let fpm = listener(
        &["/opt/homebrew/opt/php@8.3/sbin/php-fpm", "--nodaemonize"],
        "/opt/homebrew/var",
        None,
        None,
    );
    let found = classify(&fpm, None);
    assert_eq!(found.class, ProcessClass::Auxiliary);
    assert_eq!(found.origin.as_deref(), Some("Homebrew"));
    assert_eq!(found.program.as_deref(), Some("php-fpm"));
    assert_eq!(found.signals, ["homebrewDaemon"]);

    let java = listener(
        &[
            "/Users/dev/.vscode/extensions/vendor.java-1.2.3/jre/bin/java",
            "-Dserver.port=9100",
            "-jar",
            "/Users/dev/.vscode/extensions/vendor.java-1.2.3/server.jar",
        ],
        "/Users/dev/work/shop",
        Some("/Users/dev/work/shop"),
        Some("shop"),
    );
    let found = classify(&java, None);
    assert_eq!(
        found.class,
        ProcessClass::Auxiliary,
        "the tool signal beats the folder"
    );
    assert_eq!(found.origin.as_deref(), Some("VS Code"));
    assert!(found.signals.contains(&"editorExtension".to_owned()));
    assert!(
        found.signals.contains(&"knownProject".to_owned()),
        "evidence stays visible"
    );
}

// covers: UXR-5.2
#[test]
fn only_evidence_classifies_and_the_rest_stays_unclassified() {
    let server = listener(
        &["php", "-S", "localhost:18080"],
        "/Users/dev/work/shop",
        Some("/Users/dev/work/shop"),
        None,
    );
    assert_eq!(classify(&server, None).class, ProcessClass::Project);
    let known = listener(
        &["node", "dev.js"],
        "/Users/dev/work/blog",
        None,
        Some("blog"),
    );
    assert_eq!(classify(&known, None).class, ProcessClass::Project);
    let scratch = listener(&["node", "server.js"], "/tmp/scratch", None, None);
    let found = classify(&scratch, None);
    assert_eq!(found.class, ProcessClass::Unclassified);
    assert!(found.signals.is_empty() && found.origin.is_none());
    // A Homebrew node is somebody's server, not a daemon; a daemon elsewhere is not Homebrew's.
    let brew_node = listener(&["/opt/homebrew/bin/node", "app.js"], "/tmp/x", None, None);
    assert_eq!(classify(&brew_node, None).class, ProcessClass::Unclassified);
    let own_fpm = listener(&["/Users/dev/bin/php-fpm"], "/tmp/x", None, None);
    assert_eq!(classify(&own_fpm, None).class, ProcessClass::Unclassified);
}

// covers: UXR-5.2
#[test]
fn a_correction_wins_but_keeps_the_automatic_guess_and_can_be_reset() {
    let scratch = listener(&["node", "server.js"], "/tmp/scratch", None, None);
    let corrected = classify(&scratch, Some(ProcessClass::Auxiliary));
    assert_eq!(corrected.class, ProcessClass::Auxiliary);
    assert_eq!(corrected.detected, ProcessClass::Unclassified);
    assert!(corrected.corrected);
    let reset = classify(&scratch, None);
    assert_eq!(reset.class, ProcessClass::Unclassified);
    assert!(!reset.corrected);
    for class in [
        ProcessClass::Project,
        ProcessClass::Auxiliary,
        ProcessClass::Unclassified,
    ] {
        assert_eq!(ProcessClass::parse(class.as_str()), Some(class));
    }
    assert_eq!(ProcessClass::parse("ignored"), None);
}
