//! The CLI against the real core's socket server.

#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::process::Command;
use std::sync::Arc;

use devports_core::ipc::{IpcServer, Limits, PeerInfo, RequestHandler};
use devports_proto::{Op, PROTOCOL_VERSION, PingResult, Request, Response};

struct Core;

impl RequestHandler for Core {
    async fn handle(&self, request: Request, _peer: PeerInfo) -> Response {
        assert_eq!(request.op, Op::Ping, "only ping is sent in this test");
        Response::ok(
            request.id,
            &PingResult {
                version: "9.9.9".to_owned(),
                protocol: PROTOCOL_VERSION,
                pid: 4242,
            },
        )
        .expect("serialisable")
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ping_talks_to_a_real_core_over_the_socket() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let socket = dir.path().join("run").join("devports.sock");
    let server = IpcServer::bind(&socket, Limits::default())
        .await
        .expect("bind");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(server.serve(Arc::new(Core), async {
        let _ = stopped.await;
    }));

    let output = tokio::task::spawn_blocking({
        let socket = socket.clone();
        move || {
            Command::new(env!("CARGO_BIN_EXE_devports"))
                .arg("ping")
                .env("DEVPORTS_SOCKET", socket)
                .output()
                .expect("the CLI runs")
        }
    })
    .await
    .expect("join");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "devports core 9.9.9 (protocolo 1, pid 4242)\n"
    );
    let _ = stop.send(());
    task.await.expect("server task");
}
