//! A blocking client of the core's socket.
//!
//! Blocking and dependency-free on purpose: the CLI starts in milliseconds, runs
//! one request and exits, so it needs no async runtime.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use devports_proto::{ErrorBody, MAX_LINE_BYTES, Op, Request, Response};

/// How long to wait for the core to answer. It has its own deadline of ten
/// seconds, so this only fires if the core is stuck.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(15);

/// Why a call to the core failed.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Nothing is listening: the application is not running.
    #[error("the DevPorts core is not running (no socket at {})", .0.display())]
    CoreNotRunning(PathBuf),
    /// No socket path could be worked out (no `HOME` and no `TMPDIR`).
    #[error("cannot tell where the DevPorts socket is: neither HOME nor TMPDIR is set")]
    NoSocketPath,
    /// The core answered with an error.
    #[error("{}", .0.message)]
    Core(ErrorBody),
    /// The conversation broke down.
    #[error("{0}")]
    Protocol(String),
}

/// The socket the core listens on, from the environment.
///
/// # Errors
///
/// [`ClientError::NoSocketPath`] when it cannot be determined.
pub fn socket_path() -> Result<PathBuf, ClientError> {
    devports_proto::paths::socket_path_from_env().ok_or(ClientError::NoSocketPath)
}

/// Sends one request and returns the typed result.
///
/// # Errors
///
/// See [`ClientError`].
pub fn call<T: serde::de::DeserializeOwned>(path: &Path, op: Op) -> Result<T, ClientError> {
    let mut stream = UnixStream::connect(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            ClientError::CoreNotRunning(path.to_path_buf())
        }
        _ => ClientError::Protocol(format!("cannot connect to {}: {error}", path.display())),
    })?;
    let io = |error: std::io::Error| ClientError::Protocol(error.to_string());
    stream.set_read_timeout(Some(ANSWER_TIMEOUT)).map_err(io)?;
    stream.set_write_timeout(Some(ANSWER_TIMEOUT)).map_err(io)?;

    let id = std::process::id().to_string();
    let mut line = serde_json::to_vec(&Request::new(id, op))
        .map_err(|error| ClientError::Protocol(error.to_string()))?;
    line.push(b'\n');
    stream.write_all(&line).map_err(io)?;

    let mut answer = String::new();
    let read = BufReader::new(
        stream.take(
            u64::try_from(MAX_LINE_BYTES)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        ),
    )
    .read_line(&mut answer)
    .map_err(io)?;
    if read == 0 {
        return Err(ClientError::Protocol(
            "the core closed the connection without answering".to_owned(),
        ));
    }
    let response: Response = serde_json::from_str(answer.trim_end())
        .map_err(|error| ClientError::Protocol(format!("unreadable answer: {error}")))?;
    response.into_result().map_err(ClientError::Core)
}
