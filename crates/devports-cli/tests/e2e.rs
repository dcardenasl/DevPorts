//! The real CLI against the real runtime (fake host): `run`, `ls`, `stop`,
//! `extend`, `log`, `adopt` and the fail-open path.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;

use devports_core::process::fake::FakeProcessSource;
use devports_core::process::{ListeningProcess, ProcessId, ProcessInfo, ProcessStart};
use devports_core::runtime::{Core, CoreOptions};
use tempfile::TempDir;

struct Fixture {
    _home: TempDir,
    socket: PathBuf,
    project: PathBuf,
    source: Arc<FakeProcessSource>,
    core: Option<Core>,
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

async fn fixture() -> Fixture {
    let home = TempDir::new().expect("tempdir");
    let work = home.path().join("work");
    let project = work.join("shop");
    write(
        &home.path().join(".config/devports/config.yml"),
        &format!("version: 1\nroots: [{}]\n", work.display()),
    );
    write(
        &project.join(".devports.yml"),
        "version: 1\nproject: shop\nservices:\n  api:\n    cwd: .\n    port: 8080\n    start: echo serving on ${PORT}\n  web:\n    cwd: .\n",
    );
    let socket = home.path().join("s").join("d.sock");
    let source = Arc::new(FakeProcessSource::default());
    let core = Core::start(
        CoreOptions {
            home: home.path().to_path_buf(),
            database: home.path().join("d").join("devports.db"),
            socket: socket.clone(),
        },
        Arc::clone(&source),
    )
    .await
    .expect("the core starts");
    let handle = core.handle();
    for _ in 0..100 {
        if handle
            .snapshot()
            .instances
            .iter()
            .any(|v| v.service.as_deref() == Some("api"))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let project = std::fs::canonicalize(project).expect("canonical");
    Fixture {
        _home: home,
        socket,
        project,
        source,
        core: Some(core),
    }
}

impl Fixture {
    async fn devports(&self, args: &[&str]) -> Output {
        let socket = self.socket.clone();
        let project = self.project.clone();
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        tokio::task::spawn_blocking(move || {
            Command::new(env!("CARGO_BIN_EXE_devports"))
                .args(&args)
                .current_dir(project)
                .env("DEVPORTS_SOCKET", socket)
                .env("DEVPORTS_NO_AUTOSTART", "1")
                .output()
                .expect("the CLI runs")
        })
        .await
        .expect("join")
    }

    /// Makes a server appear on `port`, as the command the CLI started would.
    async fn server_appears(&self, port: u16, reference: &str) {
        let started = jiff::Timestamp::now().as_second() + 1;
        self.source.set(vec![ListeningProcess {
            info: ProcessInfo {
                id: ProcessId {
                    pid: 7777,
                    start: ProcessStart(u64::try_from(started).expect("positive") * 1_000_000),
                },
                ppid: 1,
                pgid: 7777,
                uid: 4_000_000, // never the real user: a stop in a test must be refused, not delivered
                argv: vec!["server".to_owned()],
                cwd: Some(self.project.clone()),
            },
            ports: vec![port],
        }]);
        let handle = self.core.as_ref().expect("running").handle();
        for _ in 0..100 {
            let bound = handle
                .snapshot()
                .instances
                .iter()
                .any(|view| view.instance_ref == reference && view.pid == Some(7777));
            if bound {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("the server was never bound");
    }

    async fn finish(mut self) {
        if let Some(core) = self.core.take() {
            core.handle().shutdown();
            core.wait().await;
        }
    }
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_starts_the_command_and_a_second_run_is_refused_with_code_10() {
    let fixture = fixture().await;

    let first = fixture.devports(&["run", "api"]).await;
    assert_eq!(first.status.code(), Some(0), "{}", err(&first));
    assert_eq!(out(&first).trim(), "serving on 8080");
    assert!(err(&first).contains("api (proyecto shop) como PRINCIPAL en http://localhost:8080"));

    let second = fixture.devports(&["run", "api"]).await;
    assert_eq!(second.status.code(), Some(10));
    assert!(err(&second).contains("conflicto en api (proyecto shop)"));

    let json = fixture.devports(&["run", "api", "--json"]).await;
    assert_eq!(json.status.code(), Some(10));
    let value: serde_json::Value = serde_json::from_str(out(&json).trim()).expect("json");
    assert_eq!(value["ok"], false);
    assert_eq!(value["code"], 10);
    assert_eq!(value["error"], "conflict");
    assert_eq!(value["alternatives"][2]["needs_human"], true);

    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn arguments_are_validated() {
    let fixture = fixture().await;
    let no_reason = fixture.devports(&["run", "api", "--as", "test"]).await;
    assert_eq!(no_reason.status.code(), Some(16));
    let bad_ttl = fixture.devports(&["run", "api", "--ttl", "soon"]).await;
    assert_eq!(bad_ttl.status.code(), Some(16));
    let no_service = fixture.devports(&["run", "ghost"]).await;
    assert_eq!(no_service.status.code(), Some(14));
    let no_command = fixture.devports(&["run", "web"]).await;
    assert_eq!(no_command.status.code(), Some(13));
    let command = fixture
        .devports(&["run", "web", "--", "echo", "port", "{PORT}"])
        .await;
    assert_eq!(command.status.code(), Some(0), "{}", err(&command));
    assert!(out(&command).starts_with("port "), "{}", out(&command));
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_temporal_is_listed_extended_logged_and_stopped_with_its_token() {
    let fixture = fixture().await;
    let run = fixture
        .devports(&[
            "run", "api", "--as", "test", "--reason", "QA login", "--ttl", "10m", "--", "true",
        ])
        .await;
    assert_eq!(run.status.code(), Some(0), "{}", err(&run));
    let banner = err(&run);
    assert!(banner.contains("como TEMPORAL"), "{banner}");
    let token = banner
        .lines()
        .find_map(|line| line.strip_prefix("DEVPORTS_STOP_TOKEN="))
        .and_then(|rest| rest.split_whitespace().next())
        .expect("the token is shown once")
        .to_owned();

    let port: u16 = banner
        .split("http://localhost:")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok())
        .expect("the banner names the port");
    let ls = fixture.devports(&["ls", "--json"]).await;
    let list = out(&ls);
    let reference = serde_json::from_str::<Vec<serde_json::Value>>(list.trim())
        .expect("json")
        .iter()
        .find(|view| view["role"] == "temporal")
        .and_then(|view| view["ref"].as_str())
        .expect("the temporal is listed")
        .to_owned();
    fixture.server_appears(port, &reference).await;
    assert!(list.contains("\"role\":\"temporal\""), "{list}");
    assert!(!list.contains(&token), "ls must never show the token");
    let table = fixture.devports(&["ls"]).await;
    assert!(out(&table).contains("temporal"));

    let extend = fixture.devports(&["extend", &token, "--by", "5m"]).await;
    assert_eq!(extend.status.code(), Some(0), "{}", err(&extend));
    assert!(out(&extend).contains("min acumulados"));

    let log = fixture.devports(&["log"]).await;
    assert!(out(&log).contains("QA login"), "{}", out(&log));

    let stop = fixture.devports(&["stop", &token]).await;
    assert_eq!(stop.status.code(), Some(0), "{}", err(&stop));
    let wrong = fixture.devports(&["stop", "nope"]).await;
    assert_eq!(wrong.status.code(), Some(30));
    let too_much = fixture.devports(&["extend", "nope"]).await;
    assert_eq!(too_much.status.code(), Some(30));
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopt_prints_a_proposal_and_writes_nothing() {
    let fixture = fixture().await;
    let bare = fixture.project.join("../bare");
    std::fs::create_dir_all(&bare).expect("mkdir");
    let adopt = fixture
        .devports(&["adopt", &bare.display().to_string()])
        .await;
    assert_eq!(adopt.status.code(), Some(0), "{}", err(&adopt));
    assert!(out(&adopt).starts_with("version: 1"));
    assert!(!bare.join(".devports.yml").exists());
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_a_core_commands_say_so_and_run_fails_open() {
    let dir = TempDir::new().expect("tempdir");
    let socket = dir.path().join("missing.sock");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_devports"))
            .args(args)
            .env("DEVPORTS_SOCKET", &socket)
            .env("DEVPORTS_NO_AUTOSTART", "1")
            .output()
            .expect("the CLI runs")
    };
    let ls = run(&["ls"]);
    assert_eq!(ls.status.code(), Some(17));

    let open = run(&["run", "api", "--", "echo", "unprotected"]);
    assert_eq!(open.status.code(), Some(0), "{}", err(&open));
    assert_eq!(out(&open).trim(), "unprotected");
    assert!(err(&open).contains("núcleo inactivo, sin protección"));

    let nothing = run(&["run", "api"]);
    assert_eq!(nothing.status.code(), Some(17));
}
