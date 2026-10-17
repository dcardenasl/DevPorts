//! Smoke tests of the `devports` binary that need no core.

#![allow(clippy::disallowed_methods)]

use std::process::Command;

fn devports() -> Command {
    Command::new(env!("CARGO_BIN_EXE_devports"))
}

#[test]
fn version_prints_name_and_version() {
    let output = devports().arg("version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("devports {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn an_unknown_command_exits_with_the_bad_arguments_code() {
    let output = devports().arg("nonsense").output().unwrap();
    assert_eq!(output.status.code(), Some(16));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn help_succeeds_and_goes_to_stdout() {
    let output = devports().arg("--help").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("devports"));
}

#[test]
fn without_a_core_ping_says_so_and_exits_17() {
    let dir = tempfile::TempDir::new().unwrap();
    let output = devports()
        .arg("ping")
        .env("DEVPORTS_SOCKET", dir.path().join("none.sock"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
    assert!(String::from_utf8_lossy(&output.stderr).contains("not running"));
}
