//! The `devports` command.
//!
//! A thin client: it depends on `devports-proto` only, never on the core, so it
//! starts in milliseconds and cannot grow domain logic of its own. The core
//! decides; this program asks, prints and, for `run`, replaces itself with the
//! command it was told to run.

// A command-line tool talks to the terminal; the workspace denies printing
// because the core must never do it.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod ancestry;
mod client;
mod output;
mod run;

use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use devports_proto::duration::parse_duration;
use devports_proto::{
    AdoptArgs, AdoptResult, ErrorCode, ExtendArgs, ExtendResult, LogArgs, LogResult, LsArgs,
    LsResult, Op, PingResult, StopResult, TokenArgs,
};

/// Exit code for invalid arguments (`CLI.md`, section 3).
const EXIT_BAD_ARGS: u8 = 16;
/// Exit code when the application is not running and the command needs it.
const EXIT_CORE_DOWN: u8 = 17;
/// Exit code for any other failure.
const EXIT_FAILURE: u8 = 1;
/// `stop` / `extend`: the token matches no Temporal.
const EXIT_UNKNOWN_TOKEN: u8 = 30;
/// `extend`: it would pass the maximum lease.
const EXIT_LEASE_LIMIT: u8 = 31;

/// Orden para los servidores de desarrollo: que un agente no levante otro si ya existe uno.
#[derive(Debug, Parser)]
#[command(name = "devports", version, about, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Ejecuta un servicio con la política de DevPorts.
    Run(RunArgs),
    /// Lista las instancias.
    Ls(LsCliArgs),
    /// Detiene una instancia temporal con su token.
    Stop(TokenCliArgs),
    /// Suma tiempo a una instancia temporal con su token.
    Extend(ExtendCliArgs),
    /// Muestra el registro de actividad.
    Log(LogCliArgs),
    /// Propone un manifiesto para una carpeta (solo lectura).
    Adopt(AdoptCliArgs),
    /// Muestra la versión.
    Version,
    /// Comprueba que la aplicación responde.
    #[command(hide = true)]
    Ping,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RunAs {
    /// Instancia temporal con vencimiento.
    Test,
}

#[derive(Debug, Args)]
struct RunArgs {
    /// Nombre del servicio.
    service: String,
    /// Proyecto (por defecto, el de la carpeta actual).
    #[arg(long)]
    project: Option<String>,
    /// Si ya hay una Principal sana, no inicia nada.
    #[arg(long)]
    reuse: bool,
    /// Rol de la instancia.
    #[arg(long = "as", value_enum)]
    role: Option<RunAs>,
    /// Motivo de una instancia de prueba.
    #[arg(long)]
    reason: Option<String>,
    /// Duración de una instancia de prueba (30m, 2h).
    #[arg(long, value_parser = parse_ttl)]
    ttl: Option<Duration>,
    /// Salida en JSON.
    #[arg(long)]
    json: bool,
    /// Comando a ejecutar; `{PORT}` se reemplaza por el puerto asignado.
    #[arg(last = true)]
    command: Vec<String>,
}

#[derive(Debug, Args)]
struct LsCliArgs {
    /// Proyecto.
    #[arg(long)]
    project: Option<String>,
    /// También las instancias sin registrar y sin proyecto.
    #[arg(long)]
    all: bool,
    /// Salida en JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct TokenCliArgs {
    /// El `DEVPORTS_STOP_TOKEN` de la instancia.
    token: String,
}

#[derive(Debug, Args)]
struct ExtendCliArgs {
    /// El `DEVPORTS_STOP_TOKEN` de la instancia.
    token: String,
    /// Cuánto sumar (30m, 1h).
    #[arg(long, value_parser = parse_ttl)]
    by: Option<Duration>,
}

#[derive(Debug, Args)]
struct LogCliArgs {
    /// Solo la actividad de hoy (por defecto).
    #[arg(long, conflicts_with = "since")]
    today: bool,
    /// Actividad de los últimos 30m, 2h, 1d…
    #[arg(long, value_parser = parse_ttl)]
    since: Option<Duration>,
    /// Proyecto.
    #[arg(long)]
    project: Option<String>,
    /// Salida en JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct AdoptCliArgs {
    /// Carpeta (por defecto, la actual).
    dir: Option<String>,
}

fn parse_ttl(text: &str) -> Result<Duration, String> {
    parse_duration(text).map_err(|error| error.to_string())
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
        Command::Run(args) => run::run(&run::Args {
            service: args.service,
            project: args.project,
            reuse: args.reuse,
            as_test: args.role.is_some(),
            test_reason: args.reason,
            ttl: args.ttl,
            json: args.json,
            command: args.command,
        }),
        Command::Ls(args) => ls(&args),
        Command::Stop(args) => stop(&args),
        Command::Extend(args) => extend(&args),
        Command::Log(args) => log(&args),
        Command::Adopt(args) => adopt(&args),
    }
}

fn ping() -> ExitCode {
    match call::<PingResult>(Op::Ping) {
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

fn call<T: serde::de::DeserializeOwned>(op: Op) -> Result<T, client::ClientError> {
    client::call(&client::socket_path()?, op)
}

fn current_dir() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|dir| dir.display().to_string())
}

fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("devports: {error}"),
    }
}

fn ls(args: &LsCliArgs) -> ExitCode {
    let op = Op::Ls(LsArgs {
        project: args.project.clone(),
        cwd: current_dir(),
        all: args.all,
    });
    match call::<LsResult>(op) {
        Ok(result) => {
            if args.json {
                print_json(&result.instances);
            } else {
                print!("{}", output::instances_table(&result.instances));
            }
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error),
    }
}

fn stop(args: &TokenCliArgs) -> ExitCode {
    match call::<StopResult>(Op::Stop(TokenArgs {
        token: args.token.clone(),
    })) {
        Ok(result) => {
            if result.stopped {
                println!("devports: instancia {} detenida", result.instance_ref);
            } else {
                println!(
                    "devports: la instancia {} ya no corría",
                    result.instance_ref
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error),
    }
}

fn extend(args: &ExtendCliArgs) -> ExitCode {
    match call::<ExtendResult>(Op::Extend(ExtendArgs {
        token: args.token.clone(),
        by_seconds: args.by.map(|by| by.as_secs()),
    })) {
        Ok(result) => {
            println!(
                "devports: vence ahora a las {} ({} min acumulados)",
                output::clock_time(&result.expires_at),
                result.leased_total_seconds.checked_div(60).unwrap_or(0)
            );
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error),
    }
}

fn log(args: &LogCliArgs) -> ExitCode {
    let op = Op::Log(LogArgs {
        since_seconds: args.since.map(|since| since.as_secs()),
        project: args.project.clone(),
    });
    match call::<LogResult>(op) {
        Ok(result) => {
            if args.json {
                print_json(&result.entries);
            } else {
                print!("{}", output::log_lines(&result.entries));
            }
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error),
    }
}

fn adopt(args: &AdoptCliArgs) -> ExitCode {
    let Some(dir) = args.dir.clone().or_else(current_dir) else {
        eprintln!("devports: no se pudo leer la carpeta actual");
        return ExitCode::from(EXIT_BAD_ARGS);
    };
    // The core resolves relative paths against its own folder, so send an absolute one.
    let dir = std::fs::canonicalize(&dir).map_or(dir, |path| path.display().to_string());
    match call::<AdoptResult>(Op::AdoptPropose(AdoptArgs { dir })) {
        Ok(result) => {
            print!("{}", result.proposal);
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
        client::ClientError::Core(body) => match body.code {
            ErrorCode::UnknownToken => ExitCode::from(EXIT_UNKNOWN_TOKEN),
            ErrorCode::LeaseLimit => ExitCode::from(EXIT_LEASE_LIMIT),
            ErrorCode::NotFound => ExitCode::from(14),
            _ => ExitCode::from(EXIT_FAILURE),
        },
        client::ClientError::Protocol(_) => ExitCode::from(EXIT_FAILURE),
    }
}
