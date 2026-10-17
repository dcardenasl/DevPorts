use super::*;
use tempfile::TempDir;

fn principal(reference: &str) -> InstanceRecord {
    let mut record = InstanceRecord::new(reference, Role::Principal, State::Running);
    record.project = Some("shop".to_owned());
    record.service = Some("api".to_owned());
    record.port = Some(8080);
    record.pid = Some(4121);
    record.pid_start = Some(1_700_000_000_123_456);
    record.cwd = Some("/work/shop/api".to_owned());
    record.started_at = Some(1_700_000_000);
    record
}

fn temporal(reference: &str, token_hash: &str) -> InstanceRecord {
    let mut record = InstanceRecord::new(reference, Role::Temporal, State::Running);
    record.project = Some("shop".to_owned());
    record.service = Some("api".to_owned());
    record.token_hash = Some(token_hash.to_owned());
    record.reason = Some("QA login".to_owned());
    record.expires_at = Some(1_700_001_800);
    record.leased_total_s = 1800;
    record
}

#[test]
fn a_fresh_database_is_at_the_latest_schema_in_wal_mode() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("devports.db")).unwrap();
    assert_eq!(store.schema_version().unwrap(), MIGRATIONS.len());
    let mode: String = store
        .connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn opening_twice_does_not_migrate_twice() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    drop(Store::open(&path).unwrap());
    let again = Store::open(&path).unwrap();
    assert_eq!(again.schema_version().unwrap(), MIGRATIONS.len());
}

#[test]
fn a_database_from_a_newer_build_is_refused_untouched() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("devports.db");
    drop(Store::open(&path).unwrap());
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);

    let error = Store::open(&path).unwrap_err();

    assert!(matches!(error, StoreError::TooNew { found: 99, .. }));
}

#[test]
fn an_instance_round_trips_with_every_column() {
    let store = Store::open_in_memory().unwrap();
    let mut record = temporal("c3d4", "hash-1");
    record.registered = true;
    record.port = Some(51234);
    record.pid = Some(5530);
    record.pid_start = Some(1_700_000_100_000_001);
    record.container_id = Some("abc".to_owned());
    record.branch = Some("feature/x".to_owned());
    record.worktree = Some("/work/wt".to_owned());
    record.launcher_pid = Some(5500);
    record.launcher_chain = r#"[{"pid":1,"name":"zsh"}]"#.to_owned();
    record.reserved_at = Some(1_700_000_050);
    record.started_at = Some(1_700_000_060);
    record.warned = true;
    record.healthy = Some(false);

    let stored = store.insert_instance(&record).unwrap();
    assert!(stored.id > 0);

    let mut expected = record.clone();
    expected.id = stored.id;
    assert_eq!(store.live_instances().unwrap(), [expected]);
}

#[test]
fn updating_writes_the_changes_back() {
    let store = Store::open_in_memory().unwrap();
    let mut record = store.insert_instance(&principal("a1b2")).unwrap();
    record.state = State::Down;
    record.healthy = Some(false);
    record.end_reason = Some("process gone".to_owned());
    store.update_instance(&record).unwrap();
    assert_eq!(store.live_instances().unwrap(), [record]);
}

#[test]
fn only_live_instances_are_listed() {
    let store = Store::open_in_memory().unwrap();
    for (reference, state) in [
        ("r1", State::Reserved),
        ("r2", State::Running),
        ("r3", State::Stopped),
        ("r4", State::Released),
        ("r5", State::ExpiredAlive),
    ] {
        let mut record = principal(reference);
        record.state = state;
        record.pid = None;
        store.insert_instance(&record).unwrap();
    }
    let live: Vec<String> = store
        .live_instances()
        .unwrap()
        .into_iter()
        .map(|record| record.reference)
        .collect();
    assert_eq!(live, ["r1", "r2", "r5"]);
}

#[test]
fn references_and_tokens_are_unique() {
    let store = Store::open_in_memory().unwrap();
    store.insert_instance(&temporal("t1", "same-hash")).unwrap();
    assert!(
        store.insert_instance(&temporal("t1", "other")).is_err(),
        "duplicate ref"
    );
    assert!(
        store.insert_instance(&temporal("t2", "same-hash")).is_err(),
        "duplicate token"
    );
}

#[test]
fn an_instance_is_found_by_its_token_hash_only() {
    let store = Store::open_in_memory().unwrap();
    let stored = store.insert_instance(&temporal("t1", "hash-a")).unwrap();
    assert_eq!(
        store.instance_by_token_hash("hash-a").unwrap(),
        Some(stored)
    );
    assert_eq!(store.instance_by_token_hash("hash-b").unwrap(), None);
}

#[test]
fn the_database_rejects_values_the_code_does_not_know() {
    let store = Store::open_in_memory().unwrap();
    let result = store.connection.execute(
        "INSERT INTO instances (ref, role, state) VALUES ('x', 'wizard', 'running')",
        [],
    );
    assert!(result.is_err());
    let result = store.connection.execute(
        "INSERT INTO instances (ref, role, state) VALUES ('y', 'principal', 'confused')",
        [],
    );
    assert!(result.is_err());
}

#[test]
fn a_corrupt_state_is_reported_not_guessed() {
    assert!(matches!(
        State::parse("confused"),
        Err(StoreError::Unreadable {
            column: "state",
            ..
        })
    ));
    assert!(matches!(
        Role::parse("wizard"),
        Err(StoreError::Unreadable { column: "role", .. })
    ));
}

#[test]
fn instances_survive_closing_and_reopening_the_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sub").join("devports.db");
    let stored = {
        let store = Store::open(&path).unwrap();
        store.insert_instance(&temporal("t1", "hash-a")).unwrap()
    };
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.live_instances().unwrap(), [stored]);
}

#[test]
fn events_are_read_back_in_order_and_filtered() {
    let store = Store::open_in_memory().unwrap();
    store
        .append_event(
            300,
            "run_rejected",
            Some("shop"),
            Some("api"),
            Some("a1b2"),
            r#"{"code":10}"#,
        )
        .unwrap();
    store
        .append_event(100, "core_started", None, None, None, "{}")
        .unwrap();
    store
        .append_event(
            200,
            "lease_expired",
            Some("blog"),
            Some("web"),
            Some("c3d4"),
            "{}",
        )
        .unwrap();

    let all = store.events_since(0, None).unwrap();
    let kinds: Vec<&str> = all.iter().map(|event| event.kind.as_str()).collect();
    assert_eq!(kinds, ["core_started", "lease_expired", "run_rejected"]);

    let recent = store.events_since(250, None).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].payload, r#"{"code":10}"#);

    let shop: Vec<String> = store
        .events_since(0, Some("shop"))
        .unwrap()
        .into_iter()
        .map(|event| event.kind)
        .collect();
    assert_eq!(shop, ["run_rejected"]);
}

#[test]
fn old_events_are_pruned() {
    let store = Store::open_in_memory().unwrap();
    for ts in [10, 20, 30, 40] {
        store.append_event(ts, "x", None, None, None, "{}").unwrap();
    }
    assert_eq!(store.prune_events(30).unwrap(), 2);
    let left: Vec<i64> = store
        .events_since(0, None)
        .unwrap()
        .into_iter()
        .map(|e| e.ts)
        .collect();
    assert_eq!(left, [30, 40]);
    assert_eq!(store.prune_events(0).unwrap(), 0);
}
