//! Smoke test of the `devports` binary: it starts and reports its version.

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
fn unknown_command_exits_with_the_bad_arguments_code() {
    let output = devports().arg("nonsense").output().unwrap();
    assert_eq!(output.status.code(), Some(16));
}
