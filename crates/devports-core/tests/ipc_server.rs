//! The IPC server against real sockets (`PRUEBAS.md`: I-11 and the hardening
//! around it): single instance, stale sockets, permissions, hostile clients.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::disallowed_methods
)]

use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use devports_core::ipc::{IpcError, IpcServer, Limits, PeerInfo, RequestHandler};
use devports_proto::{ErrorCode, Op, PROTOCOL_VERSION, PingResult, Request, Response};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::oneshot;

/// Counts what reaches it and answers `ping`; `Ls` hangs for a while so tests can
/// hold connections open.
struct Handler {
    calls: AtomicUsize,
    slow: Duration,
}

impl Handler {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            slow: Duration::from_secs(30),
        })
    }
}

impl RequestHandler for Handler {
    async fn handle(&self, request: Request, _peer: PeerInfo) -> Response {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if request.op == Op::Ping {
            Response::ok(
                request.id,
                &PingResult {
                    version: "test".to_owned(),
                    protocol: PROTOCOL_VERSION,
                    pid: std::process::id(),
                },
            )
            .expect("serialisable")
        } else {
            tokio::time::sleep(self.slow).await;
            Response::err(
                request.id,
                devports_proto::ErrorBody::new(ErrorCode::Internal, "late"),
            )
        }
    }
}

struct Running {
    path: PathBuf,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
    handler: Arc<Handler>,
    tempdir: TempDir,
}

impl Running {
    async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.await.expect("server task");
    }
}

fn fast_limits() -> Limits {
    Limits {
        read_timeout: Duration::from_millis(300),
        handle_timeout: Duration::from_millis(400),
        max_connections: 64,
        max_line_bytes: 1024 * 1024,
    }
}

async fn start(limits: Limits) -> Running {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("run").join("devports.sock");
    let server = IpcServer::bind(&path, limits).await.expect("bind");
    let handler = Handler::new();
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(server.serve(Arc::clone(&handler), async {
        let _ = stopped.await;
    }));
    Running {
        path,
        stop: Some(stop),
        task,
        handler,
        tempdir: dir,
    }
}

/// Sends raw bytes and reads everything the server answers until it closes.
async fn exchange(path: &Path, bytes: &[u8]) -> Vec<u8> {
    let mut stream = UnixStream::connect(path).await.expect("connect");
    stream.write_all(bytes).await.expect("write");
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).await.expect("read");
    answer
}

fn parse(answer: &[u8]) -> Response {
    serde_json::from_slice(answer).expect("a JSON response")
}

fn ping_line(id: &str) -> Vec<u8> {
    let mut line = serde_json::to_vec(&Request::new(id, Op::Ping)).expect("serialise");
    line.push(b'\n');
    line
}

#[tokio::test]
async fn answers_a_ping_and_echoes_the_id() {
    let server = start(fast_limits()).await;
    let response = parse(&exchange(&server.path, &ping_line("req-7")).await);
    assert!(response.ok);
    assert_eq!(response.id, "req-7");
    let pong = response.into_result::<PingResult>().expect("a ping result");
    assert_eq!(pong.protocol, PROTOCOL_VERSION);
    server.stop().await;
}

// covers: I-11
#[tokio::test]
async fn the_socket_is_private_to_its_owner() {
    let server = start(fast_limits()).await;
    let socket = std::fs::symlink_metadata(&server.path).expect("socket");
    assert!(socket.file_type().is_socket());
    assert_eq!(socket.permissions().mode() & 0o777, 0o600);
    let dir = std::fs::metadata(server.path.parent().expect("dir")).expect("dir");
    assert_eq!(dir.permissions().mode() & 0o777, 0o700);
    server.stop().await;
}

#[tokio::test]
async fn a_loosely_permissioned_existing_folder_is_tightened() {
    let dir = TempDir::new().expect("tempdir");
    let folder = dir.path().join("run");
    std::fs::create_dir(&folder).expect("mkdir");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let server = IpcServer::bind(&folder.join("devports.sock"), fast_limits())
        .await
        .expect("bind");

    let mode = std::fs::metadata(&folder)
        .expect("dir")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
    drop(server);
}

/// I-11: a second core must not start while one answers.
// covers: I-11
#[tokio::test]
async fn a_second_core_cannot_take_over_a_live_socket() {
    let first = start(fast_limits()).await;

    let second = IpcServer::bind(&first.path, fast_limits()).await;

    assert!(
        matches!(second, Err(IpcError::AlreadyRunning(_))),
        "{second:?}"
    );
    // The first one is untouched.
    assert!(parse(&exchange(&first.path, &ping_line("1")).await).ok);
    first.stop().await;
}

/// I-11: a socket left behind by a crash is cleaned up.
// covers: I-11
#[tokio::test]
async fn a_socket_left_by_a_crash_is_replaced() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("run").join("devports.sock");
    std::fs::create_dir_all(path.parent().expect("dir")).expect("mkdir");
    // Bind and drop without removing the file: exactly what a crash leaves.
    drop(std::os::unix::net::UnixListener::bind(&path).expect("bind"));
    assert!(path.exists());

    let server = IpcServer::bind(&path, fast_limits())
        .await
        .expect("bind over the stale socket");

    drop(server);
}

#[tokio::test]
async fn something_that_is_not_a_socket_is_never_deleted() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("run").join("devports.sock");
    std::fs::create_dir_all(path.parent().expect("dir")).expect("mkdir");
    std::fs::write(&path, "precious").expect("write");

    let result = IpcServer::bind(&path, fast_limits()).await;

    assert!(matches!(result, Err(IpcError::NotASocket(_))));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "precious");
}

#[tokio::test]
async fn an_overlong_path_is_refused_before_touching_the_disk() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("a".repeat(120)).join("devports.sock");
    let result = IpcServer::bind(&path, fast_limits()).await;
    assert!(matches!(result, Err(IpcError::PathTooLong(_))));
}

#[tokio::test]
async fn a_line_over_the_cap_is_refused_without_reaching_the_handler() {
    let server = start(fast_limits()).await;
    let mut huge = vec![b'x'; 3 * 1024 * 1024];
    huge.push(b'\n');

    let mut stream = UnixStream::connect(&server.path).await.expect("connect");
    // The server stops reading at the cap and answers; the rest of the write may
    // fail with a broken pipe, which is the point.
    let _ = stream.write_all(&huge).await;
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer).await;

    let response = parse(&answer);
    assert_eq!(response.error.expect("error").code, ErrorCode::TooLarge);
    assert_eq!(server.handler.calls.load(Ordering::SeqCst), 0);
    server.stop().await;
}

#[tokio::test]
async fn malformed_requests_get_a_specific_error() {
    let server = start(fast_limits()).await;

    let garbage = parse(&exchange(&server.path, b"this is not json\n").await);
    assert_eq!(garbage.error.expect("error").code, ErrorCode::BadRequest);

    let unknown = parse(
        &exchange(
            &server.path,
            b"{\"v\":1,\"id\":\"1\",\"op\":\"stop-principal\",\"args\":{}}\n",
        )
        .await,
    );
    let error = unknown.error.expect("error");
    assert_eq!(error.code, ErrorCode::UnknownOp);
    assert!(error.message.contains("stop-principal"));

    let version =
        parse(&exchange(&server.path, b"{\"v\":99,\"id\":\"9\",\"op\":\"ping\"}\n").await);
    assert_eq!(version.id, "9");
    assert_eq!(version.error.expect("error").code, ErrorCode::BadVersion);

    assert_eq!(server.handler.calls.load(Ordering::SeqCst), 0);
    server.stop().await;
}

#[tokio::test]
async fn a_client_that_never_sends_a_line_is_cut_off() {
    let server = start(fast_limits()).await;
    let mut stream = UnixStream::connect(&server.path).await.expect("connect");

    let mut answer = Vec::new();
    let waited =
        tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut answer)).await;

    assert!(waited.is_ok(), "the server must close the idle connection");
    assert_eq!(
        parse(&answer).error.expect("error").code,
        ErrorCode::BadRequest
    );
    server.stop().await;
}

#[tokio::test]
async fn connections_over_the_limit_are_told_so() {
    let limits = Limits {
        max_connections: 2,
        handle_timeout: Duration::from_secs(20),
        ..fast_limits()
    };
    let server = start(limits).await;
    // `ls` makes the handler hang, holding a permit each.
    let ls = b"{\"v\":1,\"id\":\"h\",\"op\":\"ls\",\"args\":{}}\n".to_vec();
    let mut holders = Vec::new();
    for _ in 0..2 {
        let mut stream = UnixStream::connect(&server.path).await.expect("connect");
        stream.write_all(&ls).await.expect("write");
        holders.push(stream);
    }
    tokio::time::sleep(Duration::from_millis(150)).await;

    let answer = exchange(&server.path, &ping_line("late")).await;

    assert_eq!(
        parse(&answer).error.expect("error").code,
        ErrorCode::Unavailable
    );
    drop(holders);
    server.stop().await;
}

#[tokio::test]
async fn a_handler_that_hangs_is_cut_off() {
    let server = start(fast_limits()).await;
    let ls = b"{\"v\":1,\"id\":\"slow\",\"op\":\"ls\",\"args\":{}}\n";

    let answer = tokio::time::timeout(Duration::from_secs(5), exchange(&server.path, ls))
        .await
        .expect("the server answers within its own deadline");

    let response = parse(&answer);
    assert_eq!(response.id, "slow");
    assert_eq!(response.error.expect("error").code, ErrorCode::Unavailable);
    server.stop().await;
}

#[tokio::test]
async fn stopping_the_server_removes_the_socket() {
    let server = start(fast_limits()).await;
    let path = server.path.clone();
    assert!(path.exists());
    // Keep the tempdir alive past the stop by checking before it drops.
    let dir = server.tempdir.path().to_path_buf();
    server.stop().await;
    assert!(!path.exists());
    assert!(!dir.join("run").join("devports.sock").exists());
}
