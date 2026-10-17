//! The CLI's output against golden files (`PRUEBAS.md`, U-11): for every
//! refusal of `devports run` the exit code, the text for people and the
//! `--json` document are fixed, because agents and scripts depend on them.
//!
//! A fake core answers each service name with a canned outcome, so the output is
//! deterministic. Regenerate with `UPDATE_GOLDEN=1 cargo test -p devports-cli --test golden`.

#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Arc;

use devports_core::ipc::{IpcServer, Limits, PeerInfo, RequestHandler};
use devports_proto::run::{
    Accepted, Alternative, AlternativeKind, RejectKind, Rejected, Reused, RunOutcome,
};
use devports_proto::views::{InstanceRole, InstanceState, InstanceView};
use devports_proto::{ErrorBody, ErrorCode, Op, Request, Response};

fn existing() -> InstanceView {
    InstanceView {
        instance_ref: "a1b2".to_owned(),
        project: Some("tienda-demo".to_owned()),
        service: Some("api".to_owned()),
        role: InstanceRole::Principal,
        state: InstanceState::Healthy,
        kind: "app".to_owned(),
        port: Some(8080),
        url: Some("http://localhost:8080".to_owned()),
        pid: Some(4121),
        target_identity: Some("process-fingerprint".to_owned()),
        ports: vec![8080],
        container: None,
        branch: Some("feature/x".to_owned()),
        worktree: Some("/Users/me/tienda-api".to_owned()),
        cwd: Some("/Users/me/tienda-api".to_owned()),
        started_at: Some("2026-10-16T12:00:00-03:00".to_owned()),
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
    }
}

fn rejection(kind: RejectKind, message: &str, with_existing: bool) -> RunOutcome {
    let alternatives = if matches!(kind, RejectKind::Conflict | RejectKind::Unhealthy) {
        vec![
            Alternative {
                kind: AlternativeKind::Reuse,
                command: Some("devports run api --reuse".to_owned()),
                needs_human: false,
            },
            Alternative {
                kind: AlternativeKind::Test,
                command: Some(
                    "devports run api --as test --reason \"<motivo>\" -- <comando con {PORT}>"
                        .to_owned(),
                ),
                needs_human: false,
            },
            Alternative {
                kind: AlternativeKind::StopInApp,
                command: None,
                needs_human: true,
            },
        ]
    } else {
        Vec::new()
    };
    RunOutcome::Rejected(Rejected {
        error: kind,
        message: message.to_owned(),
        project: Some("tienda-demo".to_owned()),
        service: Some("api".to_owned()),
        existing: with_existing.then(existing),
        alternatives,
    })
}

/// The canned answer for a service name.
fn outcome_for(service: &str) -> Option<RunOutcome> {
    Some(match service {
        "conflict" => rejection(
            RejectKind::Conflict,
            "devports: conflicto en api (proyecto tienda-demo)\nYa corre la instancia Principal de \"api\".",
            true,
        ),
        "unhealthy" => rejection(
            RejectKind::Unhealthy,
            "devports: api (proyecto tienda-demo) existe pero no responde",
            true,
        ),
        "limit" => rejection(
            RejectKind::Limit,
            "devports: api ya tiene 2 pruebas en marcha",
            false,
        ),
        "no_command" => rejection(
            RejectKind::NoCommand,
            "devports: api no tiene comando de inicio; indícalo después de `--`",
            false,
        ),
        "not_found" => rejection(
            RejectKind::NotFound,
            "devports: no existe el servicio «api» en el proyecto tienda-demo",
            false,
        ),
        "bad_args" => rejection(
            RejectKind::BadArgs,
            "devports: --ttl supera el máximo de 4 h",
            false,
        ),
        "port_busy" => rejection(
            RejectKind::PortBusy,
            "devports: el puerto 8080 lo ocupa node (pid 77), que no es api",
            false,
        ),
        "no_port" => rejection(
            RejectKind::NoPort,
            "devports: no hay un puerto libre en el rango de pruebas",
            false,
        ),
        "reuse" => RunOutcome::Reused(Reused {
            instance: existing(),
            message: "devports: api ya corre (Principal) en http://localhost:8080 (pid 4121)"
                .to_owned(),
        }),
        "accepted" => RunOutcome::Accepted(Accepted {
            instance_ref: "a1b2".to_owned(),
            project: "tienda-demo".to_owned(),
            service: "api".to_owned(),
            role: InstanceRole::Temporal,
            port: 51234,
            url: "http://localhost:51234".to_owned(),
            expires_at: Some("2026-10-17T14:50:00-03:00".to_owned()),
            token: Some("9f1c0000000000000000000000000000".to_owned()),
            run_dir: "/".to_owned(),
            shell: "exec true".to_owned(),
            env: BTreeMap::new(),
            banner: vec![
                "devports: api (proyecto tienda-demo) como TEMPORAL en http://localhost:51234, vence 14:50 (30 m)".to_owned(),
                "DEVPORTS_STOP_TOKEN=9f1c0000000000000000000000000000   (guárdalo: permite detener o extender esta prueba)".to_owned(),
            ],
        }),
        _ => return None,
    })
}

struct Canned;

impl RequestHandler for Canned {
    async fn handle(&self, request: Request, _peer: PeerInfo) -> Response {
        let Op::RunRegister(register) = request.op else {
            return Response::err(
                request.id,
                ErrorBody::new(ErrorCode::UnknownOp, "only run is canned"),
            );
        };
        match outcome_for(&register.service) {
            Some(outcome) => Response::ok(request.id, &outcome).expect("serialisable"),
            None => Response::err(
                request.id,
                ErrorBody::new(ErrorCode::NotFound, "no canned outcome"),
            ),
        }
    }
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name)
}

fn check(name: &str, actual: &str) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).expect("write the golden file");
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("missing golden file {}: {error}", path.display()));
    assert_eq!(
        expected, actual,
        "{name} differs (UPDATE_GOLDEN=1 to regenerate)"
    );
}

fn run(socket: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_devports"))
        .args(args)
        .env("DEVPORTS_SOCKET", socket)
        .env("DEVPORTS_NO_AUTOSTART", "1")
        .output()
        .expect("the CLI runs")
}

const REFUSALS: [(&str, u8); 8] = [
    ("conflict", 10),
    ("unhealthy", 11),
    ("limit", 12),
    ("no_command", 13),
    ("not_found", 14),
    ("bad_args", 16),
    ("port_busy", 18),
    ("no_port", 19),
];

// covers: U-11, RF-07
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_refusal_has_its_exit_code_text_and_json_fixed() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let socket = dir.path().join("g.sock");
    let server = IpcServer::bind(&socket, Limits::default())
        .await
        .expect("bind");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(server.serve(Arc::new(Canned), async {
        let _ = stopped.await;
    }));

    let socket_for_cli = socket.clone();
    tokio::task::spawn_blocking(move || {
        for (service, code) in REFUSALS {
            let text = run(&socket_for_cli, &["run", service]);
            assert_eq!(text.status.code(), Some(i32::from(code)), "{service}");
            assert!(
                text.stdout.is_empty(),
                "{service}: a refusal prints on stderr only"
            );
            check(
                &format!("{service}.txt"),
                &String::from_utf8_lossy(&text.stderr),
            );

            let json = run(&socket_for_cli, &["run", service, "--json"]);
            assert_eq!(
                json.status.code(),
                Some(i32::from(code)),
                "{service} --json"
            );
            let parsed: serde_json::Value =
                serde_json::from_slice(&json.stdout).expect("--json prints one JSON document");
            assert_eq!(parsed["ok"], false);
            assert_eq!(parsed["code"], i32::from(code));
            check(
                &format!("{service}.json"),
                &String::from_utf8_lossy(&json.stdout),
            );
        }

        // `--reuse` success: code 0, nothing started.
        let reuse = run(&socket_for_cli, &["run", "reuse", "--reuse"]);
        assert_eq!(reuse.status.code(), Some(0));
        check("reuse.txt", &String::from_utf8_lossy(&reuse.stdout));
        let reuse_json = run(&socket_for_cli, &["run", "reuse", "--reuse", "--json"]);
        check("reuse.json", &String::from_utf8_lossy(&reuse_json.stdout));

        // An accepted temporal: the banner on stderr and the command's own output.
        let accepted = run(
            &socket_for_cli,
            &["run", "accepted", "--as", "test", "--reason", "x"],
        );
        assert_eq!(accepted.status.code(), Some(0));
        check("accepted.txt", &String::from_utf8_lossy(&accepted.stderr));
    })
    .await
    .expect("join");

    let _ = stop.send(());
    let _ = task.await;
}

// covers: I-12, ADR-023
#[test]
fn replace_is_not_an_argument_of_run() {
    let output = Command::new(env!("CARGO_BIN_EXE_devports"))
        .args(["run", "api", "--replace"])
        .env("DEVPORTS_SOCKET", "/nonexistent/none.sock")
        .output()
        .expect("the CLI runs");
    assert_eq!(output.status.code(), Some(16));
    assert!(output.stdout.is_empty());
}
