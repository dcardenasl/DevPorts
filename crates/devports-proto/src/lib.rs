//! Wire types of the DevPorts IPC protocol.
//!
//! The `devports` CLI and the core (hosted by the menu-bar app) talk over a
//! Unix socket using one JSON document per line. This crate holds the types
//! both sides serialise, and nothing else: no I/O, no domain logic. That is
//! what lets the CLI depend on it alone and stay small enough to start in a few
//! milliseconds.

/// Version of the line-delimited JSON protocol, carried in the `v` field of
/// every request and response. A peer that receives another value answers with
/// a `bad_version` error instead of guessing.
pub const PROTOCOL_VERSION: u32 = 1;
