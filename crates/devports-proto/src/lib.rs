//! Wire types of the DevPorts IPC protocol.
//!
//! The `devports` CLI and the core (hosted by the menu-bar app) talk over a Unix
//! socket using one JSON document per line (`ARQUITECTURA.md`, section 10). This
//! crate holds the types both sides serialise, and nothing else: no I/O, no
//! domain logic. That is what lets the CLI depend on it alone and stay small
//! enough to start in a few milliseconds.
//!
//! A request carries an [`Op`]; a response carries either a result or an
//! [`ErrorBody`]. *Domain* outcomes that a caller must act on, such as "there is
//! already a Principal for this service", are successful responses with a typed
//! result (see [`run::RunOutcome`]); only protocol and permission failures are
//! errors.

pub mod duration;
pub mod paths;
pub mod preferences;
pub mod run;
pub mod views;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use preferences::{LanguagePreference, Preferences, ThemePreference};
pub use run::{RunOutcome, RunRegister};
pub use views::{InstanceView, LogEntry, LogFacts};

/// Version of the line-delimited JSON protocol, carried in the `v` field of
/// every request and response. A peer that receives another value answers with
/// a `bad_version` error instead of guessing.
pub const PROTOCOL_VERSION: u32 = 1;

/// Longest line, in bytes, either side accepts. A longer line closes the
/// connection: nothing legitimate comes close.
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

/// One request: a protocol version, an id echoed in the response and an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Protocol version; must equal [`PROTOCOL_VERSION`].
    pub v: u32,
    /// Chosen by the caller and echoed back, so a response can be matched.
    pub id: String,
    /// What is being asked.
    #[serde(flatten)]
    pub op: Op,
}

impl Request {
    /// Builds a request for the current protocol version.
    #[must_use]
    pub fn new(id: impl Into<String>, op: Op) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            op,
        }
    }
}

/// The operations the socket exposes: exactly what the CLI may do (ADR-009).
///
/// Stopping a Principal, approving anything, writing a manifest or marking a
/// process as legitimate are deliberately **not** here: they exist only in the
/// application, which runs in the same process as the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "args")]
pub enum Op {
    /// Is the core alive, and which version is it?
    #[serde(rename = "ping")]
    Ping,
    /// Ask to start a service as Principal or Temporal.
    #[serde(rename = "run.register")]
    RunRegister(RunRegister),
    /// List instances.
    #[serde(rename = "ls")]
    Ls(LsArgs),
    /// Stop the Temporal that owns a token.
    #[serde(rename = "stop")]
    Stop(TokenArgs),
    /// Extend the Temporal that owns a token.
    #[serde(rename = "extend")]
    Extend(ExtendArgs),
    /// Read the activity log.
    #[serde(rename = "log")]
    Log(LogArgs),
    /// Propose a manifest for a folder, read-only.
    #[serde(rename = "adopt.propose")]
    AdoptPropose(AdoptArgs),
}

/// Arguments of `ls`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LsArgs {
    /// Restrict to one project by name.
    pub project: Option<String>,
    /// Folder the CLI was run from; the project is resolved from it when
    /// `project` is not given.
    pub cwd: Option<String>,
    /// Also list unregistered and project-less processes.
    #[serde(default)]
    pub all: bool,
}

/// A token that proves ownership of a Temporal instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenArgs {
    /// The `DEVPORTS_STOP_TOKEN` handed out when the Temporal was registered.
    pub token: String,
}

/// Arguments of `extend`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendArgs {
    /// The `DEVPORTS_STOP_TOKEN` of the Temporal.
    pub token: String,
    /// How long to add, in seconds; the core's default when absent.
    pub by_seconds: Option<u64>,
}

/// Arguments of `log`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogArgs {
    /// Only entries from the last this many seconds; today's when absent.
    pub since_seconds: Option<u64>,
    /// Restrict to one project by name.
    pub project: Option<String>,
}

/// Arguments of `adopt.propose`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptArgs {
    /// The folder to propose a manifest for.
    pub dir: String,
}

/// Result of `ping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingResult {
    /// Version of the running core.
    pub version: String,
    /// Protocol version it speaks.
    pub protocol: u32,
    /// Process id of the core.
    pub pid: u32,
}

/// Result of `ls`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LsResult {
    /// The instances, in display order.
    pub instances: Vec<InstanceView>,
}

/// Result of `stop`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopResult {
    /// The short reference of the Temporal.
    pub instance_ref: String,
    /// `false` when it was already gone.
    pub stopped: bool,
}

/// Result of `extend`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendResult {
    /// The new expiry, ISO 8601 with offset.
    pub expires_at: String,
    /// Total time leased so far, in seconds.
    pub leased_total_seconds: u64,
}

/// Result of `log`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogResult {
    /// The entries, oldest first.
    pub entries: Vec<LogEntry>,
}

/// Result of `adopt.propose`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptResult {
    /// The proposed manifest, as YAML text. Nothing was written.
    pub proposal: String,
}

/// Machine-readable reason a request failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The line was not a valid request.
    BadRequest,
    /// The protocol version is not supported.
    BadVersion,
    /// The line exceeded [`MAX_LINE_BYTES`].
    TooLarge,
    /// The operation does not exist.
    UnknownOp,
    /// The project, service or folder was not found.
    NotFound,
    /// The token does not belong to any Temporal.
    UnknownToken,
    /// Extending would pass the maximum lease.
    LeaseLimit,
    /// The core could not do it right now.
    Unavailable,
    /// A bug in the core.
    Internal,
}

/// An error answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Why it failed, for programs.
    pub code: ErrorCode,
    /// Why it failed, for people.
    pub message: String,
}

impl ErrorBody {
    /// Builds an error answer.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// One response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// Protocol version of the answerer.
    pub v: u32,
    /// The id of the request it answers; empty if the request could not be read.
    pub id: String,
    /// Whether `result` (true) or `error` (false) is set.
    pub ok: bool,
    /// The typed result of the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// The failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    /// A successful answer carrying `result`.
    ///
    /// # Errors
    ///
    /// Fails only if `result` cannot be serialised, which is a bug in its type.
    pub fn ok<T: Serialize>(id: impl Into<String>, result: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            ok: true,
            result: Some(serde_json::to_value(result)?),
            error: None,
        })
    }

    /// A failed answer.
    #[must_use]
    pub fn err(id: impl Into<String>, error: ErrorBody) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }

    /// Decodes the answer into the typed result the caller expects.
    ///
    /// # Errors
    ///
    /// The [`ErrorBody`] of a failed answer, or an `internal` error if a
    /// successful answer does not have the expected shape.
    pub fn into_result<T: DeserializeOwned>(self) -> Result<T, ErrorBody> {
        if !self.ok {
            return Err(self.error.unwrap_or_else(|| {
                ErrorBody::new(ErrorCode::Internal, "failed without a reason")
            }));
        }
        let value = self
            .result
            .ok_or_else(|| ErrorBody::new(ErrorCode::Internal, "answer without a result"))?;
        serde_json::from_value(value).map_err(|error| {
            ErrorBody::new(
                ErrorCode::Internal,
                format!("unexpected answer shape: {error}"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_request_has_the_documented_wire_shape() {
        let request = Request::new(
            "7",
            Op::Stop(TokenArgs {
                token: "abc".to_owned(),
            }),
        );
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({"v": 1, "id": "7", "op": "stop", "args": {"token": "abc"}})
        );
    }

    #[test]
    fn ping_needs_no_arguments() {
        let request: Request = serde_json::from_str(r#"{"v":1,"id":"1","op":"ping"}"#).unwrap();
        assert_eq!(request.op, Op::Ping);
        let text = serde_json::to_string(&Request::new("1", Op::Ping)).unwrap();
        assert_eq!(text, r#"{"v":1,"id":"1","op":"ping"}"#);
    }

    #[test]
    fn every_operation_round_trips() {
        let ops = [
            Op::Ping,
            Op::Ls(LsArgs {
                project: Some("shop".to_owned()),
                cwd: None,
                all: true,
            }),
            Op::Stop(TokenArgs {
                token: "t".to_owned(),
            }),
            Op::Extend(ExtendArgs {
                token: "t".to_owned(),
                by_seconds: Some(1800),
            }),
            Op::Log(LogArgs {
                since_seconds: Some(3600),
                project: None,
            }),
            Op::AdoptPropose(AdoptArgs {
                dir: "/work/shop".to_owned(),
            }),
        ];
        for op in ops {
            let request = Request::new("x", op);
            let text = serde_json::to_string(&request).unwrap();
            assert_eq!(
                serde_json::from_str::<Request>(&text).unwrap(),
                request,
                "{text}"
            );
        }
    }

    // covers: I-08, ADR-009
    #[test]
    fn nothing_that_changes_files_or_stops_a_principal_exists_on_the_socket() {
        for forbidden in [
            "stop.instance",
            "stop.principal",
            "terminate",
            "keep",
            "ignore",
            "legitimize",
            "adopt.apply",
            "adopt.write",
            "adopt",
            "extend.instance",
            "shutdown",
        ] {
            let line = format!(r#"{{"v":1,"id":"1","op":"{forbidden}","args":{{}}}}"#);
            assert!(
                serde_json::from_str::<Request>(&line).is_err(),
                "{forbidden} must not be an operation of the socket"
            );
        }
    }

    #[test]
    fn an_unknown_operation_does_not_parse() {
        assert!(
            serde_json::from_str::<Request>(r#"{"v":1,"id":"1","op":"stop-principal","args":{}}"#)
                .is_err()
        );
    }

    #[test]
    fn the_operations_the_cli_must_not_have_do_not_exist() {
        for forbidden in [
            "stop.principal",
            "approve",
            "adopt.write",
            "legitimize",
            "replace",
        ] {
            let text = format!(r#"{{"v":1,"id":"1","op":"{forbidden}","args":{{}}}}"#);
            assert!(
                serde_json::from_str::<Request>(&text).is_err(),
                "{forbidden} parsed"
            );
        }
    }

    #[test]
    fn an_ok_response_decodes_into_its_type() {
        let pong = PingResult {
            version: "0.1.0".to_owned(),
            protocol: PROTOCOL_VERSION,
            pid: 42,
        };
        let response = Response::ok("1", &pong).unwrap();
        let text = serde_json::to_string(&response).unwrap();
        assert!(!text.contains("error"));
        let back: Response = serde_json::from_str(&text).unwrap();
        assert_eq!(back.into_result::<PingResult>().unwrap(), pong);
    }

    #[test]
    fn an_error_response_decodes_into_its_error() {
        let response = Response::err(
            "1",
            ErrorBody::new(ErrorCode::UnknownToken, "no such token"),
        );
        let text = serde_json::to_string(&response).unwrap();
        assert!(text.contains(r#""code":"unknown_token""#));
        let back: Response = serde_json::from_str(&text).unwrap();
        let error = back.into_result::<PingResult>().unwrap_err();
        assert_eq!(error.code, ErrorCode::UnknownToken);
    }

    #[test]
    fn a_result_of_the_wrong_shape_is_reported_not_guessed() {
        let response = Response::ok("1", &json!({"unexpected": true})).unwrap();
        let error = response.into_result::<PingResult>().unwrap_err();
        assert_eq!(error.code, ErrorCode::Internal);
    }
}
