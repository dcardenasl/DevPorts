//! The `devports` command.
//!
//! A thin client: it depends on `devports-proto` only, never on the core, so it
//! starts in milliseconds and cannot grow domain logic of its own. The core
//! decides; this program asks, prints and, for `run`, replaces itself with the
//! command it was told to run.

// A command-line tool talks to the terminal; the workspace denies printing
// because the core must never do it.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod client;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use devports_proto::{Op, PingResult};

/// Exit code for invalid arguments (`CLI.md`, section 3).
const EXIT_BAD_ARGS: u8 = 16;
/// Exit code when the application is not running and the command needs it.
const EXIT_CORE_DOWN: u8 = 17;
/// Exit code for any other failure.
const EXIT_FAILURE: u8 = 1;

/// Orden para los servidores de desarrollo: que un agente no levante otro si ya existe uno.
#[derive(Debug, Parser)]
#[command(name = "devports", version, about, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Muestra la versión.
    Version,
    /// Comprueba que la aplicación responde.
    #[command(hide = true)]
    Ping,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { EXIT_BAD_ARGS } else { 0 };
            // clap prints help and version to stdout and real errors to stderr.
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    match cli.command {
        Command::Version => {
            println!("devports {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Ping => ping(),
    }
}

fn ping() -> ExitCode {
    let result = client::socket_path().and_then(|path| client::call::<PingResult>(&path, Op::Ping));
    match result {
        Ok(pong) => {
            println!(
                "devports core {} (protocolo {}, pid {})",
                pong.version, pong.protocol, pong.pid
            );
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error),
    }
}

/// Prints a client error and picks the exit code for it.
fn fail(error: &client::ClientError) -> ExitCode {
    eprintln!("devports: {error}");
    match error {
        client::ClientError::CoreNotRunning(_) | client::ClientError::NoSocketPath => {
            ExitCode::from(EXIT_CORE_DOWN)
        }
        _ => ExitCode::from(EXIT_FAILURE),
    }
}
