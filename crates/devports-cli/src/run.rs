//! `devports run`: ask the core, then replace this process with the command.
//!
//! This is the only module that executes anything. `exec` keeps the pid, so the
//! server the core ends up watching is this very process, or one of its
//! descendants, which is what lets the core bind the instance with certainty.

use std::io::{BufRead, IsTerminal, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use devports_proto::run::{LauncherInfo, RunOutcome, RunRegister, RunRole};
use devports_proto::{ErrorBody, ErrorCode, Op};
use serde_json::{Value, json};

use crate::ancestry;
use crate::client::{self, ClientError};

/// Bundle identifier of the application, for `open -g -b`.
const BUNDLE_ID: &str = "com.davidcardenas.devports";
/// How long to wait for the core after trying to open the application.
const AUTOSTART_WAIT: Duration = Duration::from_secs(2);
/// Exit code when the core is down and the command cannot be run without it.
const EXIT_CORE_DOWN: u8 = 17;
/// Exit code for invalid arguments.
const EXIT_BAD_ARGS: u8 = 16;

/// What the person asked for.
#[derive(Debug, Clone)]
pub struct Args {
    pub service: String,
    pub project: Option<String>,
    pub reuse: bool,
    pub test_reason: Option<String>,
    pub as_test: bool,
    pub ttl: Option<Duration>,
    pub json: bool,
    pub command: Vec<String>,
}

pub fn run(args: &Args) -> ExitCode {
    if args.as_test
        && args
            .test_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        eprintln!("devports: --as test necesita --reason \"<motivo>\"");
        return ExitCode::from(EXIT_BAD_ARGS);
    }
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("devports: no se pudo leer la carpeta actual");
        return ExitCode::from(EXIT_BAD_ARGS);
    };
    let mut request = RunRegister {
        project: args.project.clone(),
        service: args.service.clone(),
        cwd: cwd.display().to_string(),
        role: if args.as_test {
            RunRole::Test
        } else {
            RunRole::Principal
        },
        reason: args.test_reason.clone(),
        ttl_seconds: args.ttl.map(|ttl| ttl.as_secs()),
        reuse: args.reuse,
        command: (!args.command.is_empty()).then(|| args.command.clone()),
        launcher: LauncherInfo {
            pid: std::process::id(),
            chain: ancestry::chain(),
        },
    };

    loop {
        let outcome = match ask(&request) {
            Ok(outcome) => outcome,
            Err(ClientError::CoreNotRunning(_) | ClientError::NoSocketPath) => {
                return fail_open(args);
            }
            Err(ClientError::Core(error)) => return core_error(&error, args.json),
            Err(error) => {
                eprintln!("devports: {error}");
                return ExitCode::from(1);
            }
        };
        match outcome {
            RunOutcome::Accepted(accepted) => return start(&accepted, args.json),
            RunOutcome::Reused(reused) => {
                if args.json {
                    print_json(
                        &json!({"ok": true, "code": 0, "reused": true, "instance": reused.instance}),
                    );
                } else {
                    println!("{}", reused.message);
                }
                return ExitCode::SUCCESS;
            }
            RunOutcome::Rejected(rejected) => {
                if let Some(answer) = interactive_choice(args, &rejected) {
                    match answer {
                        Choice::Reuse => request.reuse = true,
                        Choice::Test(reason) => {
                            request.role = RunRole::Test;
                            request.reason = Some(reason);
                        }
                    }
                    continue;
                }
                if args.json {
                    print_json(&rejected_json(&rejected));
                } else {
                    eprintln!("{}", rejected.message);
                }
                return ExitCode::from(rejected.error.exit_code());
            }
        }
    }
}

fn ask(request: &RunRegister) -> Result<RunOutcome, ClientError> {
    let path = client::socket_path()?;
    match client::call::<RunOutcome>(&path, Op::RunRegister(request.clone())) {
        Err(ClientError::CoreNotRunning(_)) if autostart(&path) => {
            client::call(&path, Op::RunRegister(request.clone()))
        }
        other => other,
    }
}

/// Opens the application without bringing it to the front and waits for its
/// socket. `true` when the core answers afterwards.
// The CLI's only other process spawn besides the command itself.
#[allow(clippy::disallowed_methods)]
fn autostart(path: &Path) -> bool {
    if std::env::var_os("DEVPORTS_NO_AUTOSTART").is_some_and(|value| value == "1") {
        return false;
    }
    let opened = std::process::Command::new("open")
        .args(["-g", "-b", BUNDLE_ID])
        .status()
        .is_ok_and(|status| status.success());
    if !opened {
        return false;
    }
    let started = Instant::now();
    while started.elapsed() < AUTOSTART_WAIT {
        if client::call::<devports_proto::PingResult>(path, Op::Ping).is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

fn core_error(error: &ErrorBody, json: bool) -> ExitCode {
    let code = match error.code {
        ErrorCode::NotFound => 14,
        _ => 1,
    };
    if json {
        print_json(
            &json!({"ok": false, "code": code, "error": "core_error", "message": error.message}),
        );
    } else {
        eprintln!("devports: {}", error.message);
    }
    ExitCode::from(code)
}

fn rejected_json(rejected: &devports_proto::run::Rejected) -> Value {
    let error = serde_json::to_value(rejected.error).unwrap_or(Value::Null);
    json!({
        "ok": false,
        "code": rejected.error.exit_code(),
        "error": error,
        "project": rejected.project,
        "service": rejected.service,
        "existing": rejected.existing,
        "alternatives": rejected.alternatives,
        "message": rejected.message,
    })
}

fn print_json(value: &Value) {
    println!("{value}");
}

/// Prints the banner and becomes the command. Returns only if that failed.
fn start(accepted: &devports_proto::run::Accepted, json: bool) -> ExitCode {
    if json {
        eprintln!(
            "{}",
            json!({
                "ok": true, "code": 0, "ref": accepted.instance_ref, "project": accepted.project,
                "service": accepted.service, "port": accepted.port, "url": accepted.url,
                "expires_at": accepted.expires_at, "token": accepted.token,
            })
        );
    } else {
        for line in &accepted.banner {
            eprintln!("{line}");
        }
    }
    exec(
        &accepted.shell,
        Some(Path::new(&accepted.run_dir)),
        &accepted.env,
    )
}

/// Replaces this process with `sh -c <shell>`.
#[allow(clippy::disallowed_methods)]
fn exec(
    shell: &str,
    dir: Option<&Path>,
    env: &std::collections::BTreeMap<String, String>,
) -> ExitCode {
    let mut command = std::process::Command::new("sh");
    command.arg("-c").arg(shell).envs(env);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let error = command.exec();
    eprintln!("devports: no se pudo ejecutar el comando: {error}");
    ExitCode::from(if error.kind() == std::io::ErrorKind::NotFound {
        127
    } else {
        126
    })
}

/// The core is down: warn and run the command anyway, unprotected (ADR-018).
fn fail_open(args: &Args) -> ExitCode {
    if args.command.is_empty() {
        eprintln!(
            "devports: núcleo inactivo y no hay comando después de `--`; abre DevPorts y vuelve a intentarlo"
        );
        return ExitCode::from(EXIT_CORE_DOWN);
    }
    let port = std::env::var("PORT").ok();
    let mut words = Vec::with_capacity(args.command.len());
    for word in &args.command {
        if word.contains("{PORT}") {
            let Some(port) = &port else {
                eprintln!(
                    "devports: núcleo inactivo y el comando usa {{PORT}} sin que haya un PORT definido"
                );
                return ExitCode::from(EXIT_CORE_DOWN);
            };
            words.push(word.replace("{PORT}", port));
        } else {
            words.push(word.clone());
        }
    }
    eprintln!("devports: núcleo inactivo, sin protección");
    let quoted: Vec<String> = words.iter().map(|word| shell_quote(word)).collect();
    exec(
        &format!("exec {}", quoted.join(" ")),
        None,
        &std::collections::BTreeMap::new(),
    )
}

/// Quotes one word for `sh`.
#[must_use]
pub fn shell_quote(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=,+@%".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

enum Choice {
    Reuse,
    Test(String),
}

/// `DEVPORTS_INTERACTIVE=1` on a terminal asks what to do about a conflict
/// (`CLI.md`, section 3). Without it, nothing is ever asked.
fn interactive_choice(args: &Args, rejected: &devports_proto::run::Rejected) -> Option<Choice> {
    use devports_proto::run::RejectKind;
    let wanted = std::env::var_os("DEVPORTS_INTERACTIVE").is_some_and(|value| value == "1");
    if !wanted
        || args.json
        || !std::io::stdin().is_terminal()
        || rejected.error != RejectKind::Conflict
    {
        return None;
    }
    eprintln!("{}", rejected.message);
    eprint!("[r]eutilizar / [t]emporal / [c]ancelar: ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer).ok()?;
    match answer.trim().chars().next()? {
        'r' | 'R' => Some(Choice::Reuse),
        't' | 'T' => {
            eprint!("Motivo [prueba manual]: ");
            let _ = std::io::stderr().flush();
            let mut reason = String::new();
            std::io::stdin().lock().read_line(&mut reason).ok()?;
            let reason = reason.trim();
            Some(Choice::Test(
                if reason.is_empty() {
                    "prueba manual"
                } else {
                    reason
                }
                .to_owned(),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_leaves_safe_words_and_protects_the_rest() {
        assert_eq!(shell_quote("php"), "php");
        assert_eq!(shell_quote("--port=8080"), "--port=8080");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("$HOME;rm"), "'$HOME;rm'");
    }
}
