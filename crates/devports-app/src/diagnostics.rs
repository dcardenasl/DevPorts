//! Where failures go until the application has a logger (task T2.1).
//!
//! Nothing can be shown to the user in a menu-bar app that has no open window, so
//! failures are written to stderr, which `launchd` captures and a terminal shows.

/// Reports a failure that cannot be handled where it happened.
#[allow(
    clippy::print_stderr,
    reason = "there is no logger yet; stderr is the only channel"
)]
pub fn report(error: &dyn std::fmt::Display) {
    eprintln!("devports: {error}");
}
