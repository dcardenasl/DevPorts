//! Preferences through the running actor: saved, read back, and kept across a restart.
#![allow(clippy::expect_used)]

use std::sync::Arc;

use devports_core::process::fake::FakeProcessSource;
use devports_core::runtime::{Core, CoreHandle, CoreOptions};
use devports_proto::{LanguagePreference, Preferences, ThemePreference};
use tempfile::TempDir;

async fn start(home: &TempDir) -> (Core, CoreHandle) {
    let core = Core::start(
        CoreOptions {
            home: home.path().to_path_buf(),
            database: home.path().join("db"),
            socket: home.path().join("s/d.sock"),
        },
        Arc::new(FakeProcessSource::default()),
    )
    .await
    .expect("core");
    let handle = core.handle();
    (core, handle)
}

// covers: UXR-9.1
#[tokio::test]
async fn preferences_default_to_system_and_survive_a_restart() {
    let home = TempDir::new().expect("home");
    let chosen = Preferences {
        theme: ThemePreference::Dark,
        language: LanguagePreference::En,
    };
    {
        let (core, handle) = start(&home).await;
        assert_eq!(
            handle.preferences().await.expect("read"),
            Preferences::default()
        );
        assert_eq!(handle.set_preferences(chosen).await.expect("save"), chosen);
        assert_eq!(handle.preferences().await.expect("read"), chosen);
        handle.shutdown();
        core.wait().await;
    }
    let (_core, handle) = start(&home).await;
    assert_eq!(handle.preferences().await.expect("read"), chosen);
}
