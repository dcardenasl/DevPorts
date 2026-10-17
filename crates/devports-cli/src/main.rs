//! The `devports` command.
//!
//! A thin client: it depends on `devports-proto` only, never on the core, so it
//! starts in milliseconds and cannot grow domain logic of its own.

// A command-line tool talks to the terminal; the workspace denies printing
// because the core must never do it.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

/// Exit code for invalid arguments (`CLI.md`, section 3).
const EXIT_BAD_ARGS: u8 = 16;

fn main() -> ExitCode {
    if let Some("version" | "--version") = std::env::args().nth(1).as_deref() {
        println!("devports {}", env!("CARGO_PKG_VERSION"));
        ExitCode::SUCCESS
    } else {
        eprintln!("devports: command not implemented yet");
        ExitCode::from(EXIT_BAD_ARGS)
    }
}
