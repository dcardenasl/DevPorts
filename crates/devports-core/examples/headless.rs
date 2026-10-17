//! Runs the core with no interface at all, to try the CLI against it:
//!
//! ```text
//! cargo run -p devports-core --example headless
//! DEVPORTS_SOCKET=... cargo run -p devports-cli -- ping
//! ```
//!
//! Stops on Ctrl-C and removes its socket.

// An example talks to the terminal.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;

use devports_core::ipc::{IpcServer, Limits, PeerInfo, RequestHandler};
use devports_proto::{ErrorBody, ErrorCode, Op, PROTOCOL_VERSION, PingResult, Request, Response};

struct Headless;

impl RequestHandler for Headless {
    async fn handle(&self, request: Request, _peer: PeerInfo) -> Response {
        match request.op {
            Op::Ping => Response::ok(
                request.id.clone(),
                &PingResult {
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                    protocol: PROTOCOL_VERSION,
                    pid: std::process::id(),
                },
            )
            .unwrap_or_else(|error| {
                Response::err(
                    request.id,
                    ErrorBody::new(ErrorCode::Internal, error.to_string()),
                )
            }),
            _ => Response::err(
                request.id,
                ErrorBody::new(
                    ErrorCode::Unavailable,
                    "this operation is not implemented yet",
                ),
            ),
        }
    }
}

#[tokio::main]
async fn main() {
    let Some(path) = devports_proto::paths::socket_path_from_env() else {
        eprintln!("cannot tell where to put the socket: set DEVPORTS_SOCKET");
        std::process::exit(1);
    };
    let server = match IpcServer::bind(&path, Limits::default()).await {
        Ok(server) => server,
        Err(error) => {
            eprintln!("cannot start: {error}");
            std::process::exit(1);
        }
    };
    println!("listening on {}", server.path().display());
    server
        .serve(Arc::new(Headless), async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
}
