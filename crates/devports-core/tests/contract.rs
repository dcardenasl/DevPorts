//! The JSON the panel receives has the shape `ui/src/types.ts` declares.
//!
//! The Rust side serialises a snapshot and compares it with the committed
//! fixture; `ui/src/contract.test.ts` checks the TypeScript types against the
//! same fixture. Change a field on either side and one of the two fails.
//! Regenerate with `UPDATE_FIXTURES=1 cargo test -p devports-core --test contract`.

#![allow(clippy::expect_used, clippy::indexing_slicing)]

use devports_core::runtime::Snapshot;
use devports_proto::views::{
    AttentionItem, AttentionKind, InstanceRole, InstanceState, InstanceView, ProjectSummary,
    RepoSummary, ServiceSummary,
};

fn snapshot() -> Snapshot {
    let instance = InstanceView {
        instance_ref: "a1b2".to_owned(),
        project: Some("tienda-demo".to_owned()),
        service: Some("api".to_owned()),
        role: InstanceRole::Principal,
        state: InstanceState::Healthy,
        kind: "app".to_owned(),
        port: Some(8080),
        url: Some("http://localhost:8080".to_owned()),
        pid: Some(4121),
        container: None,
        branch: Some("main".to_owned()),
        worktree: None,
        cwd: Some("/work/tienda-demo/tienda-api".to_owned()),
        started_at: Some("2026-10-17T12:00:00Z".to_owned()),
        expires_at: None,
        reason: None,
        registered: true,
        launcher_chain: Vec::new(),
        duplicate_of: None,
        stale: None,
        shared: false,
        compose: None,
        argv: vec!["php".to_owned()],
        image: None,
        ignored: false,
    };
    Snapshot {
        revision: 1,
        instances: vec![instance],
        docker_reachable: true,
        docker_error: None,
        problems: Vec::new(),
        projects: vec![ProjectSummary {
            name: "tienda-demo".to_owned(),
            root: "/work/tienda-demo".to_owned(),
            uses: vec!["mysql".to_owned()],
            repos: vec![RepoSummary {
                name: "tienda-api".to_owned(),
                path: "/work/tienda-demo/tienda-api".to_owned(),
                branch: Some("main".to_owned()),
            }],
            services: vec![ServiceSummary {
                name: "api".to_owned(),
                repo: "/work/tienda-demo/tienda-api".to_owned(),
                port: Some(8080),
                kind: "app".to_owned(),
            }],
        }],
        attention: vec![AttentionItem {
            kind: AttentionKind::Duplicate,
            instance_ref: "a1b2".to_owned(),
        }],
        extend_minutes: 30,
    }
}

#[test]
fn the_snapshot_json_matches_the_committed_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/src/fixtures/snapshot.json");
    let mut text = serde_json::to_string_pretty(&snapshot()).expect("serialisable");
    text.push('\n');
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, &text).expect("write the fixture");
    }
    let committed = std::fs::read_to_string(&path).expect("the fixture exists");
    assert_eq!(committed, text, "regenerate with UPDATE_FIXTURES=1");
}
