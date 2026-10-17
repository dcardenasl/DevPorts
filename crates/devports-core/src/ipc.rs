//! The IPC server: a Unix socket speaking one JSON document per line
//! (`ARQUITECTURA.md`, section 10).
//!
//! What this module guarantees, independently of what the handler does:
//!
//! * the socket is reachable only by the current user: its folder is `0700`, the
//!   socket `0600`, and every connection's peer uid is checked (NF-04);
//! * a second core cannot start while one answers, and a socket left behind by a
//!   crash is cleaned up;
//! * a hostile or broken client cannot exhaust the core: lines are capped at
//!   [`MAX_LINE_BYTES`], reads and handlers have deadlines, and the number of
//!   simultaneous connections is limited;
//! * the protocol version is validated before the handler sees a request.

use std::future::Future;
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use devports_proto::{ErrorBody, ErrorCode, MAX_LINE_BYTES, PROTOCOL_VERSION, Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Semaphore;

/// Who is on the other end of a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerInfo {
    /// User id of the peer process.
    pub uid: u32,
    /// Process id of the peer, when the system reports it.
    pub pid: Option<u32>,
}

/// Answers requests. The server has already checked the peer, the line size and
/// the protocol version.
pub trait RequestHandler: Send + Sync + 'static {
    /// Handles one request.
    fn handle(&self, request: Request, peer: PeerInfo) -> impl Future<Output = Response> + Send;
}

/// Limits that protect the core from its clients.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// How long a client may take to send its request line.
    pub read_timeout: Duration,
    /// How long a handler may take to answer.
    pub handle_timeout: Duration,
    /// How many connections are served at once.
    pub max_connections: usize,
    /// Longest accepted request line, in bytes.
    pub max_line_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_secs(5),
            handle_timeout: Duration::from_secs(10),
            max_connections: 64,
            max_line_bytes: MAX_LINE_BYTES,
        }
    }
}

/// Why the server could not start.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// Another core is already answering on this socket.
    #[error("another DevPorts core is already running on {}", .0.display())]
    AlreadyRunning(PathBuf),
    /// The path is not a socket, so it is not ours to delete.
    #[error("{} exists and is not a socket", .0.display())]
    NotASocket(PathBuf),
    /// The folder of the socket belongs to someone else.
    #[error("{} is not owned by the current user", .0.display())]
    ForeignDirectory(PathBuf),
    /// The path is longer than a Unix socket path can be.
    #[error("socket path is too long: {}", .0.display())]
    PathTooLong(PathBuf),
    /// A system call failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: String,
        /// The system error.
        #[source]
        source: io::Error,
    },
}

fn io_error(context: impl Into<String>) -> impl FnOnce(io::Error) -> IpcError {
    let context = context.into();
    move |source| IpcError::Io { context, source }
}

/// A bound socket, ready to serve.
#[derive(Debug)]
pub struct IpcServer {
    listener: UnixListener,
    path: PathBuf,
    uid: u32,
    limits: Limits,
}

impl IpcServer {
    /// Binds the socket at `path`.
    ///
    /// # Errors
    ///
    /// See [`IpcError`].
    pub async fn bind(path: &Path, limits: Limits) -> Result<Self, IpcError> {
        if path.as_os_str().len() > devports_proto::paths::MAX_SOCKET_PATH_BYTES + 3 {
            return Err(IpcError::PathTooLong(path.to_path_buf()));
        }
        let uid = current_uid()?;
        if let Some(dir) = path.parent() {
            ensure_private_directory(dir, uid)?;
        }
        remove_stale_socket(path).await?;
        let listener =
            UnixListener::bind(path).map_err(io_error(format!("binding {}", path.display())))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(io_error("restricting the socket to its owner"))?;
        Ok(Self {
            listener,
            path: path.to_path_buf(),
            uid,
            limits,
        })
    }

    /// The path the server listens on.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Serves connections until `shutdown` completes, then removes the socket.
    pub async fn serve<H: RequestHandler>(
        self,
        handler: Arc<H>,
        shutdown: impl Future<Output = ()>,
    ) {
        let permits = Arc::new(Semaphore::new(self.limits.max_connections));
        let mut shutdown = std::pin::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                accepted = self.listener.accept() => {
                    let Ok((stream, _)) = accepted else { continue };
                    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                        // Over the limit: tell the client instead of leaving it hanging.
                        tokio::spawn(refuse(stream, ErrorCode::Unavailable, "too many connections"));
                        continue;
                    };
                    let handler = Arc::clone(&handler);
                    let (uid, limits) = (self.uid, self.limits);
                    tokio::spawn(async move {
                        connection(stream, handler, uid, limits).await;
                        drop(permit);
                    });
                }
            }
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The user id of this process, learned from a socket pair so that no `unsafe`
/// call is needed.
fn current_uid() -> Result<u32, IpcError> {
    let (ours, _theirs) = UnixStream::pair().map_err(io_error("creating a socket pair"))?;
    let credentials = ours
        .peer_cred()
        .map_err(io_error("reading own credentials"))?;
    Ok(credentials.uid())
}

/// Creates `dir` as `0700` if needed and makes sure it is ours and private.
fn ensure_private_directory(dir: &Path, uid: u32) -> Result<(), IpcError> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io_error(format!("creating {}", dir.display())))?;
    let metadata =
        std::fs::metadata(dir).map_err(io_error(format!("inspecting {}", dir.display())))?;
    if metadata.uid() != uid {
        return Err(IpcError::ForeignDirectory(dir.to_path_buf()));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(io_error(format!("restricting {}", dir.display())))?;
    }
    Ok(())
}

/// Removes a socket left by a crash. A live one means another core is running;
/// anything that is not a socket is never touched.
async fn remove_stale_socket(path: &Path) -> Result<(), IpcError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(format!("inspecting {}", path.display()))(error)),
    };
    if !metadata.file_type().is_socket() {
        return Err(IpcError::NotASocket(path.to_path_buf()));
    }
    let probe = tokio::time::timeout(Duration::from_millis(300), UnixStream::connect(path)).await;
    if matches!(probe, Ok(Ok(_))) {
        return Err(IpcError::AlreadyRunning(path.to_path_buf()));
    }
    std::fs::remove_file(path).map_err(io_error(format!("removing the stale {}", path.display())))
}

/// Answers with an error and closes.
async fn refuse(mut stream: UnixStream, code: ErrorCode, message: &str) {
    let response = Response::err("", ErrorBody::new(code, message));
    let _ = write_response(&mut stream, &response).await;
}

async fn write_response(stream: &mut UnixStream, response: &Response) -> io::Result<()> {
    let mut line = serde_json::to_vec(response).map_err(io::Error::other)?;
    line.push(b'\n');
    stream.write_all(&line).await?;
    stream.flush().await?;
    stream.shutdown().await
}

/// Why a request line could not be read.
enum LineError {
    TooLarge,
    Closed,
    Io,
}

/// Reads one line of at most `max` bytes, without ever buffering more.
async fn read_line_limited(
    reader: &mut BufReader<&mut UnixStream>,
    max: usize,
) -> Result<Vec<u8>, LineError> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await.map_err(|_| LineError::Io)?;
        if available.is_empty() {
            return if line.is_empty() {
                Err(LineError::Closed)
            } else {
                Ok(line)
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.unwrap_or(available.len());
        if line.len().saturating_add(take) > max {
            return Err(LineError::TooLarge);
        }
        line.extend_from_slice(available.get(..take).unwrap_or_default());
        let consumed = take.saturating_add(usize::from(newline.is_some()));
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(line);
        }
    }
}

async fn connection<H: RequestHandler>(
    mut stream: UnixStream,
    handler: Arc<H>,
    uid: u32,
    limits: Limits,
) {
    let peer = match stream.peer_cred() {
        Ok(credentials) => PeerInfo {
            uid: credentials.uid(),
            pid: credentials.pid().and_then(|pid| u32::try_from(pid).ok()),
        },
        Err(_) => return,
    };
    if !peer_allowed(peer.uid, uid) {
        // Not our user: do not even say why.
        return;
    }

    let read = tokio::time::timeout(limits.read_timeout, async {
        let mut reader = BufReader::new(&mut stream);
        read_line_limited(&mut reader, limits.max_line_bytes).await
    })
    .await;
    let line = match read {
        Ok(Ok(line)) => line,
        Ok(Err(LineError::TooLarge)) => {
            return refuse(stream, ErrorCode::TooLarge, "request line is too long").await;
        }
        Ok(Err(LineError::Closed | LineError::Io)) => return,
        Err(_) => {
            return refuse(
                stream,
                ErrorCode::BadRequest,
                "timed out waiting for the request",
            )
            .await;
        }
    };

    let response = answer(&line, peer, &*handler, limits).await;
    let _ = write_response(&mut stream, &response).await;
}

/// Whether a peer with `peer_uid` may talk to a core run by `server_uid`.
#[must_use]
pub fn peer_allowed(peer_uid: u32, server_uid: u32) -> bool {
    peer_uid == server_uid
}

async fn answer<H: RequestHandler>(
    line: &[u8],
    peer: PeerInfo,
    handler: &H,
    limits: Limits,
) -> Response {
    let request: Request = match serde_json::from_slice(line) {
        Ok(request) => request,
        Err(error) => {
            return Response::err("", classify_parse_error(line, &error));
        }
    };
    if request.v != PROTOCOL_VERSION {
        return Response::err(
            request.id,
            ErrorBody::new(
                ErrorCode::BadVersion,
                format!(
                    "protocol version {} is not supported; this core speaks {PROTOCOL_VERSION}",
                    request.v
                ),
            ),
        );
    }
    let id = request.id.clone();
    match tokio::time::timeout(limits.handle_timeout, handler.handle(request, peer)).await {
        Ok(response) => response,
        Err(_) => Response::err(
            id,
            ErrorBody::new(ErrorCode::Unavailable, "the core took too long to answer"),
        ),
    }
}

/// An unknown `op` is reported as such; anything else is a malformed request.
fn classify_parse_error(line: &[u8], error: &serde_json::Error) -> ErrorBody {
    let value: Option<serde_json::Value> = serde_json::from_slice(line).ok();
    let op_name = value
        .as_ref()
        .and_then(|value| value.get("op"))
        .and_then(serde_json::Value::as_str);
    match op_name {
        Some(name) if error.to_string().contains("unknown variant") => {
            ErrorBody::new(ErrorCode::UnknownOp, format!("unknown operation `{name}`"))
        }
        _ => ErrorBody::new(ErrorCode::BadRequest, format!("malformed request: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_same_user_is_allowed() {
        assert!(peer_allowed(501, 501));
        assert!(!peer_allowed(0, 501));
        assert!(!peer_allowed(502, 501));
    }
}
